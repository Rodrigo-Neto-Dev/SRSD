#!/usr/bin/env bash
# =============================================================================
#  Gallery Log — Reset/Clean Script
#  Clears all log files from the logs directory
# =============================================================================

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RESET='\033[0m'

echo -e "${CYAN}Resetting Gallery Logs...${RESET}"

LOGS_DIR="${LOGS_DIR:-/app/logs}"
LOGS_DIR="${LOGS_DIR:-./logs}"

if [ ! -d "$LOGS_DIR" ]; then
    echo -e "${YELLOW}No logs directory found at $LOGS_DIR. Creating it...${RESET}"
    mkdir -p "$LOGS_DIR"
    echo -e "${GREEN}✓ Created logs directory${RESET}"
    exit 0
fi

COUNT=0
for file in "$LOGS_DIR"/*; do
    if [ -f "$file" ]; then
        rm -f "$file"
        echo -e "  Removed: $(basename "$file")"
        COUNT=$((COUNT + 1))
    fi
done

if [ $COUNT -eq 0 ]; then
    echo -e "${YELLOW}No log files to remove.${RESET}"
else
    echo -e "${GREEN}✓ Removed $COUNT log file(s)${RESET}"
fi

echo -e "${GREEN}Reset complete!${RESET}"