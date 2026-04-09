//! Integration tests for gallery-log.
//!
//! These tests call the compiled binaries (logappend / logread) as child
//! processes, exactly as a real user would, and assert on stdout / stderr /
//! exit-code.  Run with:
//!
//!   cargo test
//!
//! The binaries must already be compiled:
//!   cargo build          (debug, used by default)
//!   cargo build --release (set env GALLERY_RELEASE=1 to use release builds)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bin(name: &str) -> PathBuf {
    // Allow CI / release builds via env var
    let profile = if std::env::var("GALLERY_RELEASE").is_ok() { "release" } else { "debug" };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(profile)
        .join(name)
}

fn logappend(args: &[&str]) -> Output {
    Command::new(bin("logappend")).args(args).output().expect("logappend binary not found — run `cargo build` first")
}

fn logread(args: &[&str]) -> Output {
    Command::new(bin("logread")).args(args).output().expect("logread binary not found — run `cargo build` first")
}

fn stdout(o: &Output) -> String { String::from_utf8_lossy(&o.stdout).into_owned() }
fn stderr(o: &Output) -> String { String::from_utf8_lossy(&o.stderr).into_owned() }

/// Create a unique temp log path for each test so tests don't interfere.
fn tmp_log(name: &str) -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test_logs");
    fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{}.log", name)).to_string_lossy().into_owned()
}

fn cleanup(path: &str) { let _ = fs::remove_file(path); }

// ── 1. Basic append & state ───────────────────────────────────────────────────

#[test]
fn test_01_employee_arrives_and_state() {
    let log = tmp_log("t01");
    cleanup(&log);

    let o = logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    assert!(o.status.success(), "append should succeed");

    let o = logread(&["-K", "secret", "-S", &log]);
    assert!(o.status.success());
    let out = stdout(&o);
    assert!(out.contains("Alice"), "Alice should appear in state:\n{}", out);
}

#[test]
fn test_02_guest_arrives_and_state() {
    let log = tmp_log("t02");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-G", "Bob", &log]);

    let o = logread(&["-K", "secret", "-S", &log]);
    assert!(o.status.success());
    let out = stdout(&o);
    // line 2 is guests
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2);
    assert!(lines[1].contains("Bob"), "Bob should be on guests line:\n{}", out);
}

#[test]
fn test_03_employee_enters_and_leaves_room() {
    let log = tmp_log("t03");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);

    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    assert!(out.contains("1:"), "room 1 should appear:\n{}", out);
    assert!(out.contains("Alice"), "Alice should be in room 1:\n{}", out);

    // Alice leaves room
    logappend(&["-T", "3", "-K", "secret", "-L", "-E", "Alice", "-R", "1", &log]);
    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    assert!(!out.contains("1:"), "room 1 should be gone after Alice leaves:\n{}", out);
}

#[test]
fn test_04_employee_leaves_gallery() {
    let log = tmp_log("t04");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-L", "-E", "Alice", &log]);

    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    // employees line should be empty
    let first_line = out.lines().next().unwrap_or("");
    assert!(first_line.trim().is_empty(), "employee line should be empty:\n{}", out);
}

// ── 2. Multiple people ────────────────────────────────────────────────────────

#[test]
fn test_05_multiple_people_sorted() {
    let log = tmp_log("t05");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Charlie", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "3", "-K", "secret", "-A", "-G", "Zara", &log]);
    logappend(&["-T", "4", "-K", "secret", "-A", "-G", "Bob", &log]);

    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    let lines: Vec<&str> = out.lines().collect();

    // Employees line: Alice,Charlie (sorted)
    assert_eq!(lines[0], "Alice,Charlie", "employees should be sorted:\n{}", out);
    // Guests line: Bob,Zara (sorted)
    assert_eq!(lines[1], "Bob,Zara", "guests should be sorted:\n{}", out);
}

#[test]
fn test_06_multiple_rooms() {
    let log = tmp_log("t06");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-G", "Bob", &log]);
    logappend(&["-T", "3", "-K", "secret", "-A", "-E", "Alice", "-R", "2", &log]);
    logappend(&["-T", "4", "-K", "secret", "-A", "-G", "Bob", "-R", "5", &log]);

    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    assert!(out.contains("2:"), "room 2 should appear:\n{}", out);
    assert!(out.contains("5:"), "room 5 should appear:\n{}", out);
}

// ── 3. Room history ───────────────────────────────────────────────────────────

#[test]
fn test_07_room_history_employee() {
    let log = tmp_log("t07");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", "-R", "3", &log]);
    logappend(&["-T", "3", "-K", "secret", "-L", "-E", "Alice", "-R", "3", &log]);
    logappend(&["-T", "4", "-K", "secret", "-A", "-E", "Alice", "-R", "7", &log]);

    let o = logread(&["-K", "secret", "-R", "-E", "Alice", &log]);
    assert!(o.status.success());
    let out = stdout(&o).trim().to_string();
    assert_eq!(out, "3,7", "room history should be 3,7:\n{}", out);
}

#[test]
fn test_08_room_history_guest_with_revisit() {
    let log = tmp_log("t08");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-G", "Bob", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-G", "Bob", "-R", "1", &log]);
    logappend(&["-T", "3", "-K", "secret", "-L", "-G", "Bob", "-R", "1", &log]);
    logappend(&["-T", "4", "-K", "secret", "-A", "-G", "Bob", "-R", "2", &log]);
    logappend(&["-T", "5", "-K", "secret", "-L", "-G", "Bob", "-R", "2", &log]);
    logappend(&["-T", "6", "-K", "secret", "-A", "-G", "Bob", "-R", "1", &log]); // revisit room 1

    let o = logread(&["-K", "secret", "-R", "-G", "Bob", &log]);
    let out = stdout(&o).trim().to_string();
    // Room 1 visited first, then 2, then room 1 again — revisits ARE recorded
    assert_eq!(out, "1,2,1", "room history should be 1,2,1 (revisits included):\n{}", out);
}

#[test]
fn test_09_room_history_never_entered_room() {
    let log = tmp_log("t09");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    // Alice never enters a room

    let o = logread(&["-K", "secret", "-R", "-E", "Alice", &log]);
    assert!(o.status.success());
    let out = stdout(&o).trim().to_string();
    assert!(out.is_empty(), "room history should be empty for gallery-only person:\n'{}'", out);
}

#[test]
fn test_10_room_history_unknown_person() {
    let log = tmp_log("t10");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);

    // Bob never appeared
    let o = logread(&["-K", "secret", "-R", "-E", "Bob", &log]);
    assert_eq!(o.status.code(), Some(111), "unknown person should exit 111");
    assert!(stderr(&o).contains("invalid"));
}

// ── 4. Intersection ───────────────────────────────────────────────────────────

#[test]
fn test_11_intersection_basic() {
    let log = tmp_log("t11");
    cleanup(&log);

    // Alice and Bob both in room 1 at the same time; Carol only in room 2
    logappend(&["-T", "1",  "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2",  "-K", "secret", "-A", "-G", "Bob",   &log]);
    logappend(&["-T", "3",  "-K", "secret", "-A", "-G", "Carol", &log]);
    logappend(&["-T", "4",  "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    logappend(&["-T", "5",  "-K", "secret", "-A", "-G", "Bob",   "-R", "1", &log]);
    logappend(&["-T", "6",  "-K", "secret", "-A", "-G", "Carol", "-R", "2", &log]);
    logappend(&["-T", "7",  "-K", "secret", "-L", "-G", "Bob",   "-R", "1", &log]);
    logappend(&["-T", "8",  "-K", "secret", "-L", "-E", "Alice", "-R", "1", &log]);

    // Which rooms did Alice and Bob share at the same time?
    // Alice and Bob were both in room 1 concurrently → output should be "1"
    let o = logread(&["-K", "secret", "-I", "-E", "Alice", "-G", "Bob", &log]);
    assert!(o.status.success());
    let out = stdout(&o).trim().to_string();
    assert_eq!(out, "1", "shared room should be room 1:\n{}", out);
}

#[test]
fn test_12_intersection_no_overlap() {
    let log = tmp_log("t12");
    cleanup(&log);

    // Alice in room 1, Bob in room 2 — never share a room
    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-G", "Bob",   &log]);
    logappend(&["-T", "3", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    logappend(&["-T", "4", "-K", "secret", "-A", "-G", "Bob",   "-R", "2", &log]);

    let o = logread(&["-K", "secret", "-I", "-E", "Alice", "-G", "Bob", &log]);
    let out = stdout(&o).trim().to_string();
    assert!(out.is_empty(), "intersection should be empty:\n'{}'", out);
}

// ── 5. Validation — illegal state transitions ─────────────────────────────────

#[test]
fn test_13_cannot_enter_gallery_twice() {
    let log = tmp_log("t13");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    let o = logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_14_cannot_enter_room_without_entering_gallery() {
    let log = tmp_log("t14");
    cleanup(&log);

    // Alice never entered gallery
    let o = logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_15_cannot_be_in_two_rooms() {
    let log = tmp_log("t15");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    let o = logappend(&["-T", "3", "-K", "secret", "-A", "-E", "Alice", "-R", "2", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_16_cannot_leave_gallery_while_in_room() {
    let log = tmp_log("t16");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    let o = logappend(&["-T", "3", "-K", "secret", "-L", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_17_cannot_leave_room_not_entered() {
    let log = tmp_log("t17");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    let o = logappend(&["-T", "2", "-K", "secret", "-L", "-E", "Alice", "-R", "5", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_18_timestamp_must_increase() {
    let log = tmp_log("t18");
    cleanup(&log);

    logappend(&["-T", "10", "-K", "secret", "-A", "-E", "Alice", &log]);
    let o = logappend(&["-T", "10", "-K", "secret", "-A", "-G", "Bob", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));

    let o = logappend(&["-T", "5", "-K", "secret", "-A", "-G", "Bob", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_19_cannot_leave_gallery_never_entered() {
    let log = tmp_log("t19");
    cleanup(&log);

    let o = logappend(&["-T", "1", "-K", "secret", "-L", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

// ── 6. Security — wrong token & tampering ─────────────────────────────────────

#[test]
fn test_20_wrong_token_on_read() {
    let log = tmp_log("t20");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "correct", "-A", "-E", "Alice", &log]);

    let o = logread(&["-K", "wrong", "-S", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("integrity violation"));
}

#[test]
fn test_21_wrong_token_on_append() {
    let log = tmp_log("t21");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "correct", "-A", "-E", "Alice", &log]);

    let o = logappend(&["-T", "2", "-K", "wrong", "-A", "-G", "Bob", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("integrity violation"));
}

#[test]
fn test_22_tampered_log_detected() {
    let log = tmp_log("t22");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);

    // Flip some bytes in the middle of the file
    let mut bytes = fs::read(&log).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    fs::write(&log, &bytes).unwrap();

    let o = logread(&["-K", "secret", "-S", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("integrity violation"));
}

#[test]
fn test_23_truncated_log_detected() {
    let log = tmp_log("t23");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);

    // Truncate the file
    let bytes = fs::read(&log).unwrap();
    fs::write(&log, &bytes[..bytes.len() / 2]).unwrap();

    let o = logread(&["-K", "secret", "-S", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("integrity violation"));
}

// ── 7. Input validation ───────────────────────────────────────────────────────

#[test]
fn test_24_invalid_name_with_numbers() {
    let log = tmp_log("t24");
    cleanup(&log);

    let o = logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice123", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_25_invalid_token_with_symbols() {
    let log = tmp_log("t25");
    cleanup(&log);

    let o = logappend(&["-T", "1", "-K", "bad token!", "-A", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_26_timestamp_zero_rejected() {
    let log = tmp_log("t26");
    cleanup(&log);

    let o = logappend(&["-T", "0", "-K", "secret", "-A", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_27_missing_action_flag() {
    let log = tmp_log("t27");
    cleanup(&log);

    // No -A or -L
    let o = logappend(&["-T", "1", "-K", "secret", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_28_conflicting_action_flags() {
    let log = tmp_log("t28");
    cleanup(&log);

    let o = logappend(&["-T", "1", "-K", "secret", "-A", "-L", "-E", "Alice", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

#[test]
fn test_29_both_employee_and_guest_flags() {
    let log = tmp_log("t29");
    cleanup(&log);

    let o = logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", "-G", "Bob", &log]);
    assert_eq!(o.status.code(), Some(111));
    assert!(stderr(&o).contains("invalid"));
}

// ── 8. Batch mode ─────────────────────────────────────────────────────────────

#[test]
fn test_30_batch_mode_basic() {
    let log = tmp_log("t30");
    cleanup(&log);

    let batch_path = tmp_log("t30_batch");
    let batch_content = format!(
        "-T 1 -K secret -A -E Alice {log}\n\
         -T 2 -K secret -A -G Bob {log}\n\
         -T 3 -K secret -A -E Alice -R 1 {log}\n",
        log = log
    );
    fs::write(&batch_path, batch_content).unwrap();

    let o = logappend(&["-B", &batch_path]);
    assert!(o.status.success(), "batch should succeed:\n{}", stderr(&o));

    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    assert!(out.contains("Alice"));
    assert!(out.contains("Bob"));
    assert!(out.contains("1:"));

    cleanup(&batch_path);
}

#[test]
fn test_31_batch_invalid_line_continues() {
    let log = tmp_log("t31");
    cleanup(&log);

    let batch_path = tmp_log("t31_batch");
    // Line 2 is invalid (duplicate arrival), but line 3 should still run
    let batch_content = format!(
        "-T 1 -K secret -A -E Alice {log}\n\
         -T 2 -K secret -A -E Alice {log}\n\
         -T 3 -K secret -A -G Bob {log}\n",
        log = log
    );
    fs::write(&batch_path, batch_content).unwrap();

    // Batch exits 0 even if some lines fail
    logappend(&["-B", &batch_path]);

    // Bob (line 3) should have been added despite line 2 failing
    let o = logread(&["-K", "secret", "-S", &log]);
    let out = stdout(&o);
    assert!(out.contains("Bob"), "Bob from valid line should appear:\n{}", out);

    cleanup(&batch_path);
}

// ── 9. Log file creation ──────────────────────────────────────────────────────

#[test]
fn test_32_log_created_if_not_exists() {
    let log = tmp_log("t32");
    cleanup(&log);

    assert!(!Path::new(&log).exists(), "log should not exist yet");
    let o = logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    assert!(o.status.success());
    assert!(Path::new(&log).exists(), "log should now exist");
}

#[test]
fn test_33_empty_log_gives_empty_state() {
    let log = tmp_log("t33");
    cleanup(&log);

    // Never appended anything — logread -S on non-existent log
    let o = logread(&["-K", "secret", "-S", &log]);
    assert!(o.status.success());
    let out = stdout(&o);
    // Both employee and guest lines should be empty
    let lines: Vec<&str> = out.lines().collect();
    // At minimum two empty lines
    assert!(lines.len() >= 2 || out.trim().is_empty());
}

// ── 10. Full scenario ─────────────────────────────────────────────────────────

#[test]
fn test_34_full_scenario() {
    let log = tmp_log("t34");
    cleanup(&log);

    // A realistic gallery session
    logappend(&["-T", "1",  "-K", "tok", "-A", "-E", "Alice",   &log]);
    logappend(&["-T", "2",  "-K", "tok", "-A", "-G", "Bob",     &log]);
    logappend(&["-T", "3",  "-K", "tok", "-A", "-G", "Carol",   &log]);
    logappend(&["-T", "4",  "-K", "tok", "-A", "-E", "Alice",   "-R", "1", &log]);
    logappend(&["-T", "5",  "-K", "tok", "-A", "-G", "Bob",     "-R", "1", &log]);
    logappend(&["-T", "6",  "-K", "tok", "-A", "-G", "Carol",   "-R", "2", &log]);
    logappend(&["-T", "7",  "-K", "tok", "-L", "-G", "Bob",     "-R", "1", &log]);
    logappend(&["-T", "8",  "-K", "tok", "-A", "-G", "Bob",     "-R", "3", &log]);
    logappend(&["-T", "9",  "-K", "tok", "-L", "-E", "Alice",   "-R", "1", &log]);
    logappend(&["-T", "10", "-K", "tok", "-L", "-E", "Alice",   &log]);

    // State check
    let o = logread(&["-K", "tok", "-S", &log]);
    assert!(o.status.success());
    let out = stdout(&o);
    assert!(!out.contains("Alice"), "Alice left gallery:\n{}", out);
    assert!(out.contains("Bob"),    "Bob still in gallery:\n{}", out);
    assert!(out.contains("Carol"),  "Carol still in gallery:\n{}", out);
    assert!(out.contains("3:"),     "room 3 should have Bob:\n{}", out);
    assert!(out.contains("2:"),     "room 2 should have Carol:\n{}", out);

    // Room history
    let o = logread(&["-K", "tok", "-R", "-E", "Alice", &log]);
    assert_eq!(stdout(&o).trim(), "1", "Alice visited room 1 only");

    let o = logread(&["-K", "tok", "-R", "-G", "Bob", &log]);
    assert_eq!(stdout(&o).trim(), "1,3", "Bob visited rooms 1 then 3");

    // Intersection: which rooms did Alice and Bob share at the same time?
    let o = logread(&["-K", "tok", "-I", "-E", "Alice", "-G", "Bob", &log]);
    let out = stdout(&o).trim().to_string();
    assert_eq!(out, "1", "Alice and Bob shared room 1:\n{}", out);
}

// ── 11. Intersection — additional room-ID output tests ────────────────────────

#[test]
fn test_35_intersection_multiple_shared_rooms() {
    let log = tmp_log("t35");
    cleanup(&log);

    // Alice and Bob share room 1, then later both share room 3
    logappend(&["-T", "1",  "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2",  "-K", "secret", "-A", "-G", "Bob",   &log]);
    // Both in room 1
    logappend(&["-T", "3",  "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    logappend(&["-T", "4",  "-K", "secret", "-A", "-G", "Bob",   "-R", "1", &log]);
    logappend(&["-T", "5",  "-K", "secret", "-L", "-E", "Alice", "-R", "1", &log]);
    logappend(&["-T", "6",  "-K", "secret", "-L", "-G", "Bob",   "-R", "1", &log]);
    // Both in room 3
    logappend(&["-T", "7",  "-K", "secret", "-A", "-E", "Alice", "-R", "3", &log]);
    logappend(&["-T", "8",  "-K", "secret", "-A", "-G", "Bob",   "-R", "3", &log]);
    logappend(&["-T", "9",  "-K", "secret", "-L", "-E", "Alice", "-R", "3", &log]);
    logappend(&["-T", "10", "-K", "secret", "-L", "-G", "Bob",   "-R", "3", &log]);

    let o = logread(&["-K", "secret", "-I", "-E", "Alice", "-G", "Bob", &log]);
    assert!(o.status.success());
    // Should output rooms in ascending numerical order: 1,3
    assert_eq!(stdout(&o).trim(), "1,3", "shared rooms should be 1,3");
}

#[test]
fn test_36_intersection_unknown_person_ignored() {
    let log = tmp_log("t36");
    cleanup(&log);

    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);

    // Bob never appears — spec says unknown persons are ignored.
    // With only Alice known, we get rooms Alice was in.
    let o = logread(&["-K", "secret", "-I", "-E", "Alice", "-E", "Bob", &log]);
    assert!(o.status.success());
    // Bob unknown → ignored → only Alice's rooms count → room 1
    assert_eq!(stdout(&o).trim(), "1");
}

#[test]
fn test_37_intersection_no_concurrent_overlap() {
    let log = tmp_log("t37");
    cleanup(&log);

    // Alice in room 1 first, leaves, then Bob enters room 1 — never concurrent
    logappend(&["-T", "1", "-K", "secret", "-A", "-E", "Alice", &log]);
    logappend(&["-T", "2", "-K", "secret", "-A", "-G", "Bob",   &log]);
    logappend(&["-T", "3", "-K", "secret", "-A", "-E", "Alice", "-R", "1", &log]);
    logappend(&["-T", "4", "-K", "secret", "-L", "-E", "Alice", "-R", "1", &log]);
    logappend(&["-T", "5", "-K", "secret", "-A", "-G", "Bob",   "-R", "1", &log]);

    let o = logread(&["-K", "secret", "-I", "-E", "Alice", "-G", "Bob", &log]);
    let out = stdout(&o).trim().to_string();
    assert!(out.is_empty(), "no concurrent overlap — should be empty: '{}'", out);
}