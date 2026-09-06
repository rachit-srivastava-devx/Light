#!/usr/bin/env bash
# F02: three mirrors, one corpus, one comparison. Exit 6 on any divergence (fleet's invariant code).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"          # -> fleet/
LIGHT="$(cd "$ROOT/.." && pwd)"                       # -> Light/
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT

# 1. Each mirror emits its own report. Statuses captured directly -- never $? after a pipe (E1/S11).
( cd "$LIGHT/orb" && F02_REPORT_OUT="$OUT/ts.json" npx vitest run apps/mobile/src/build/lld-v1.test.ts ) \
  >"$OUT/ts.log" 2>&1; ts_rc=$?
( cd "$LIGHT/orb/backend/relay-py" && F02_REPORT_OUT="$OUT/py.json" .venv/bin/pytest -q tests/test_lld_schema_conformance.py ) \
  >"$OUT/py.log" 2>&1; py_rc=$?
( cd "$ROOT/keel" && F02_REPORT_OUT="$OUT/rs.json" cargo test --quiet --test f02_lld_crosslang ) \
  >"$OUT/rs.log" 2>&1; rs_rc=$?

for pair in "ts:$ts_rc" "py:$py_rc" "rs:$rs_rc"; do
  case "$pair" in *:0) ;; *) echo "MIRROR FAILED: ${pair%%:*} (see $OUT/${pair%%:*}.log)"; exit 6 ;; esac
done

# 2. MEASURING NOTHING IS A FAILURE. A missing/empty report must not read as agreement.
for m in ts py rs; do
  [ -s "$OUT/$m.json" ] || { echo "NO REPORT from $m -- three mirrors must all report"; exit 6; }
done
n=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["fixtures"]))' "$OUT/ts.json")
[ "$n" -ge 6 ] || { echo "corpus shrank to $n fixtures (expected >= 6)"; exit 6; }

# 3. The comparison. Reports are byte-identical except the "mirror" key.
for m in py rs; do
  if ! diff -u <(sed 's/"mirror": "ts"/"mirror": "X"/' "$OUT/ts.json") \
               <(sed "s/\"mirror\": \"$m\"/\"mirror\": \"X\"/" "$OUT/$m.json"); then
    echo "MIRROR DIVERGENCE: ts vs $m -- $n fixtures compared"; exit 6
  fi
done
echo "ok  3 mirrors agree on $n fixtures"
