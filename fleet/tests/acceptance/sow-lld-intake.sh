#!/usr/bin/env bash
# F07: the `fleet sow --lld <PATH>` acceptance drive, against the REAL compiled binary (not
# in-process -- see keel/fleet/tests/f07_sow_intake.rs for the 14-test contract suite that IS
# in-process/subprocess-per-test; this script is the single done-definition smoke sequence from
# the F07 lane contract §10.5: feed a gate-passed lld.v1 node in, get the same SOW_READY_AWAITING_
# REVIEW exit fleet's `--task` path already produces, and prove the round-trip is the SAME sow by
# id -- not just "some sow got created".
#
# Status captured directly, never `$?` after a pipe (verify.sh:20, E1/S11).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"          # -> fleet/
FLEET_BIN="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"
FIXTURE="$ROOT/contracts/fixtures/lld/complete_freeze.json"

if [ ! -x "$FLEET_BIN" ]; then
  echo "fleet binary not found at $FLEET_BIN -- build it first:"
  echo "  cargo build --manifest-path \"$ROOT/keel/Cargo.toml\""
  exit 3
fi
if [ ! -f "$FIXTURE" ]; then
  echo "fixture not found at $FIXTURE"
  exit 3
fi

STATE="$(mktemp -d)"
OUT1="$(mktemp)"; ERR1="$(mktemp)"; OUT2="$(mktemp)"; ERR2="$(mktemp)"; ACCEPT_OUT="$(mktemp)"
cleanup() { rm -rf "$STATE" "$OUT1" "$ERR1" "$OUT2" "$ERR2" "$ACCEPT_OUT"; }
trap cleanup EXIT

fail() { echo "FAIL  $1"; exit 6; }

# --- (1) fleet sow --lld <complete_freeze.json> -> exit 9 -----------------------------------
FLEET_STATE="$STATE" "$FLEET_BIN" sow --lld "$FIXTURE" >"$OUT1" 2>"$ERR1"
rc1=$?
[ "$rc1" -eq 9 ] || { cat "$ERR1"; fail "sow --lld exited $rc1, expected 9 (SOW_READY_AWAITING_REVIEW); stderr above"; }

id1="$(grep -o 'SOW_READY_AWAITING_REVIEW id=[0-9a-f]\{64\}' "$ERR1" | head -1 | cut -d= -f2)"
[ -n "$id1" ] || { cat "$ERR1"; fail "no SOW_READY_AWAITING_REVIEW id=<64 hex> line on stderr"; }
[ -f "$STATE/sows/$id1.json" ] || fail "state/sows/$id1.json was not written"

task_text="$(awk '/^SOW_LLD_TASK_BEGIN$/{flag=1; next} /^SOW_LLD_TASK_END$/{flag=0} flag' "$ERR1")"
[ -n "$task_text" ] || { cat "$ERR1"; fail "no SOW_LLD_TASK_BEGIN/END delimited text on stderr"; }

# --- (2) identity round-trip: fleet sow --task "<extracted text>" -> SAME id, exit 9 ---------
FLEET_STATE="$STATE" "$FLEET_BIN" sow --task "$task_text" >"$OUT2" 2>"$ERR2"
rc2=$?
[ "$rc2" -eq 9 ] || { cat "$ERR2"; fail "round-trip sow --task exited $rc2, expected 9 (not 8 EXIT_MISMATCH); stderr above"; }

id2="$(grep -o 'SOW_READY_AWAITING_REVIEW id=[0-9a-f]\{64\}' "$ERR2" | head -1 | cut -d= -f2)"
[ "$id2" = "$id1" ] || fail "round-trip id ($id2) != original id ($id1) -- the two intakes produced different SOWs"

record_count="$(find "$STATE/sows" -maxdepth 1 -name '*.json' | wc -l | tr -d ' ')"
[ "$record_count" -eq 1 ] || fail "expected exactly 1 record in state/sows/, found $record_count"

# --- (3) accept, then fleet run gets past the SOW gate ---------------------------------------
FLEET_STATE="$STATE" "$FLEET_BIN" sow accept --id "$id1" >"$ACCEPT_OUT" 2>&1
rc3=$?
accept_out="$(cat "$ACCEPT_OUT")"
[ "$rc3" -eq 0 ] || { echo "$accept_out"; fail "sow accept --id $id1 exited $rc3, expected 0"; }
case "$accept_out" in
  *"SOW_ACCEPTED id=$id1"*) : ;;
  *) fail "accept output did not confirm id=$id1: $accept_out" ;;
esac

# `fleet run` needs a real git repo target; use fleet's own checkout as --repo so the SOW gate
# (enforce_accepted_sow) is exercised for real, without asserting anything about what happens
# AFTER the gate (unrelated failures downstream are out of scope -- lane contract §10.5).
run_out="$(FLEET_STATE="$STATE" "$FLEET_BIN" run --task "$task_text" --repo "$ROOT" --agent stub 2>&1)"
run_rc=$?
if echo "$run_out" | grep -q "task has no accepted SOW\|SOW_NOT_ACCEPTED"; then
  fail "fleet run was blocked at the SOW gate (enforce_accepted_sow) despite acceptance; output: $run_out"
fi
echo "note: fleet run --task <compiled text> --repo fleet --agent stub exited $run_rc past the SOW gate (downstream outcome not asserted, lane contract §10.5)"

echo "ok  sow-lld-intake: --lld exit 9, round-trip same id ($id1), 1 record, accepted, run passed the SOW gate"
exit 0
