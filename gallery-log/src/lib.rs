use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

type HmacSha256 = Hmac<Sha256>;

// ── Key derivation ────────────────────────────────────────────────────────────

pub fn derive_key(token: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().into()
}

// ── Stream cipher  keystream = SHA256(K || counter) ──────────────────────────

pub fn stream_cipher(key: &[u8; 32], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter: u64 = 0;
    let mut ks: Vec<u8> = Vec::new();
    let mut ks_off = 0usize;
    for &b in data {
        if ks_off >= ks.len() {
            let mut h = Sha256::new();
            h.update(key);
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
        format!("{}|{}|{}|{}|{}|{}", self.timestamp, pt, self.name, act, room_s, hex::encode(prev_hash))
            .into_bytes()
    }

    pub fn decode(data: &[u8]) -> Option<(Self, [u8; 32])> {
        let s = std::str::from_utf8(data).ok()?;
        let mut it = s.splitn(6, '|');
        let timestamp = it.next()?.parse::<u64>().ok()?;
        let pt = match it.next()? { "E" => PersonType::Employee, "G" => PersonType::Guest, _ => return None };
        let name = it.next()?.to_owned();
        let action = match it.next()? { "A" => Action::Arrival, "L" => Action::Departure, _ => return None };
        let room_s = it.next()?;
        let room = if room_s.is_empty() { None } else { Some(room_s.parse::<u32>().ok()?) };
        let prev_hex = it.next()?;
        let prev_bytes = hex::decode(prev_hex).ok()?;
        if prev_bytes.len() != 32 { return None; }
        let mut prev_hash = [0u8; 32];
        prev_hash.copy_from_slice(&prev_bytes);
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
    let mut f = File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;

    let mut off = 0usize;
    let mut entries = Vec::new();
    let mut prev_hash = [0u8; 32];

    while off < buf.len() {
        let (ct, mac, next) = unpack_record(&buf, off).ok_or(LogError::Integrity)?;
        if compute_mac(key, &ct) != mac { return Err(LogError::Integrity); }
        let plain = stream_cipher(key, &ct);
        let (entry, stored_prev) = LogEntry::decode(&plain).ok_or(LogError::Integrity)?;
        if stored_prev != prev_hash { return Err(LogError::Integrity); }
        prev_hash = hash_bytes(&ct);
        entries.push(entry);
        off = next;
    }

    Ok(LoadedLog { entries, last_hash: prev_hash })
}

// ── Append ────────────────────────────────────────────────────────────────────

pub fn append_entry(path: &str, key: &[u8; 32], entry: &LogEntry, prev_hash: &[u8; 32]) -> io::Result<()> {
    let plain = entry.encode(prev_hash);
    let ct = stream_cipher(key, &plain);
    let mac = compute_mac(key, &ct);
    let record = pack_record(&ct, &mac);
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(&record)?;
    Ok(())
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
    let mut seen = HashSet::new();
    for e in entries {
        if &e.person_type == pt && e.name == name {
            if matches!(e.action, Action::Arrival) {
                if let Some(r) = e.room { if seen.insert(r) { rooms.push(r); } }
            }
        }
    }
    Some(rooms)
}

// ── Intersection ──────────────────────────────────────────────────────────────

pub fn intersection_query(
    entries: &[LogEntry],
    targets: &[(PersonType, String)],
) -> Vec<(PersonType, String)> {
    if targets.is_empty() { return vec![]; }

    type Iv = (u32, u64, u64); // room, enter, leave
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
    for (k, (r, ts)) in cur {
        intervals.entry(k).or_default().push((r, ts, u64::MAX));
    }

    let target_ivs: Vec<Option<&Vec<Iv>>> = targets.iter()
        .map(|(pt, n)| intervals.get(&(pt.clone(), n.clone())))
        .collect();
    if target_ivs.iter().any(|o| o.is_none()) { return vec![]; }
    let target_ivs: Vec<&Vec<Iv>> = target_ivs.into_iter().map(|o| o.unwrap()).collect();

    let mut result = Vec::new();
    'outer: for (cand, civs) in &intervals {
        if targets.iter().any(|(pt, n)| pt == &cand.0 && n == &cand.1) { continue; }
        for civ in civs {
            let (room, cs, ce) = *civ;
            let all = target_ivs.iter().all(|tivs| {
                tivs.iter().any(|&(tr, ts, te)| tr == room && ts < ce && te > cs)
            });
            if all { result.push((cand.0.clone(), cand.1.clone())); continue 'outer; }
        }
    }
    result.sort_by(|a, b| a.1.cmp(&b.1));
    result
}

// ── Validation helpers ────────────────────────────────────────────────────────

pub fn is_valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic())
}

pub fn is_valid_token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}