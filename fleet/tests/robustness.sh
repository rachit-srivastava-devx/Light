#!/usr/bin/env bash
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FLEET="$ROOT/keel/target/debug/fleet"
STATE="$(mktemp -d "${TMPDIR:-/tmp}/fleet-robustness.XXXXXX")"
TARGET="$(mktemp -d "${TMPDIR:-/tmp}/fleet-robustness-target.XXXXXX")"
cleanup() { rm -rf "$STATE" "$TARGET"; }
trap cleanup EXIT

export FLEET_STATE="$STATE"
( cd "$TARGET" && git init -q && printf 'fn main(){}\n' > main.rs && git add -A && git -c user.email=t@t -c user.name=t commit -qm init )

pass=0
fail=0
ok() { pass=$((pass + 1)); printf '  ok   %s\n' "$1"; }
bad() { fail=$((fail + 1)); printf '  FAIL %s\n' "$1"; }

echo "== robustness =="

set +e
FLEET_MAX_WORKERS=0 "$FLEET" run --task bad-cap --repo "$TARGET" --agent stub >/dev/null 2>&1
rc=$?
set -e
[ "$rc" -eq 3 ] && ok "BAD max-worker cap is rejected (rc=3)" || bad "BAD max-worker cap is rejected (rc=$rc)"

export FLEET_MAX_WORKERS=1
export FLEET_WORKER_DELAY_MS=5000
set +e
"$FLEET" run --task signal-me --repo "$TARGET" --agent stub >/dev/null 2>&1 &
run_pid=$!
set -e
children=""
for _ in $(seq 1 100); do
    children="$(pgrep -P "$run_pid" 2>/dev/null || true)"
    [ -n "$children" ] && break
    sleep 0.02
done
kill -TERM "$run_pid" 2>/dev/null
set +e
wait "$run_pid"
rc=$?
set -e
[ "$rc" -eq 130 ] && ok "GOOD SIGTERM exits 130" || bad "GOOD SIGTERM exits 130 (rc=$rc)"
sleep 0.2
orphan=0
for pid in $children; do
    if kill -0 "$pid" 2>/dev/null; then
        orphan=1
    fi
done
[ "$orphan" -eq 0 ] && ok "child process group has no surviving pid" || bad "child process group has no surviving pid"

unset FLEET_WORKER_DELAY_MS
"$FLEET" ledger verify >/dev/null 2>&1
rc=$?
[ "$rc" -eq 0 ] && ok "ledger verifies after signal" || bad "ledger verifies after signal (rc=$rc)"

"$FLEET" run --task lease-reuse --repo "$TARGET" --agent stub >/dev/null 2>&1
rc=$?
[ "$rc" -eq 0 ] && ok "worker lease is released for the next run" || bad "worker lease is released for the next run (rc=$rc)"

export FLEET_WORKER_DELAY_MS=300
start="$(date +%s)"
pids=""
for i in $(seq 1 6); do
    "$FLEET" run --task "queued-$i" --repo "$TARGET" --agent stub >/dev/null 2>&1 &
    pids="$pids $!"
done
for pid in $pids; do wait "$pid"; done
finish="$(date +%s)"
elapsed=$((finish - start))
[ "$elapsed" -ge 1 ] && ok "cap+5 dispatches queue at max=1 (elapsed=${elapsed}s)" || bad "cap+5 dispatches queue at max=1 (elapsed=${elapsed}s)"

unset FLEET_WORKER_DELAY_MS
"$FLEET" ledger verify >/dev/null 2>&1
rc=$?
[ "$rc" -eq 0 ] && ok "GOOD ledger control verifies" || bad "GOOD ledger control verifies (rc=$rc)"
cp "$STATE/ledger/chain.jsonl" "$STATE/ledger/chain.jsonl.good"
python3 - "$STATE/ledger/chain.jsonl" <<'PY'
import json
import sys
path = sys.argv[1]
with open(path, encoding="utf-8") as stream:
    rows = [json.loads(line) for line in stream if line.strip()]
rows[0]["hash"] = "blake3:" + ("0" * 64)
with open(path, "w", encoding="utf-8") as stream:
    for row in rows:
        stream.write(json.dumps(row, separators=(",", ":")) + "\n")
PY
set +e
"$FLEET" ledger verify >/dev/null 2>&1
rc=$?
set -e
[ "$rc" -eq 8 ] && ok "BAD tampered ledger is rejected (rc=8)" || bad "BAD tampered ledger is rejected (rc=$rc)"
mv "$STATE/ledger/chain.jsonl.good" "$STATE/ledger/chain.jsonl"

echo "-- $pass passed, $fail failed (denominator: $((pass + fail))) --"
[ "$fail" -eq 0 ]
