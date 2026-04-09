# Gallery Log System

A secure art-gallery audit log implemented in Rust.

Log files are stored in the `logs/` folder inside the project directory. The programs create log files automatically if they do not exist — you never need to create them manually.

---

## Project structure

```
gallery-log/
  src/
    lib.rs
    bin/
      logappend.rs
      logread.rs
  logs/              ← log files are written here (persists on your machine)
  Cargo.toml
  Dockerfile
  docker-compose.yml
  README.md
```

---

## Option A — Run locally (Rust installed)

### 1. Navigate to the project folder

```bash
cd path/to/gallery-log
```

### 2. Build

```bash
cargo build --release
```

Binaries will be created at:
- `target/release/logappend`
- `target/release/logread`

### 3. Run

All log file paths should point to the `logs/` folder:

```bash
# Employee Alice arrives at gallery
./target/release/logappend -T 1 -K secret -A -E Alice logs/log1

# Alice enters room 1
./target/release/logappend -T 2 -K secret -A -E Alice -R 1 logs/log1

# Guest Bob arrives at gallery
./target/release/logappend -T 3 -K secret -A -G Bob logs/log1

# Read current state
./target/release/logread -K secret -S logs/log1
```

On Windows use backslashes and the `.exe` extension:

```powershell
.\target\release\logappend.exe -T 1 -K secret -A -E Alice logs\log1
.\target\release\logread.exe -K secret -S logs\log1
```

---

## Option B — Run with Docker (no local Rust required)

The Dockerfile uses a **multi-stage build**:
1. **Stage 1** (`rust:latest`) — compiles the project
2. **Stage 2** (`debian:bookworm-slim`) — copies only the final binaries

You do **not** need Rust installed. Docker handles everything.

The `logs/` folder in your project is automatically mounted into the container, so every log file written inside the container is saved to your machine and persists across runs.

### 1. Open a terminal and navigate to the project folder

```bash
cd path/to/gallery-log
```

> **Windows (PowerShell):**
> ```powershell
> cd C:\Users\yourname\path\to\gallery-log
> ```

This step is important — all Docker commands must be run from this folder so that Docker can find the `Dockerfile` and mount the `logs/` folder correctly.

### 2. Build the Docker image

```bash
docker build -t gallery-log .
```

The `.` at the end means "use the current folder as the build context". Run this once, or again whenever you change the source code.

### 3. Run the programs

#### Using `docker run` directly

```bash
# Employee Alice arrives at gallery
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log \
  ./logappend -T 1 -K secret -A -E Alice logs/log1

# Alice enters room 1
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log \
  ./logappend -T 2 -K secret -A -E Alice -R 1 logs/log1

# Guest Bob arrives at gallery
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log \
  ./logappend -T 3 -K secret -A -G Bob logs/log1

# Alice leaves room 1
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log \
  ./logappend -T 4 -K secret -L -E Alice -R 1 logs/log1

# Alice leaves gallery
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log \
  ./logappend -T 5 -K secret -L -E Alice logs/log1

# Read current state
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log \
  ./logread -K secret -S logs/log1
```

> **Windows (PowerShell):** replace `${PWD}` with `${PWD}` — PowerShell supports this natively. If it does not work, use the full path:
> ```powershell
> docker run --rm -v "C:\Users\yourname\path\to\gallery-log\logs:/app/logs" gallery-log ./logappend -T 1 -K secret -A -E Alice logs/log1
> ```

#### Using docker-compose (simpler for repeated use)

```bash
# Start an interactive container session
docker-compose run --rm gallery-log

# Then inside the container you can run commands directly:
./logappend -T 1 -K secret -A -E Alice logs/log1
./logappend -T 2 -K secret -A -E Alice -R 1 logs/log1
./logread -K secret -S logs/log1

# Type exit when done
exit
```

The `logs/` mount is configured automatically by `docker-compose.yml` — no extra flags needed.

---

## Batch mode

Create a batch file, for example `logs/batch.txt`:

```
logappend -T 1 -K secret -A -E Alice logs/log1
logappend -T 2 -K secret -A -G Bob logs/log1
logappend -T 3 -K secret -A -E Alice -R 1 logs/log1
```

Run it:

```bash
# Locally
./target/release/logappend -B logs/batch.txt

# Docker
docker run --rm -v "${PWD}/logs:/app/logs" gallery-log ./logappend -B logs/batch.txt
```

---

## logread query modes

### Current state (`-S`)

Shows who is currently in the gallery and which rooms they are in.

```bash
./logread -K secret -S logs/log1
```

Output format:
```
Alice,Charlie        ← employees currently in gallery (sorted, comma-separated)
Bob                  ← guests currently in gallery
1: Alice,Bob         ← room 1 occupants
3: Charlie           ← room 3 occupants
```

### Room history (`-R`)

Shows which rooms a person has visited, in order of first visit.

```bash
./logread -K secret -R -E Alice logs/log1
./logread -K secret -R -G Bob logs/log1
```

### Intersection (`-I`)

Shows all people who were ever in the same room at the same time as all listed persons.

```bash
./logread -K secret -I -E Alice -G Bob logs/log1
```

---

## Security design

| Mechanism | Implementation |
|-----------|---------------|
| Key derivation | `K = SHA-256(token)` |
| Encryption | Stream cipher: `keystream = SHA-256(K ∥ counter)`, `ct = pt ⊕ ks` |
| Authentication | `MAC = HMAC-SHA-256(K, ciphertext)` per record |
| Integrity chain | `prev_hash = SHA-256(prev_ciphertext)` embedded in every plaintext entry |

### On-disk record format

```
[u32 BE: ciphertext length][ciphertext bytes][32-byte HMAC-SHA-256]
```

Each decrypted plaintext entry:
```
timestamp|E/G|name|A/L|room_or_empty|prev_hash_hex
```

---

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 111 | Invalid input or integrity violation |

Stderr output:
- `invalid` — bad arguments or illegal state transition
- `integrity violation` — MAC failure or broken hash chain

---

## Running tests

The test suite calls the compiled binaries as child processes and covers all functionality end-to-end.

### Build first (tests require binaries to exist)

```bash
cargo build
```

### Run all tests

```bash
cargo test
```

### Run a specific test by name

```bash
cargo test test_07_room_history
```

### Run with release binaries

```bash
cargo build --release
GALLERY_RELEASE=1 cargo test
```

### What is tested

| # | Test | What it checks |
|---|------|----------------|
| 01–04 | Basic arrivals / departures | Employee/guest enter and leave gallery and state reflects it |
| 05–06 | Multiple people & rooms | Sorted output, multiple rooms visible in state |
| 07–09 | Room history | Ordered list of distinct rooms, no duplicates, empty when no rooms entered |
| 10 | Unknown person history | Exit 111 for a person who never appeared |
| 11–12 | Intersection | Correct overlap detection, empty when no shared room |
| 13–19 | Illegal transitions | All invalid state changes rejected with exit 111 |
| 20–23 | Security | Wrong token, tampered bytes, truncated file → integrity violation |
| 24–29 | Input validation | Invalid names, tokens, timestamps, conflicting flags |
| 30–31 | Batch mode | Successful batch, bad lines skipped gracefully |
| 32–33 | File creation | Log created on first write, empty log gives empty state |
| 34 | Full scenario | Complete realistic session covering all features |

---

## Interactive demo

The `demo.sh` script walks through every feature interactively with coloured output and pause points between sections.

### Run locally

```bash
# Build first
cargo build

# Run demo
./demo.sh
```

### Run with Docker

```bash
# Build image first
docker build -t gallery-log .

# Run demo
./demo.sh --docker
```

### What the demo covers

1. Basic employee and guest arrivals
2. Entering and leaving rooms
3. Current state query (`-S`)
4. Room history query (`-R`)
5. Intersection query (`-I`)
6. All illegal transitions — shown rejected one by one
7. Security — wrong token and byte-flipped tampered log
8. Batch mode with one intentionally invalid line