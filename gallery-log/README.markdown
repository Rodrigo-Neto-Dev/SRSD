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
  demo.sh
  reset.sh
  makefile
  README.md
```

---

## Option A — Run locally (Rust installed) -> Not Windows!

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
---

## Option B — Run with Docker (no local Rust required works with windows)

The Dockerfile uses a **multi-stage build**:
1. **Stage 1** (`rust:latest`) — compiles the project
2. **Stage 2** (`debian:bookworm-slim`) — copies only the final binaries

You do **not** need Rust installed. Docker handles everything.

The `logs/` folder in your project is automatically mounted into the container, so every log file written inside the container is saved to your machine and persists across runs.

### 1. Open a terminal and navigate to the project folder


```bash
# 1. Build everything
make build

# 2. Run tests
make test

# 3. Try the demo
make demo

# 4. Clean up
make reset
```

### Available Commands

| Command                             | Description |
|-------------------------------------|-------------|
| `make build`                        | Build all Docker images |
| `make build-runtime`                | Build only runtime image (faster) |
| `make test`                         | Run all tests in Docker |
| `make test-specific TEST=test_name` | Run a specific test |
| `make demo`                         | Run interactive demo |
| `make reset`                        | Clear all log files |
| `make shell`                        | Open bash shell in container |
| `make dev`                          | Open dev shell with Rust/cargo |
| `make clean`                        | Remove logs and containers |
| `make clean-all`                    | Remove everything including images |
| `make rebuild`                      | Clean everything and rebuild |
| `local-build`                       | Build locally with cargo|
| `local-test`                        | Run tests locally|
| `local-demo`                        | Run demo locally|
| `local-clean`                       | Clean local build artifacts|

---
## Recommended actions:
Use only docker and the non-local make commands
Use make demo to take a look at the demo
Use the make dev to have acess to everything (use ls for context if needed)
Running commands in that dev shell will be similar to running them locally (see above 3. Run)


### Running Specific Tests

```bash
# Run a single test
make test-specific TEST=test_07_room_history

# Run tests matching a pattern
make test-specific TEST=test_1

# Run tests with output
docker-compose run --rm gallery-test-specific cargo test test_07_room_history -- --nocapture
```

### Direct Docker Commands (without make)

```bash
# Build
docker-compose build

# Run all tests
docker-compose run --rm gallery-test

# Run specific test
docker-compose run --rm gallery-test-specific cargo test test_07_room_history

# Run demo
docker-compose run --rm gallery-demo

# Interactive shell
docker-compose run --rm gallery-log /bin/bash

# Reset logs
docker-compose run --rm gallery-reset
```

### Manual Testing Examples

```bash
# Create a test scenario
docker-compose run --rm gallery-log ./logappend -T 1 -K secret -A -E Alice logs/test.log
docker-compose run --rm gallery-log ./logappend -T 2 -K secret -A -E Alice -R 1 logs/test.log
docker-compose run --rm gallery-log ./logread -K secret -S logs/test.log

# Expected output:
# Alice
# 
# 1: Alice
```

### Windows PowerShell

```powershell
# Build
docker-compose build

# Run tests
docker-compose run --rm gallery-test

# Run demo
docker-compose run --rm gallery-demo

# Using make (if installed via Chocolatey)
make build
make test
make demo
```

### Troubleshooting

**Tests fail with "binary not found"**
```bash
# Ensure target directory is mounted
docker-compose run --rm -v ${PWD}/target:/app/target gallery-test
```

**Permission denied on logs/**
```bash
# Reset permissions (Linux/Mac)
sudo chown -R $USER:$USER logs/
```

**Clean rebuild**
```bash
make rebuild
``` 

---
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
-T 1 -K secret -A -E Alice logs/log1
-T 2 -K secret -A -G Bob logs/log1
-T 3 -K secret -A -E Alice -R 1 logs/log1
```

Run it:

```bash
# Locally
./target/release/logappend -B logs/batch.txt

# Docker (while running)
logappend -B logs/batch.txt
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

Shows rooms where all mentioned people have been at the same time.

```bash
./logread -K secret -I -E Alice -G Bob logs/log1
```


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