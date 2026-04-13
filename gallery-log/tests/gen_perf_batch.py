#!/usr/bin/env python3
"""Generate a valid logappend batch file for perf testing.

Usage: gen_perf_batch.py <N> <log_path> <batch_output_path>

Pattern (keeps the gallery state machine valid for any N >= 1):
  Line 1:  T=1  Alice arrives gallery
  Line 2:  T=2  Bob   arrives gallery
  Then cycles of 4 lines per room k (k cycles 1..=MAX_ROOMS):
    Alice enter room k
    Bob   enter room k   <- both in room k simultaneously (intersection)
    Alice leave room k
    Bob   leave room k
"""
import sys

MAX_ROOMS = 100


def main() -> None:
    if len(sys.argv) != 4:
        sys.stderr.write("usage: gen_perf_batch.py <N> <log_path> <batch_path>\n")
        sys.exit(2)
    n = int(sys.argv[1])
    log = sys.argv[2]
    out = sys.argv[3]

    lines: list[str] = []
    t = 1

    def add(cmd: str) -> None:
        nonlocal t
        if len(lines) < n:
            lines.append(f"-T {t} -K secret {cmd} {log}")
            t += 1

    add("-A -E Alice")
    add("-A -E Bob")

    k = 0
    while len(lines) < n:
        room = (k % MAX_ROOMS) + 1
        k += 1
        for op in (
            f"-A -E Alice -R {room}",
            f"-A -E Bob -R {room}",
            f"-L -E Alice -R {room}",
            f"-L -E Bob -R {room}",
        ):
            add(op)
            if len(lines) >= n:
                break

    with open(out, "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
