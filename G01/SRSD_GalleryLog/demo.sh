#!/usr/bin/env bash
# =============================================================================
#  Gallery Log — Java Interactive Demo (Full Suite)
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

# ── Paths & JAR Configuration ──────────────────────────────────────────────────
LOG_DIR="./logs"
mkdir -p "$LOG_DIR"

LOG="$LOG_DIR/demo.log"
BATCH="$LOG_DIR/demo_batch.txt"
VLOG="$LOG_DIR/demo_validation.log"
SLOG="$LOG_DIR/demo_security.log"
BLOG="$LOG_DIR/demo_batch_log.log"

# Wipe previous demo logs
rm -f "$LOG" "$VLOG" "$SLOG" "$BLOG" "$BATCH"

APPEND_JAR="target/secure-gallery-1.0-SNAPSHOT-logappend.jar"
READ_JAR="target/secure-gallery-1.0-SNAPSHOT-logread.jar"

# ── Execution Helpers ──────────────────────────────────────────────────────────
run_append() { java -jar "$APPEND_JAR" "$@"; }
run_read()   { java -jar "$READ_JAR" "$@"; }

cmd_append() {
    echo -e "  ${BOLD}\$ logappend $*${RESET}"
    if run_append "$@"; then ok "success"; else fail "exit $?"; fi
}

cmd_read() {
    echo -e "  ${BOLD}\$ logread $*${RESET}"
    # Capture output for display
    OUTPUT=$(run_read "$@" 2>&1) && { ok "success"; echo "$OUTPUT"; } \
        || { fail "exit $? — $OUTPUT"; }
}

expect_fail() {
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
    echo -e "  ${BOLD}\$ logread $* ${CYAN}(expected: failure)${RESET}"
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
echo -e "${BOLD}║      Gallery Log — Java Interactive Demo         ║${RESET}"
echo -e "${BOLD}╚══════════════════════════════════════════════════╝${RESET}"

if [[ ! -f "$APPEND_JAR" ]]; then
    say "JARs not found. Running maven build..."
    mvn package
fi

say "Log file: $LOG"
pause

# =============================================================================
section "1 · Basic arrivals — employees & guests"
# =============================================================================
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
cmd_append -T 9  -K secret -L -G Bob     -R 1 "$LOG"
cmd_append -T 10 -K secret -L -G Bob          "$LOG"
cmd_append -T 11 -K secret -L -E Alice   -R 1 "$LOG"
cmd_append -T 12 -K secret -L -E Alice          "$LOG"
echo ""
say "Current state (-S) — Alice and Bob should be gone:"
cmd_read   -K secret -S "$LOG"
pause

# =============================================================================
section "4 · Room history (-R)"
# =============================================================================
cmd_append -T 13 -K secret -L -E Charlie -R 3 "$LOG"
cmd_append -T 14 -K secret -A -E Charlie -R 7 "$LOG"
cmd_append -T 15 -K secret -L -E Charlie -R 7 "$LOG"
cmd_append -T 16 -K secret -A -E Charlie -R 3 "$LOG"

echo ""
say "Room history for Charlie (employee):"
cmd_read   -K secret -R -E Charlie "$LOG"
echo "  Expected: 3,7"

echo ""
say "Room history for Diana (guest):"
cmd_read   -K secret -R -G Diana "$LOG"
echo "  Expected: 2"

echo ""
say "Room history for Bob (guest):"
cmd_read   -K secret -R -G Bob "$LOG"
echo "  Expected: 1"
pause

# =============================================================================
section "6 · Validation — illegal transitions (all rejected)"
# =============================================================================
say "Setting up validation log with Alice in gallery..."
run_append -T 1 -K secret -A -E Alice "$VLOG" > /dev/null

expect_fail "Enter gallery twice"          -T 2  -K secret -A -E Alice              "$VLOG"
expect_fail "Enter room without gallery"   -T 2  -K secret -A -E Bob -R 1           "$VLOG"

run_append  -T 2  -K secret -A -E Alice -R 1 "$VLOG" > /dev/null
expect_fail "Enter room 2 while in room 1" -T 3  -K secret -A -E Alice -R 2         "$VLOG"
expect_fail "Leave gallery while in room"  -T 3  -K secret -L -E Alice              "$VLOG"
expect_fail "Leave a room never entered"   -T 3  -K secret -L -E Alice -R 9         "$VLOG"
expect_fail "Timestamp not increasing"     -T 2  -K secret -L -E Alice -R 1         "$VLOG"
expect_fail "Timestamp going backwards"    -T 1  -K secret -L -E Alice -R 1         "$VLOG"
expect_fail "Invalid name (digits)"        -T 3  -K secret -A -E Alice2             "$VLOG"
expect_fail "Zero timestamp"               -T 0  -K secret -A -G Fred               "$VLOG"
expect_fail "Both -A and -L"               -T 3  -K secret -A -L -E Fred            "$VLOG"
expect_fail "Both -E and -G"               -T 3  -K secret -A -E Fred -G Gina       "$VLOG"
pause

# =============================================================================
section "7 · Security — wrong token & tampering"
# =============================================================================
run_append -T 1 -K correcttoken -A -E Alice "$SLOG" > /dev/null

say "Trying to READ with the wrong token:"
expect_read_fail "Wrong token logread" -K wrongtoken -S "$SLOG"

say "Trying to APPEND with the wrong token:"
expect_fail "Wrong token logappend" -T 2 -K wrongtoken -A -G Bob "$SLOG"

echo ""
say "Manually flipping a byte in the log file..."
SIZE=$(stat -c%s "$SLOG")
MID=$((SIZE / 2))
printf '\xFF' | dd of="$SLOG" bs=1 seek="$MID" count=1 conv=notrunc 2>/dev/null

say "Reading tampered log (integrity check):"
expect_read_fail "Tampered log" -K correcttoken -S "$SLOG"
pause

# =============================================================================
section "8 · Batch mode (-B)"
# =============================================================================
# Creating batch file (line 4 is intentionally invalid)
cat > "$BATCH" << EOF
-T 1 -K batchkey -A -E Alice $BLOG
-T 2 -K batchkey -A -G Bob $BLOG
-T 3 -K batchkey -A -E Alice -R 1 $BLOG
-T 3 -K batchkey -A -G Bob -R 1 $BLOG
-T 4 -K batchkey -A -G Carol $BLOG
EOF

say "Running batch file (Line 4 should fail, but script continues):"
run_append -B "$BATCH" || true

echo ""
say "State after batch:"
cmd_read -K batchkey -S "$BLOG"
pause

# =============================================================================
section "All done!"
# =============================================================================
echo -e "${GREEN}${BOLD}✔ All autonomous test cases processed.${RESET}"