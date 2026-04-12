use gallery_log::*;
use std::fs;
use std::process;

fn invalid() -> ! {
    eprintln!("invalid");
    process::exit(111);
}

fn integrity() -> ! {
    eprintln!("integrity violation");
    process::exit(111);
}

struct Args {
    timestamp: u64,
    token: String,
    person_type: PersonType,
    name: String,
    action: Action,
    room: Option<u32>,
    log_path: String,
}

fn parse_args(raw: &[String]) -> Result<Args, ()> {
    let mut timestamp: Option<u64> = None;
    let mut token: Option<String> = None;
    let mut person_type: Option<PersonType> = None;
    let mut name: Option<String> = None;
    let mut action: Option<Action> = None;
    let mut room: Option<u32> = None;
    let mut log_path: Option<String> = None;

    let mut i = 0usize;
    while i < raw.len() {
        match raw[i].as_str() {
            "-T" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                timestamp = Some(raw[i].parse::<u64>().map_err(|_| ())?);
            }
            "-K" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                token = Some(raw[i].clone());
            }
            "-E" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                if person_type.is_some() { return Err(()); }
                person_type = Some(PersonType::Employee);
                name = Some(raw[i].clone());
            }
            "-G" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                if person_type.is_some() { return Err(()); }
                person_type = Some(PersonType::Guest);
                name = Some(raw[i].clone());
            }
            "-A" => {
                if action.is_some() { return Err(()); }
                action = Some(Action::Arrival);
            }
            "-L" => {
                if action.is_some() { return Err(()); }
                action = Some(Action::Departure);
            }
            "-R" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                room = Some(raw[i].parse::<u32>().map_err(|_| ())?);
            }
            s if !s.starts_with('-') => {
                if log_path.is_some() { return Err(()); }
                log_path = Some(s.to_string());
            }
            _ => return Err(()),
        }
        i += 1;
    }

    let timestamp = timestamp.ok_or(())?;
    let token = token.ok_or(())?;
    let person_type = person_type.ok_or(())?;
    let name = name.ok_or(())?;
    let action = action.ok_or(())?;
    let log_path = log_path.ok_or(())?;

    // timestamp must be > 0
    if timestamp == 0 { return Err(()); }

    if !is_valid_token(&token) { return Err(()); }
    if !is_valid_name(&name) { return Err(()); }

    Ok(Args { timestamp, token, person_type, name, action, room, log_path })
}

fn run_single(args: &[String]) {
    let a = parse_args(args).unwrap_or_else(|_| invalid());
    let key = derive_key(&a.token);

    // Hold exclusive lock for the entire load → validate → append sequence
    let _lock = lock_log_exclusive(&a.log_path).unwrap_or_else(|_| invalid());

    let log = match load_log(&a.log_path, &key) {
        Ok(l) => l,
        Err(LogError::Integrity) => integrity(),
        Err(LogError::Io(_)) => invalid(),
    };

    let mut state = compute_state(&log.entries).unwrap_or_else(|_| integrity());

    let entry = LogEntry {
        timestamp: a.timestamp,
        person_type: a.person_type,
        name: a.name,
        action: a.action,
        room: a.room,
    };

    if state.apply(&entry).is_err() {
        invalid();
    }

    append_entry(&a.log_path, &key, &entry, &log.last_hash).unwrap_or_else(|_| invalid());
    // _lock dropped here, releasing the exclusive lock
}

fn run_batch(batch_file: &str) {
    let content = fs::read_to_string(batch_file).unwrap_or_else(|_| invalid());

    // Cache: (log_path, key) → (state, last_hash, lock_handle)
    // Load + validate once per log/key combination, then apply incrementally.
    let mut cache: Option<(String, [u8; 32], GalleryState, [u8; 32], std::fs::File)> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let args: Vec<String> = shell_split(line);

        let a = match parse_args(&args) {
            Ok(a) => a,
            Err(_) => { eprintln!("invalid"); continue; }
        };
        let key = derive_key(&a.token);

        // Check if we can reuse the cached state or need to reload
        let cache_hit = match &cache {
            Some((path, k, _, _, _)) => *path == a.log_path && *k == key,
            None => false,
        };

        if !cache_hit {
            // Drop old lock before acquiring a new one
            drop(cache.take());

            let lock = match lock_log_exclusive(&a.log_path) {
                Ok(f) => f,
                Err(_) => { eprintln!("invalid"); continue; }
            };
            let log = match load_log(&a.log_path, &key) {
                Ok(l) => l,
                Err(LogError::Integrity) => { eprintln!("integrity violation"); continue; }
                Err(LogError::Io(_)) => { eprintln!("invalid"); continue; }
            };
            let state = match compute_state(&log.entries) {
                Ok(s) => s,
                Err(_) => { eprintln!("integrity violation"); continue; }
            };
            cache = Some((a.log_path.clone(), key, state, log.last_hash, lock));
        }

        let (_, _, ref mut state, ref mut last_hash, _) = cache.as_mut().unwrap();

        let entry = LogEntry {
            timestamp: a.timestamp,
            person_type: a.person_type,
            name: a.name,
            action: a.action,
            room: a.room,
        };
        if state.apply(&entry).is_err() {
            eprintln!("invalid");
            continue;
        }
        match append_entry(&a.log_path, &key, &entry, last_hash) {
            Ok(new_hash) => { *last_hash = new_hash; }
            Err(_) => { eprintln!("invalid"); }
        }
    }
}

fn shell_split(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => in_quote = !in_quote,
            ' ' | '\t' if !in_quote => {
                if !cur.is_empty() { args.push(cur.clone()); cur.clear(); }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() { args.push(cur); }
    args
}

fn main() {
    let all_args: Vec<String> = std::env::args().skip(1).collect();
    if all_args.is_empty() { invalid(); }

    if all_args[0] == "-B" {
        if all_args.len() != 2 { invalid(); }
        run_batch(&all_args[1]);
    } else {
        run_single(&all_args);
    }
}