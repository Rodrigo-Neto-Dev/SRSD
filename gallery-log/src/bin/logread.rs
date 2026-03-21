use gallery_log::*;
use std::process;

fn invalid() -> ! {
    eprintln!("invalid");
    process::exit(111);
}

fn integrity() -> ! {
    eprintln!("integrity violation");
    process::exit(111);
}

#[derive(Debug)]
enum Command {
    State,
    RoomHistory { person_type: PersonType, name: String },
    Intersection { persons: Vec<(PersonType, String)> },
}

struct Args {
    token: String,
    command: Command,
    log_path: String,
}

fn parse_args(raw: &[String]) -> Result<Args, ()> {
    let mut token: Option<String> = None;
    let mut mode_s = false;
    let mut mode_r = false;
    let mut mode_i = false;
    let mut persons: Vec<(PersonType, String)> = Vec::new();
    let mut log_path: Option<String> = None;

    let mut i = 0usize;
    while i < raw.len() {
        match raw[i].as_str() {
            "-K" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                token = Some(raw[i].clone());
            }
            "-S" => { mode_s = true; }
            "-R" => { mode_r = true; }
            "-I" => { mode_i = true; }
            "-E" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                persons.push((PersonType::Employee, raw[i].clone()));
            }
            "-G" => {
                i += 1;
                if i >= raw.len() { return Err(()); }
                persons.push((PersonType::Guest, raw[i].clone()));
            }
            s if !s.starts_with('-') => {
                if log_path.is_some() { return Err(()); }
                log_path = Some(s.to_string());
            }
            _ => return Err(()),
        }
        i += 1;
    }

    let token = token.ok_or(())?;
    let log_path = log_path.ok_or(())?;

    if !is_valid_token(&token) { return Err(()); }

    // Validate person names
    for (_, n) in &persons {
        if !is_valid_name(n) { return Err(()); }
    }

    // Exactly one mode flag
    let mode_count = mode_s as u8 + mode_r as u8 + mode_i as u8;
    if mode_count != 1 { return Err(()); }

    let command = if mode_s {
        if !persons.is_empty() { return Err(()); }
        Command::State
    } else if mode_r {
        if persons.len() != 1 { return Err(()); }
        let (pt, name) = persons.remove(0);
        Command::RoomHistory { person_type: pt, name }
    } else {
        // intersection: need at least 1 person (spec says multiple)
        if persons.is_empty() { return Err(()); }
        Command::Intersection { persons }
    };

    Ok(Args { token, command, log_path })
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() { invalid(); }

    let args = parse_args(&raw).unwrap_or_else(|_| invalid());
    let key = derive_key(&args.token);

    let log = match load_log(&args.log_path, &key) {
        Ok(l) => l,
        Err(LogError::Integrity) => integrity(),
        Err(LogError::Io(_)) => invalid(),
    };

    match args.command {
        Command::State => {
            let state = compute_state(&log.entries).unwrap_or_else(|_| integrity());

            // Line 1: employees in gallery (sorted)
            let emps = sorted_names(&state.employees);
            println!("{}", emps.join(","));

            // Line 2: guests in gallery (sorted)
            let gsts = sorted_names(&state.guests);
            println!("{}", gsts.join(","));

            // Next lines: room_id: sorted employees,sorted guests
            let rooms = rooms_occupancy(&state.employees, &state.guests);
            for (room_id, (re, rg)) in &rooms {
                let mut names = Vec::new();
                names.extend(re.iter().cloned());
                names.extend(rg.iter().cloned());
                // Spec says "room_id: sorted names" — combine employees then guests both sorted
                println!("{}: {}", room_id, names.join(","));
            }
        }

        Command::RoomHistory { person_type, name } => {
            match room_history(&log.entries, &person_type, &name) {
                None => invalid(),
                Some(rooms) => {
                    let s: Vec<String> = rooms.iter().map(|r| r.to_string()).collect();
                    println!("{}", s.join(","));
                }
            }
        }

        Command::Intersection { persons } => {
            let result = intersection_query(&log.entries, &persons);
            let names: Vec<String> = result.iter().map(|(_, n)| n.clone()).collect();
            println!("{}", names.join(","));
        }
    }
}