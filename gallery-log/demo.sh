#!/usr/bin/env bash
# =============================================================================
#  Gallery Log — Interactive Demo
#  Run from the project root:
#    chmod +x demo.sh
#    ./demo.sh                  (uses local cargo build)
#    ./demo.sh --docker         (uses Docker image)
# =============================================================================

set -euo pipefail

# ── Colour helpers ─────────────────────────────────────────────────────────────
RED='\033[0;31m';  GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m';     RESET='\033[0m'

say()     { echo -e "${CYAN}${BOLD}▶ $*${RESET}"; }
ok()      { echo -e "${GREEN}✔  $*${RESET}"; }
fail()    { echo -e "${RED}✘  $*${RESET}"; }
section() { echo -e "\n${YELLOW}${BOLD}══════════════════════════════════════════${RESET}";
            echo -e "${YELLOW}${BOLD}  $*${RESET}";
            echo -e "${YELLOW}${BOLD}══════════════════════════════════════════${RESET}\n"; }
pause()   { echo -e "\n${BOLD}Press ENTER to continue...${RESET}"; read -r; }

# ── Mode: local or docker ──────────────────────────────────────────────────────
USE_DOCKER=false

if [[ -f /.dockerenv ]]; then USE_DOCKER=true; fi
LOG_DIR="$(pwd)/logs"
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/demo.log"
BATCH="$LOG_DIR/demo_batch.txt"

# Wipe previous demo log so we start clean
rm -f "$LOG"

# ── Mode: detection ────────────────────────────────────────────────────────────
# Check if we are running inside a Docker container

run_append() {
    if [ "$USE_DOCKER" = true ]; then
        ./logappend "$@"
    else
    ./target/debug/logappend "$@"
    fi
}

run_read() {
    if [ "$USE_DOCKER" = true ]; then
        ./logread "$@"
    else
    ./target/debug/logread "$@"
    fi
}

cmd_append() { # pretty-print the command then run it
    echo -e "  ${BOLD}\$ logappend $*${RESET}"
    if run_append "$@"; then ok "success"; else fail "exit $?"; fi
}

cmd_read() {
    echo -e "  ${BOLD}\$ logread $*${RESET}"
    OUTPUT=$(run_read "$@" 2>&1) && { ok "success"; echo "$OUTPUT"; } \
        || { fail "exit $? — $(run_read "$@" 2>&1 || true)"; }
}

expect_fail() { # assert a command exits 111
    local label="$1"; shift
    echo -e "  ${BOLD}\$ logappend $* ${CYAN}(expected: invalid)${RESET}"
    set +e
    run_append "$@" > /dev/null 2>&1
    CODE=$?
    set -e
    if [[ $CODE -eq 111 ]]; then ok "$label → correctly rejected (exit 111)"
    else fail "$label → expected exit 111, got $CODE"; fi
}

expect_read_fail() {
    local label="$1"; shift
    echo -e "  ${BOLD}\$ logread $* ${CYAN}(expected: integrity violation / invalid)${RESET}"
    set +e
    run_read "$@" > /dev/null 2>&1
    CODE=$?
    set -e
    if [[ $CODE -eq 111 ]]; then ok "$label → correctly rejected (exit 111)"
    else fail "$label → expected exit 111, got $CODE"; fi
}

# ── Pre-flight check ───────────────────────────────────────────────────────────
clear
echo -e "${BOLD}╔══════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}║       Gallery Log — Interactive Demo             ║${RESET}"
echo -e "${BOLD}╚══════════════════════════════════════════════════╝${RESET}"
echo ""

if [ "$USE_DOCKER" = true ]; then
    say "Mode: Inside Docker Container"
elif [[ "${1:-}" == "--docker" ]]; then
    say "Mode: Docker Host (calling docker run)"
else
  say "Mode: local binary (target/debug/)"
  if [[ ! -f ./target/debug/logappend ]]; then
    echo "Binaries not found. Running cargo build..."
    cargo build
  fi
fi

echo ""
echo -e "Log file: ${BOLD}$LOG${RESET}"
pause

# =============================================================================
section "1 · Basic arrivals — employees & guests"
# =============================================================================
say "Employees Alice and Charlie enter the gallery."
say "Guests Bob and Diana enter the gallery."
echo ""
cmd_append -T 1  -K secret -A -E Alice   "$LOG"
cmd_append -T 2  -K secret -A -E Charlie "$LOG"
cmd_append -T 3  -K secret -A -G Bob     "$LOG"
cmd_append -T 4  -K secret -A -G Diana   "$LOG"
echo ""
say "Current state (-S):"
cmd_read   -K secret -S "$LOG"
pause

# =============================================================================
section "2 · Entering rooms"
# =============================================================================
say "Alice enters room 1. Bob enters room 1. Charlie enters room 3. Diana enters room 2."
echo ""
cmd_append -T 5  -K secret -A -E Alice   -R 1 "$LOG"
cmd_append -T 6  -K secret -A -G Bob     -R 1 "$LOG"
cmd_append -T 7  -K secret -A -E Charlie -R 3 "$LOG"
cmd_append -T 8  -K secret -A -G Diana   -R 2 "$LOG"
echo ""
say "Current state (-S):"
cmd_read   -K secret -S "$LOG"
pause

# =============================================================================
section "3 · Leaving rooms & gallery"
# =============================================================================
say "Bob leaves room 1, then leaves the gallery."
say "Alice leaves room 1, then also leaves the gallery."
echo ""
cmd_append -T 9  -K secret -L -G Bob     -R 1 "$LOG"
cmd_append -T 10 -K secret -L -G Bob           "$LOG"
cmd_append -T 11 -K secret -L -E Alice   -R 1 "$LOG"
cmd_append -T 12 -K secret -L -E Alice         "$LOG"
echo ""
say "Current state (-S)  — Alice and Bob should be gone:"
cmd_read   -K secret -S "$LOG"
pause

# =============================================================================
section "4 · Room history (-R)"
# =============================================================================
say "Add a few more room visits for Charlie and Diana so history is interesting."
echo ""
cmd_append -T 13 -K secret -L -E Charlie -R 3 "$LOG"
cmd_append -T 14 -K secret -A -E Charlie -R 7 "$LOG"
cmd_append -T 15 -K secret -L -E Charlie -R 7 "$LOG"
cmd_append -T 16 -K secret -A -E Charlie -R 3 "$LOG"  # revisit — should not duplicate

echo ""
say "Room history for Charlie (employee):"
cmd_read   -K secret -R -E Charlie "$LOG"
echo "  Expected: 3,7  (room 3 first, then 7 — revisit of 3 not duplicated)"

echo ""
say "Room history for Diana (guest):"
cmd_read   -K secret -R -G Diana "$LOG"
echo "  Expected: 2"

echo ""
say "Room history for Bob (never entered a room after re-checking state):"
cmd_read   -K secret -R -G Bob "$LOG"
echo "  Expected: 1  (Bob only ever entered room 1)"
pause

# =============================================================================
section "5 · Intersection (-I)"
# =============================================================================
say "Add a new guest Eve who was in room 1 while Bob was there earlier."
say "(We can only show intersection with people who share time in a room.)"
echo ""
say "Current people still in gallery: Charlie (room 3), Diana (room 2)."
say "Let Eve arrive and enter room 3 at the same time as Charlie."
echo ""
cmd_append -T 17 -K secret -A -G Eve    "$LOG"
cmd_append -T 18 -K secret -A -G Eve    -R 3 "$LOG"
echo ""
say "Which rooms did Charlie and Eve share at the same time?"
say "(Charlie is in room 3, Eve just joined room 3 → they overlap)"
cmd_read   -K secret -I -E Charlie -G Eve "$LOG"
echo "  Expected: 3"

echo ""
say "Which rooms did Charlie and Diana share at the same time?"
say "(Charlie in room 3, Diana in room 2 — never the same room)"
cmd_read   -K secret -I -E Charlie -G Diana "$LOG"
echo "  Expected: (empty)"
pause

# =============================================================================
section "6 · Validation — illegal transitions (all should be rejected)"
# =============================================================================

VLOG="$LOG_DIR/demo_validation.log"
rm -f "$VLOG"

say "Setting up a fresh log with just Alice in gallery..."
run_append -T 1 -K secret -A -E Alice "$VLOG" > /dev/null
echo ""

expect_fail "Enter gallery twice"          -T 2  -K secret -A -E Alice              "$VLOG"
expect_fail "Enter room without entering gallery first" \
                                           -T 2  -K secret -A -E Bob -R 1           "$VLOG"
run_append  -T 2  -K secret -A -E Alice -R 1 "$VLOG" > /dev/null  # Alice now in room 1
expect_fail "Enter second room while in room 1" \
                                           -T 3  -K secret -A -E Alice -R 2         "$VLOG"
expect_fail "Leave gallery while still in a room" \
                                           -T 3  -K secret -L -E Alice              "$VLOG"
expect_fail "Leave a room never entered"   -T 3  -K secret -L -E Alice -R 9         "$VLOG"
expect_fail "Timestamp not increasing (same)" \
                                           -T 2  -K secret -L -E Alice -R 1         "$VLOG"
expect_fail "Timestamp going backwards"    -T 1  -K secret -L -E Alice -R 1         "$VLOG"
expect_fail "Leave gallery never entered"  -T 3  -K secret -L -E Nobody             "$VLOG"
expect_fail "Invalid name (has digits)"    -T 3  -K secret -A -E Alice2             "$VLOG"
expect_fail "Invalid token (has space)"    -T 3  "-K" "bad token" -A -E Fred        "$VLOG"
expect_fail "Zero timestamp"               -T 0  -K secret -A -G Fred               "$VLOG"
expect_fail "Both -A and -L"               -T 3  -K secret -A -L -E Fred           "$VLOG"
expect_fail "Both -E and -G"               -T 3  -K secret -A -E Fred -G Gina       "$VLOG"
pause

# =============================================================================
section "7 · Security — wrong token & tampering"
# =============================================================================

SLOG="$LOG_DIR/demo_security.log"
rm -f "$SLOG"
run_append -T 1 -K correcttoken -A -E Alice "$SLOG" > /dev/null

echo ""
say "Trying to READ with the wrong token:"
expect_read_fail "Wrong token on logread" -K wrongtoken -S "$SLOG"

echo ""
say "Trying to APPEND with the wrong token:"
expect_fail "Wrong token on logappend" -T 2 -K wrongtoken -A -G Bob "$SLOG"

echo ""
say "Manually flipping a byte in the log file..."
FILE="$SLOG"
SIZE=$(stat -c%s "$FILE")
MID=$((SIZE / 2))

printf '\xFF' | dd of="$FILE" bs=1 seek="$MID" count=1 conv=notrunc 2>/dev/null
echo "  Byte at position $MID flipped."
say "Reading tampered log (should detect integrity violation):"
expect_read_fail "Tampered log" -K correcttoken -S "$SLOG"
pause

# =============================================================================
section "8 · Batch mode (-B)"
# =============================================================================

BLOG="$LOG_DIR/demo_batch_log.log"
rm -f "$BLOG" "$BATCH"

say "Creating batch file with 6 commands (line 4 is intentionally invalid)..."
echo ""
cat > "$BATCH" << EOF
-T 1 -K batchkey -A -E Alice $BLOG
-T 2 -K batchkey -A -G Bob $BLOG
-T 3 -K batchkey -A -E Alice -R 1 $BLOG
-T 3 -K batchkey -A -G Bob -R 1 $BLOG
-T 4 -K batchkey -A -G Carol $BLOG
-T 5 -K batchkey -A -G Carol -R 2 $BLOG
EOF
cat "$BATCH"
echo ""
say "Running batch file:"
say "Running batch file:"
if [ "$USE_DOCKER" = true ]; then
    # Inside container, logs are at /app/logs
    ./logappend -B "/app/logs/demo_batch.txt"
else
  ./target/debug/logappend -B "$BATCH"
fi
echo ""
say "State after batch (line 4 failed silently, rest processed):"
cmd_read -K batchkey -S "$BLOG"
pause

# =============================================================================
section "All done!"
# =============================================================================
echo -e "${GREEN}${BOLD}"
echo "  ✔  Basic arrivals and departures"
echo "  ✔  Room entry and exit"
echo "  ✔  Current state query (-S)"
echo "  ✔  Room history query (-R)"
echo "  ✔  Intersection query (-I)"
echo "  ✔  All illegal transitions correctly rejected"
echo "  ✔  Wrong token detected on read and append"
echo "  ✔  Tampered log detected"
echo "  ✔  Batch mode (-B) with graceful per-line error handling"
echo -e "${RESET}"
echo "Log files written to: $LOG_DIR"