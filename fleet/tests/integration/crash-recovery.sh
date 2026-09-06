#!/usr/bin/env bash

set -u
source "$(dirname "$0")/common.sh"

require_fleet || exit 3
TMP="$(new_tmp)"
trap 'rm -rf "$TMP"' EXIT
export FLEET_STATE="$TMP/state"
mkdir -p "$FLEET_STATE"
make_repo "$TMP/repo" crash-recovery

echo '== integration: crash recovery =='

# GOOD control: an ordinary run proves the fixture and binary can complete.
run_capture "$TMP/good.out" "$FLEET" run --task control --repo "$TMP/repo" --agent stub
assert_rc 0 "$RUN_RC" 'good control run completes'

# BAD fixture: hold the real ledger lease, then SIGKILL the run while it waits.
LOCK="$FLEET_STATE/ledger/chain.lock"
READY="$TMP/holder.ready"
python3 - "$LOCK" "$READY" <<'PY' &
import fcntl, sys, time
with open(sys.argv[1], "a+") as handle:
    fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
    open(sys.argv[2], "w").close()
    time.sleep(30)
PY
HOLDER=$!

tries=0
while [ ! -f "$READY" ] && [ "$tries" -lt 50 ]; do
  sleep 0.1
  tries=$((tries + 1))
done
if [ ! -f "$READY" ]; then
  no 'bad crash fixture acquired the ledger lease'
else
  ok 'bad crash fixture acquired the ledger lease'
fi

run_capture "$TMP/killed.out" "$FLEET" run --task killed-mid-run --repo "$TMP/repo" --agent stub &
RUN_PID=$!
sleep 0.2
if kill -0 "$RUN_PID" 2>/dev/null; then
  kill -9 "$RUN_PID"
  wait "$RUN_PID"
  KILLED_RC=$?
  assert_rc 137 "$KILLED_RC" 'SIGKILL mid-run has expected shell status'
else
  no 'SIGKILL mid-run has expected shell status' 'run exited before the kill window'
fi

kill "$HOLDER" 2>/dev/null || true
wait "$HOLDER" 2>/dev/null || true

ledger_verify "$TMP/after-crash.verify"
assert_rc 0 "$RUN_RC" 'ledger verifies after SIGKILL'

run_capture "$TMP/recovery.out" "$FLEET" run --task recovery --repo "$TMP/repo" --agent stub
assert_rc 0 "$RUN_RC" 'subsequent run succeeds after lease release'
assert_no_agent_processes "$TMP/ps.out"
finish
