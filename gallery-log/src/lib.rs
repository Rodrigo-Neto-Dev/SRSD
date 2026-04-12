use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

type HmacSha256 = Hmac<Sha256>;

// ── Key derivation ────────────────────────────────────────────────────────────

pub fn derive_key(token: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().into()
}

// ── Stream cipher  keystream = SHA256(K || nonce || counter) ────────────────

pub fn stream_cipher(key: &[u8; 32], nonce: &[u8; 32], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter: u64 = 0;
    let mut ks: Vec<u8> = Vec::new();
    let mut ks_off = 0usize;
    for &b in data {
        if ks_off >= ks.len() {
            let mut h = Sha256::new();
            h.update(key);
            h.update(nonce);
            h.update(counter.to_le_bytes());
            ks = h.finalize().to_vec();
            ks_off = 0;
            counter += 1;
        }
        out.push(b ^ ks[ks_off]);
        ks_off += 1;
    }
    out
}

// ── MAC ───────────────────────────────────────────────────────────────────────

pub fn compute_mac(key: &[u8; 32], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("valid key");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

// ── Hash ──────────────────────────────────────────────────────────────────────

pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

// ── Entry types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PersonType {
    Employee,
    Guest,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Arrival,
    Departure,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub person_type: PersonType,
    pub name: String,
    pub action: Action,
    pub room: Option<u32>,
}

impl LogEntry {
    pub fn encode(&self, prev_hash: &[u8; 32]) -> Vec<u8> {
        let pt = match self.person_type { PersonType::Employee => "E", PersonType::Guest => "G" };
        let act = match self.action { Action::Arrival => "A", Action::Departure => "L" };
        let room_s = self.room.map(|r| r.to_string()).unwrap_or_default();
        let mut out = format!("{}|{}|{}|{}|{}", self.timestamp, pt, self.name, act, room_s)
            .into_bytes();
        out.extend_from_slice(prev_hash); // raw 32 bytes, no hex encoding
        out
    }

    pub fn decode(data: &[u8]) -> Option<(Self, [u8; 32])> {
        if data.len() < 32 { return None; }
        let (text_part, hash_part) = data.split_at(data.len() - 32);
        let mut prev_hash = [0u8; 32];
        prev_hash.copy_from_slice(hash_part);
        let s = std::str::from_utf8(text_part).ok()?;
        let mut it = s.splitn(5, '|');
        let timestamp = it.next()?.parse::<u64>().ok()?;
        let pt = match it.next()? { "E" => PersonType::Employee, "G" => PersonType::Guest, _ => return None };
        let name = it.next()?.to_owned();
        let action = match it.next()? { "A" => Action::Arrival, "L" => Action::Departure, _ => return None };
        let room_s = it.next()?;
        let room = if room_s.is_empty() { None } else { Some(room_s.parse::<u32>().ok()?) };
        Some((LogEntry { timestamp, person_type: pt, name, action, room }, prev_hash))
    }
}

// ── On-disk record: [u32 BE length][ciphertext][32-byte MAC] ─────────────────

pub fn pack_record(ct: &[u8], mac: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + ct.len() + 32);
    v.extend_from_slice(&(ct.len() as u32).to_be_bytes());
    v.extend_from_slice(ct);
    v.extend_from_slice(mac);
    v
}

pub fn unpack_record(buf: &[u8], off: usize) -> Option<(Vec<u8>, [u8; 32], usize)> {
    if off + 4 > buf.len() { return None; }
    let len = u32::from_be_bytes(buf[off..off+4].try_into().ok()?) as usize;
    let ce = off + 4 + len;
    let me = ce + 32;
    if me > buf.len() { return None; }
    let ct = buf[off+4..ce].to_vec();
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&buf[ce..me]);
    Some((ct, mac, me))
}

// ── Error ─────────────────────────────────────────────────────────────────────

pub enum LogError {
    Integrity,
    Io(io::Error),
}

impl From<io::Error> for LogError {
    fn from(e: io::Error) -> Self { LogError::Io(e) }
}

// ── Loaded log ────────────────────────────────────────────────────────────────

pub struct LoadedLog {
    pub entries: Vec<LogEntry>,
    pub last_hash: [u8; 32],
}

pub fn load_log(path: &str, key: &[u8; 32]) -> Result<LoadedLog, LogError> {
    if !Path::new(path).exists() {
        return Ok(LoadedLog { entries: vec![], last_hash: [0u8; 32] });
    }
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;

    let mut off = 0usize;
    let mut entries = Vec::new();
    let mut prev_hash = [0u8; 32];

    while off < buf.len() {
        let (ct, mac, next) = unpack_record(&buf, off).ok_or(LogError::Integrity)?;
        if compute_mac(key, &ct) != mac { return Err(LogError::Integrity); }
        let plain = stream_cipher(key, &prev_hash, &ct);
        let (entry, stored_prev) = LogEntry::decode(&plain).ok_or(LogError::Integrity)?;
        if stored_prev != prev_hash { return Err(LogError::Integrity); }
        prev_hash = hash_bytes(&ct);
        entries.push(entry);
        off = next;
    }

    Ok(LoadedLog { entries, last_hash: prev_hash })
}

// ── Append ────────────────────────────────────────────────────────────────────

pub fn append_entry(path: &str, key: &[u8; 32], entry: &LogEntry, prev_hash: &[u8; 32]) -> io::Result<[u8; 32]> {
    let plain = entry.encode(prev_hash);
    let ct = stream_cipher(key, prev_hash, &plain);
    let mac = compute_mac(key, &ct);
    let record = pack_record(&ct, &mac);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(&record)?;
    Ok(hash_bytes(&ct))
}

// ── Gallery state machine ─────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub struct GalleryState {
    pub employees: HashMap<String, Option<u32>>,
    pub guests: HashMap<String, Option<u32>>,
    pub last_timestamp: u64,
    pub has_entries: bool,
}

impl GalleryState {
    pub fn apply(&mut self, entry: &LogEntry) -> Result<(), ()> {
        if self.has_entries && entry.timestamp <= self.last_timestamp { return Err(()); }
        let map = match entry.person_type {
            PersonType::Employee => &mut self.employees,
            PersonType::Guest => &mut self.guests,
        };
        match (&entry.action, entry.room) {
            (Action::Arrival, None) => {
                if map.contains_key(&entry.name) { return Err(()); }
                map.insert(entry.name.clone(), None);
            }
            (Action::Arrival, Some(r)) => {
                match map.get(&entry.name) {
                    Some(None) => { map.insert(entry.name.clone(), Some(r)); }
                    _ => return Err(()),
                }
            }
            (Action::Departure, None) => {
                match map.get(&entry.name) {
                    Some(None) => { map.remove(&entry.name); }
                    _ => return Err(()),
                }
            }
            (Action::Departure, Some(r)) => {
                match map.get(&entry.name) {
                    Some(Some(cur)) if *cur == r => { map.insert(entry.name.clone(), None); }
                    _ => return Err(()),
                }
            }
        }
        self.last_timestamp = entry.timestamp;
        self.has_entries = true;
        Ok(())
    }
}

pub fn compute_state(entries: &[LogEntry]) -> Result<GalleryState, ()> {
    let mut state = GalleryState::default();
    for e in entries { state.apply(e)?; }
    Ok(state)
}

// ── Output helpers ────────────────────────────────────────────────────────────

pub fn sorted_names(map: &HashMap<String, Option<u32>>) -> Vec<String> {
    let mut v: Vec<_> = map.keys().cloned().collect();
    v.sort();
    v
}

pub fn rooms_occupancy(
    employees: &HashMap<String, Option<u32>>,
    guests: &HashMap<String, Option<u32>>,
) -> BTreeMap<u32, (Vec<String>, Vec<String>)> {
    let mut map: BTreeMap<u32, (Vec<String>, Vec<String>)> = BTreeMap::new();
    for (n, r) in employees { if let Some(r) = r { map.entry(*r).or_default().0.push(n.clone()); } }
    for (n, r) in guests   { if let Some(r) = r { map.entry(*r).or_default().1.push(n.clone()); } }
    for (_, (e, g)) in map.iter_mut() { e.sort(); g.sort(); }
    map
}

// ── Room history ──────────────────────────────────────────────────────────────

pub fn room_history(entries: &[LogEntry], pt: &PersonType, name: &str) -> Option<Vec<u32>> {
    if !entries.iter().any(|e| &e.person_type == pt && e.name == name) { return None; }
    let mut rooms = Vec::new();
    for e in entries {
        if &e.person_type == pt && e.name == name {
            if matches!(e.action, Action::Arrival) {
                if let Some(r) = e.room { rooms.push(r); }
            }
        }
    }
    Some(rooms)
}

// ── Intersection ──────────────────────────────────────────────────────────────

/// Returns the sorted list of room IDs that were occupied by ALL specified
/// persons at the same time, over the complete history of the gallery.
/// Persons not present in the log are ignored (not treated as an error).
pub fn intersection_query(
    entries: &[LogEntry],
    targets: &[(PersonType, String)],
) -> Vec<u32> {
    if targets.is_empty() { return vec![]; }

    type Iv = (u32, u64, u64); // (room, enter_ts, leave_ts)
    let mut intervals: HashMap<(PersonType, String), Vec<Iv>> = HashMap::new();
    let mut cur: HashMap<(PersonType, String), (u32, u64)> = HashMap::new();

    for e in entries {
        let k = (e.person_type.clone(), e.name.clone());
        match (&e.action, e.room) {
            (Action::Arrival, Some(r)) => { cur.insert(k, (r, e.timestamp)); }
            (Action::Departure, Some(_)) => {
                if let Some((r, ts)) = cur.remove(&k) {
                    intervals.entry(k).or_default().push((r, ts, e.timestamp));
                }
            }
            _ => {}
        }
    }
    // Anyone still in a room at end-of-log gets u64::MAX as leave time
    for (k, (r, ts)) in cur {
        intervals.entry(k).or_default().push((r, ts, u64::MAX));
    }

    // Keep only targets that actually appear in the log; ignore unknowns per spec
    let known_targets: Vec<&Vec<Iv>> = targets.iter()
        .filter_map(|(pt, n)| intervals.get(&(pt.clone(), n.clone())))
        .collect();

    // Need at least one known target to do anything meaningful
    if known_targets.is_empty() { return vec![]; }

    // Collect every room ID that ever appears across all known targets
    let all_rooms: HashSet<u32> = known_targets.iter()
        .flat_map(|ivs| ivs.iter().map(|&(r, _, _)| r))
        .collect();

    // A room qualifies if there exists a time window during which ALL known
    // targets were simultaneously present in that room.
    let mut result: Vec<u32> = all_rooms.into_iter().filter(|&room| {
        // For each known target collect intervals in this room
        let per_target: Vec<Vec<(u64, u64)>> = known_targets.iter().map(|ivs| {
            ivs.iter()
                .filter(|&&(r, _, _)| r == room)
                .map(|&(_, s, e)| (s, e))
                .collect()
        }).collect();

        // Every target must have at least one interval in this room
        if per_target.iter().any(|v| v.is_empty()) { return false; }

        // Check whether any combination of one interval per target overlaps
        // i.e. intersection of [start, end) across targets is non-empty.
        // We try all combinations: overlap_start = max of starts, overlap_end = min of ends.
        fn overlaps(per_target: &[Vec<(u64, u64)>], idx: usize, win_start: u64, win_end: u64) -> bool {
            if win_start >= win_end { return false; }
            if idx == per_target.len() { return true; }
            for &(s, e) in &per_target[idx] {
                let new_start = win_start.max(s);
                let new_end   = win_end.min(e);
                if overlaps(per_target, idx + 1, new_start, new_end) { return true; }
            }
            false
        }
        overlaps(&per_target, 0, 0, u64::MAX)
    }).collect();

    result.sort_unstable();
    result
}

// ── File locking ─────────────────────────────────────────────────────────────

/// Acquire an exclusive lock on a companion `.lock` file.
/// Returns the lock file handle — the lock is held until the handle is dropped.
pub fn lock_log_exclusive(path: &str) -> io::Result<File> {
    let lock_path = format!("{}.lock", path);
    let f = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(f)
}

/// Acquire a shared lock on a companion `.lock` file.
pub fn lock_log_shared(path: &str) -> io::Result<File> {
    let lock_path = format!("{}.lock", path);
    let f = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_SH) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(f)
}

// ── Validation helpers ────────────────────────────────────────────────────────

pub fn is_valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic())
}

pub fn is_valid_token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}