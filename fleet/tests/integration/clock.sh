#!/usr/bin/env bash
set -u
source "$(dirname "$0")/common.sh"
require_fleet || exit 3
TMP="$(new_tmp)"
trap 'rm -rf "$TMP"' EXIT
export FLEET_STATE="$TMP/state"
mkdir -p "$FLEET_STATE"
make_repo "$TMP/repo" clock
echo '== integration: receipt clock =='
run_capture "$TMP/good.out" "$FLEET" run --task clock-control --repo "$TMP/repo" --agent stub
assert_rc 0 "$RUN_RC" 'clock control run completes'
ledger_dump_to "$TMP/good-ledger.jsonl"
if [ "$RUN_RC" -eq 0 ] && python3 - "$TMP/good-ledger.jsonl" <<'PY'
import datetime, json, sys
now = datetime.datetime.now(datetime.timezone.utc)
for line in open(sys.argv[1]):
    if not line.strip():
        continue
    ts = datetime.datetime.fromisoformat(json.loads(line)["ts_wall"].replace("Z", "+00:00"))
    if ts > now + datetime.timedelta(seconds=1):
        raise SystemExit(1)
raise SystemExit(0)
PY
then
  ok 'no written receipt has a future timestamp'
else
  no 'no written receipt has a future timestamp'
fi
cp "$TMP/good-ledger.jsonl" "$TMP/tampered-ledger.jsonl"
python3 - "$TMP/tampered-ledger.jsonl" <<'PY'
import json, sys
path = sys.argv[1]
rows = [json.loads(line) for line in open(path) if line.strip()]
rows[-1]["ts_wall"] = "2999-01-01T00:00:00Z"
with open(path, "w") as handle:
    for row in rows:
        handle.write(json.dumps(row, separators=(",", ":")) + "\n")
PY
cp "$TMP/tampered-ledger.jsonl" "$FLEET_STATE/ledger/chain.jsonl"
run_capture "$TMP/bad.out" "$FLEET" ledger verify
assert_rc 8 "$RUN_RC" 'future-timestamp tamper is a verification mismatch'
cp "$TMP/good-ledger.jsonl" "$FLEET_STATE/ledger/chain.jsonl"
ledger_verify "$TMP/restored.out"
assert_rc 0 "$RUN_RC" 'restoring the untampered chain verifies'
finish
