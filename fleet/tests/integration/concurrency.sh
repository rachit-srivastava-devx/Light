#!/usr/bin/env bash

set -u
source "$(dirname "$0")/common.sh"

require_fleet || exit 3
TMP="$(new_tmp)"
trap 'rm -rf "$TMP"' EXIT
export FLEET_STATE="$TMP/state"
mkdir -p "$FLEET_STATE"

echo '== integration: concurrent runs =='

# BAD fixture: the stub cannot operate on a repository without main.rs.
BAD="$TMP/bad-repo"
mkdir -p "$BAD"
( cd "$BAD" && git init -q && printf 'tracked\n' > README && git add -A && git -c user.email=fleet@test -c user.name=fleet commit -qm init )
run_capture "$TMP/bad.out" "$FLEET" run --task bad-control --repo "$BAD" --agent stub
assert_rc 6 "$RUN_RC" 'bad repository fixture is typed as invariant failure'

for i in 1 2 3; do
  make_repo "$TMP/repo-$i" "concurrency-$i"
done

PIDS=()
for i in 1 2 3; do
  run_capture "$TMP/run-$i.out" "$FLEET" run --task "concurrent-task-$i" --repo "$TMP/repo-$i" --agent stub &
  PIDS[$i]=$!
done

for i in 1 2 3; do
  wait "${PIDS[$i]}"
  rc=$?
  assert_rc 0 "$rc" "good concurrent run $i"
done

IDS=()
for i in 1 2 3; do
  aid="$(artifact_id_from "$TMP/run-$i.out")"
  IDS+=("$aid")
  if [ -n "$aid" ] && [ -f "$FLEET_STATE/artifacts/$aid" ]; then
    ok "run $i produced an artifact"
  else
    no "run $i produced an artifact" "id=$aid"
  fi
done

if python3 - "${IDS[@]}" <<'PY'
import sys
ids = sys.argv[1:]
raise SystemExit(0 if len(ids) == 3 and all(ids) and len(set(ids)) == 3 else 1)
PY
then
  ok 'three concurrent artifacts have distinct ids'
else
  no 'three concurrent artifacts have distinct ids'
fi

ledger_dump_to "$TMP/ledger.jsonl"
if [ "$RUN_RC" -eq 0 ] && python3 - "$TMP/ledger.jsonl" "$FLEET_STATE/artifacts" "$TMP/repo-1" "$TMP/repo-2" "$TMP/repo-3" <<'PY'
import json, pathlib, sys
rows = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
repos = set(sys.argv[3:])
starts = {r["body"].get("repo") for r in rows if r.get("event") == "run_start"}
ends = [r for r in rows if r.get("event") == "run_end" and r.get("body", {}).get("status") == "ok"]
artifacts = {r["body"].get("artifact_id") for r in rows if r.get("event") == "artifact_frozen"}
if not repos.issubset(starts) or len(ends) < 3 or not artifacts:
    raise SystemExit(1)
for artifact in artifacts:
    data = pathlib.Path(sys.argv[2], artifact).read_text()
    if not any(f"concurrency-{i}" in data for i in (1, 2, 3)):
        raise SystemExit(1)
raise SystemExit(0)
PY
then
  ok 'ledger attribution maps repos to landed artifacts'
else
  no 'ledger attribution maps repos to landed artifacts'
fi

ledger_verify "$TMP/verify.out"
assert_rc 0 "$RUN_RC" 'concurrent ledger chain verifies'
assert_no_agent_processes "$TMP/ps.out"
finish
