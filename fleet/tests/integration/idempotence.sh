#!/usr/bin/env bash
set -u
source "$(dirname "$0")/common.sh"
require_fleet || exit 3
TMP="$(new_tmp)"
trap 'rm -rf "$TMP"' EXIT
export FLEET_STATE="$TMP/state"
mkdir -p "$FLEET_STATE"
make_repo "$TMP/repo" idempotence
echo '== integration: idempotence =='
mkdir -p "$TMP/bad-repo"
( cd "$TMP/bad-repo" && git init -q && printf 'tracked\n' > README && git add -A && git -c user.email=fleet@test -c user.name=fleet commit -qm init )
run_capture "$TMP/bad.out" "$FLEET" run --task same-task --repo "$TMP/bad-repo" --agent stub
assert_rc 6 "$RUN_RC" 'bad idempotence fixture is typed as invariant failure'
run_capture "$TMP/first.out" "$FLEET" run --task same-task --repo "$TMP/repo" --agent stub
FIRST_RC=$RUN_RC
run_capture "$TMP/second.out" "$FLEET" run --task same-task --repo "$TMP/repo" --agent stub
SECOND_RC=$RUN_RC
assert_rc 0 "$FIRST_RC" 'first identical task run succeeds'
assert_rc 0 "$SECOND_RC" 'second identical task run succeeds'
FIRST_ID="$(artifact_id_from "$TMP/first.out")"
SECOND_ID="$(artifact_id_from "$TMP/second.out")"
if [ -n "$FIRST_ID" ] && [ -n "$SECOND_ID" ] && [ "$FIRST_ID" != "$SECOND_ID" ]; then
  ok 'identical tasks produce distinct artifact ids'
else
  no 'identical tasks produce distinct artifact ids' "first=$FIRST_ID second=$SECOND_ID"
fi
ledger_verify "$TMP/verify.out"
assert_rc 0 "$RUN_RC" 'idempotent runs leave the ledger valid'
if [ -f "$FLEET_STATE/artifacts/$FIRST_ID" ] && [ -f "$FLEET_STATE/artifacts/$SECOND_ID" ]; then
  ok 'both idempotent artifacts remain addressable'
else
  no 'both idempotent artifacts remain addressable'
fi
assert_no_agent_processes "$TMP/ps.out"
finish
