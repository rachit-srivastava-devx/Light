#!/usr/bin/env bash

# status.sh — B14: `fleet status --json` on a genuinely empty FLEET_STATE.
#
# BACKLOG.md's B14: a fresh/empty store printed `{ "checked": 0, "total": 0, ... }` and exited 0 --
# indistinguishable from "everything was checked and is fine." This project's own law, enforced in
# every corpus detector, is "measuring nothing is a failure, not a pass" -- but forcing exit 6 on
# every brand-new `fleet` install (a real, common, legitimate state) would be more surprising than
# helpful.
#
# DECISION (see docs/delta.d/B14.md for the full writeup): keep exit 0 for a genuinely empty store,
# but the JSON MUST say so unambiguously -- an explicit `"empty": true` field that cannot be
# confused with "20/20 checked, all fine", and the human-readable render must say "empty store" in
# words, not just print zero counts. This suite drives the REAL binary against a REAL empty
# FLEET_STATE (a proxy is not the property -- reading status.rs is not enough).
set -u
if [ -n "${GIT_DIR:-}" ] || [ -n "${GIT_INDEX_FILE:-}" ] || [ -n "${GIT_WORK_TREE:-}" ]; then
  echo "FATAL: refusing to run with an inherited git context (D23)." >&2; exit 3
fi
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLEET="${FLEET_BIN:-$ROOT/keel/target/debug/fleet}"
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
no(){ FAIL=$((FAIL+1)); printf '  FAIL %s   %s\n' "$1" "${2:-}"; }
[ -x "$FLEET" ] || { echo "  FATAL: $FLEET not built"; exit 3; }

echo "== status (B14 cold-start) acceptance =="

# A genuinely empty store: mktemp'd dir, never touched by any `fleet` command before this.
export FLEET_STATE="$(mktemp -d "${TMPDIR:-/tmp}/fleet-status-empty.XXXXXX")"
trap 'rm -rf "$FLEET_STATE"' EXIT

# --- A. JSON mode ------------------------------------------------------------
JOUT="$("$FLEET" status --json 2>&1)"; JRC=$?
[ $JRC -eq 0 ] && ok "A1 status --json on empty store exits 0 (cold start is legitimate, not a fault)" \
  || no "A1 status --json on empty store exits 0" "rc=$JRC out=$(printf '%s' "$JOUT" | head -5)"

python3 - "$JOUT" <<'PY' && ok "A2 JSON publishes checked==total==0 AND an explicit empty=true marker" \
  || no "A2 JSON publishes an unambiguous empty marker"
import json, sys
doc = json.loads(sys.argv[1])
assert doc["checked"] == 0, doc.get("checked")
assert doc["total"] == 0, doc.get("total")
assert doc["empty"] is True, doc.get("empty")
# every per-state group must ALSO carry the zero denominator explicitly -- a group silently
# omitted would look identical to "no failing tasks" rather than "no tasks measured at all".
assert len(doc["groups"]) > 0, "no groups published for an empty store"
for g in doc["groups"]:
    assert g["checked"] == 0 and g["total"] == 0, g
PY

# `empty:true` must not be spoofable by a store that genuinely has zero CURRENT tasks but a
# non-empty ledger (e.g. every run already reconciled away) -- that is a DIFFERENT state from
# "never touched", and this suite only asserts the never-touched case above. Confirm the ledger
# file itself was never created by a bare `status` call, i.e. we really did measure nothing:
[ ! -e "$FLEET_STATE/ledger/chain.jsonl" ] && ok "A3 status is read-only; empty store stays empty" \
  || no "A3 status is read-only; empty store stays empty" "status created a ledger on a bare read"

# --- B. human mode must say "empty" IN WORDS, not just print zero counts -----
HOUT="$("$FLEET" status 2>&1)"; HRC=$?
[ $HRC -eq 0 ] && ok "B1 human status on empty store exits 0" || no "B1 human status on empty store exits 0" "rc=$HRC"
case "$HOUT" in
  *empty*) ok "B2 human status says 'empty' in words" ;;
  *) no "B2 human status says 'empty' in words" "$(printf '%s' "$HOUT" | head -3)" ;;
esac
case "$HOUT" in
  *"0 of 0"*) ok "B3 human status shows the 0-of-0 denominator" ;;
  *) no "B3 human status shows the 0-of-0 denominator" "$(printf '%s' "$HOUT" | head -3)" ;;
esac

# --- C. contrast: a store with a real derivable task must NOT set empty=true ------------
# A bare `note`/gate_verdict receipt attaches to no task by itself (there is no run yet), so it
# would not flip `empty` -- that is not a loophole in the contract, it correctly still means
# "nothing to measure". Use a real `run_start` receipt, which derive_tasks always turns into a
# task regardless of how it later resolves.
"$FLEET" ledger append --event run_start --body '{"task":"probe","agent":"stub","run_id":"probe-1"}' >/dev/null 2>&1
COUT="$("$FLEET" status --json 2>&1)"
python3 - "$COUT" <<'PY' && ok "C1 a non-empty ledger is never reported as empty=true" \
  || no "C1 a non-empty ledger is never reported as empty=true"
import json, sys
doc = json.loads(sys.argv[1])
assert doc["empty"] is False, doc.get("empty")
PY

echo "== $PASS passed, $FAIL failed =="
[ "$FAIL" -eq 0 ]
