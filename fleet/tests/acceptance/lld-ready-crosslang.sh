#!/usr/bin/env bash
# F06: two mirrors (Rust/fleet, TS/orb via a small fleet-owned Node script -- no orb/ edit), one
# corpus (the same 6 lld fixtures F02 established), one comparison. Exit 6 on any divergence
# (fleet's invariant code), modeled line-for-line on fleet/tests/acceptance/lld-crosslang.sh.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"          # -> fleet/
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT

# 1. Each mirror emits its own report. Statuses captured directly -- never $? after a pipe (E1/S11).
( F06_REPORT_OUT="$OUT/ts.json" node "$ROOT/tests/acceptance/lld_ready_ts_report.mts" ) \
  >"$OUT/ts.log" 2>&1; ts_rc=$?
( cd "$ROOT/keel" && F06_REPORT_OUT="$OUT/rs.json" cargo test --quiet --test f06_lld_ready ) \
  >"$OUT/rs.log" 2>&1; rs_rc=$?

for pair in "ts:$ts_rc" "rs:$rs_rc"; do
  case "$pair" in *:0) ;; *) echo "MIRROR FAILED: ${pair%%:*} (see $OUT/${pair%%:*}.log)"; exit 6 ;; esac
done

# 2. MEASURING NOTHING IS A FAILURE. A missing/empty report must not read as agreement.
for m in ts rs; do
  [ -s "$OUT/$m.json" ] || { echo "NO REPORT from $m -- both mirrors must report"; exit 6; }
done
n=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["fixtures"]))' "$OUT/ts.json")
[ "$n" -ge 6 ] || { echo "corpus shrank to $n fixtures (expected >= 6)"; exit 6; }

# 3. The comparison. Reports are byte-identical except the "mirror" key.
if ! diff -u <(sed 's/"mirror": "ts"/"mirror": "X"/' "$OUT/ts.json") \
             <(sed 's/"mirror": "rs"/"mirror": "X"/' "$OUT/rs.json"); then
  echo "MIRROR DIVERGENCE: ts vs rs -- $n fixtures compared"; exit 6
fi
echo "ok  2 mirrors agree on $n fixtures"
