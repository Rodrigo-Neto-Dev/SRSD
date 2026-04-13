//! Performance test: single-line logappend vs batch-mode (-B) logappend.
//!
//! Each configuration appends N entries, then runs three read queries
//! against the resulting log:
//!   -S                    current state
//!   -R -E Alice           room history for Alice
//!   -I -E Alice -E Bob    rooms Alice and Bob shared simultaneously
//!
//! Uses the RELEASE binaries at target/release/{logappend,logread}.
//! Build first with:
//!     cargo build --release
//!
//! Then run the suite (it is #[ignore]d so regular `cargo test` skips it):
//!     cargo test --test perf_test -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join(name)
}

fn require_bin(name: &str) {
    let p = bin(name);
    assert!(
        p.exists(),
        "missing {} — run `cargo build --release` first",
        p.display()
    );
}

fn workdir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("perf_logs");
    fs::create_dir_all(&d).unwrap();
    d
}

fn time_ms<F: FnOnce() -> Output>(f: F) -> (f64, Output) {
    let t0 = Instant::now();
    let o = f();
    (t0.elapsed().as_secs_f64() * 1000.0, o)
}

fn gen_batch(n: usize, log: &str, batch: &str) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("gen_perf_batch.py");
    let o = Command::new("python3")
        .arg(&script)
        .arg(n.to_string())
        .arg(log)
        .arg(batch)
        .output()
        .expect("python3 not found");
    assert!(
        o.status.success(),
        "gen_perf_batch.py failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn median(xs: &mut [f64]) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[xs.len() / 2]
}

// ── Benchmark core ────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Mode {
    Single,
    Batch,
}

struct Sample {
    append_ms: f64,
    read_s_ms: f64,
    read_r_ms: f64,
    read_i_ms: f64,
}

fn one_run(mode: Mode, n: usize, log: &str, batch: &str) -> Sample {
    let _ = fs::remove_file(log);

    let (append_ms, o) = match mode {
        Mode::Single => time_ms(|| {
            Command::new(bin("logappend"))
                .args(["-T", "1", "-K", "secret", "-A", "-E", "Alice", log])
                .output()
                .unwrap()
        }),
        Mode::Batch => time_ms(|| {
            Command::new(bin("logappend"))
                .args(["-B", batch])
                .output()
                .unwrap()
        }),
    };
    assert!(
        o.status.success(),
        "logappend failed (n={}): {}",
        n,
        String::from_utf8_lossy(&o.stderr)
    );

    let (read_s_ms, o) = time_ms(|| {
        Command::new(bin("logread"))
            .args(["-K", "secret", "-S", log])
            .output()
            .unwrap()
    });
    assert!(
        o.status.success(),
        "logread -S failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let (read_r_ms, o) = time_ms(|| {
        Command::new(bin("logread"))
            .args(["-K", "secret", "-R", "-E", "Alice", log])
            .output()
            .unwrap()
    });
    assert!(
        o.status.success(),
        "logread -R Alice failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let (read_i_ms, o) = time_ms(|| {
        Command::new(bin("logread"))
            .args(["-K", "secret", "-I", "-E", "Alice", "-E", "Bob", log])
            .output()
            .unwrap()
    });
    assert!(
        o.status.success(),
        "logread -I Alice Bob failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    Sample {
        append_ms,
        read_s_ms,
        read_r_ms,
        read_i_ms,
    }
}

struct PerfRow {
    mode: &'static str,
    n: usize,
    append: f64,
    read_s: f64,
    read_r: f64,
    read_i: f64,
}

fn bench(mode: Mode, n: usize, runs: usize) -> PerfRow {
    let tag = match mode {
        Mode::Single => "single",
        Mode::Batch => "batch",
    };
    let log = workdir()
        .join(format!("perf_{}_{}.log", tag, n))
        .to_string_lossy()
        .into_owned();
    let batch = workdir()
        .join(format!("perf_{}_{}.txt", tag, n))
        .to_string_lossy()
        .into_owned();
    if matches!(mode, Mode::Batch) {
        gen_batch(n, &log, &batch);
    }

    let mut a = Vec::with_capacity(runs);
    let mut s = Vec::with_capacity(runs);
    let mut r = Vec::with_capacity(runs);
    let mut i = Vec::with_capacity(runs);
    for _ in 0..runs {
        let x = one_run(mode, n, &log, &batch);
        a.push(x.append_ms);
        s.push(x.read_s_ms);
        r.push(x.read_r_ms);
        i.push(x.read_i_ms);
    }
    PerfRow {
        mode: tag,
        n,
        append: median(&mut a),
        read_s: median(&mut s),
        read_r: median(&mut r),
        read_i: median(&mut i),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn perf_suite() {
    require_bin("logappend");
    require_bin("logread");

    // (mode, N, runs)
    let configs: &[(Mode, usize, usize)] = &[
        (Mode::Single, 1, 5),
        (Mode::Batch, 5, 5),
        (Mode::Batch, 50, 5),
        (Mode::Batch, 500, 5),
        (Mode::Batch, 5000, 3),
        (Mode::Batch, 50000, 3),
    ];

    let mut results = Vec::new();
    for &(m, n, runs) in configs {
        results.push(bench(m, n, runs));
    }

    println!();
    println!("Gallery-log perf suite (medians, release binary)");
    println!(
        "┌────────┬───────┬─────────────┬────────────┬────────────┬────────────┬────────────┐"
    );
    println!(
        "│ mode   │     N │ append (ms) │ per-line ms│  read-S ms │  read-R ms │  read-I ms │"
    );
    println!(
        "├────────┼───────┼─────────────┼────────────┼────────────┼────────────┼────────────┤"
    );
    for r in &results {
        println!(
            "│ {:6} │ {:5} │ {:11.2} │ {:10.4} │ {:10.2} │ {:10.2} │ {:10.2} │",
            r.mode,
            r.n,
            r.append,
            r.append / r.n as f64,
            r.read_s,
            r.read_r,
            r.read_i
        );
    }
    println!(
        "└────────┴───────┴─────────────┴────────────┴────────────┴────────────┴────────────┘"
    );
}
