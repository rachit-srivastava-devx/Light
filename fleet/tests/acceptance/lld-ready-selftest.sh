#!/usr/bin/env bash
# F06: the gate's own guard (blueprint 03 §2.4 -- "a gate that passes its bad fixture is
# disabled at startup with a banner, not silently trusted"). Runs `fleet gate lld-ready
# --selftest`, which proves BOTH directions in one process: the bad fixture
# (one_line_freeze.json) is refused AND the control fixture (complete_module.json) is accepted
# (the S5 outage guard). Non-zero exit if EITHER fails -- including if it wrongly accepts the bad
# fixture. Status captured directly, never `$?` after a pipe (verify.sh:20, E1/S11).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"          # -> fleet/
FLEET_BIN="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"

if [ ! -x "$FLEET_BIN" ]; then
  echo "fleet binary not found at $FLEET_BIN -- build it first:"
  echo "  cargo build --manifest-path \"$ROOT/keel/Cargo.toml\""
  exit 3
fi

"$FLEET_BIN" gate lld-ready --selftest
rc=$?
if [ "$rc" -eq 0 ]; then
  echo "ok  lld-ready --selftest: bad fixture refused, control fixture accepted"
else
  echo "FAIL  lld-ready --selftest exited $rc -- see output above"
fi
exit "$rc"
