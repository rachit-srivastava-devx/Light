#!/usr/bin/env bash

# D23: REFUSE to run with an inherited git context. This suite does `git init` / `git add -A` /
# `git commit` inside throwaway repos. If GIT_DIR or GIT_INDEX_FILE point somewhere else, those
# writes land in THAT repo instead. That is not hypothetical: probing a hook-PATH theory with
# `env GIT_DIR=$PWD/.git bash tests/acceptance/p0.sh` wrote a bogus "init" commit into fleet-rs
# itself, deleting 218 files from HEAD (recovered via reflog). A test suite that can corrupt the
# repo under test must refuse, not warn.
if [ -n "${GIT_DIR:-}" ] || [ -n "${GIT_INDEX_FILE:-}" ] || [ -n "${GIT_WORK_TREE:-}" ]; then
  echo "FATAL: refusing to run with an inherited git context." >&2
  echo "  GIT_DIR=${GIT_DIR:-} GIT_INDEX_FILE=${GIT_INDEX_FILE:-} GIT_WORK_TREE=${GIT_WORK_TREE:-}" >&2
  echo "  This suite creates throwaway repos; those vars would redirect its writes into yours." >&2
  echo "  Re-run with:  env -u GIT_DIR -u GIT_INDEX_FILE -u GIT_WORK_TREE bash $0" >&2
  exit 3
fi
# P0 ACCEPTANCE SUITE — authored by the LEAD before implementation. Builders MUST NOT edit this file.
# Every assertion drives the REAL `fleet` binary end-to-end. No harness stand-ins (lesson S10:
# a suite passed 14/0 while the production defect was untouched, because it drove a synthetic harness).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLEET="${FLEET_BIN:-$ROOT/keel/target/debug/fleet}"
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
no(){ FAIL=$((FAIL+1)); printf '  FAIL %s   %s\n' "$1" "${2:-}"; }
have(){ [ -x "$FLEET" ]; }

# p0 tests the EVIDENCE KERNEL (freeze, ledger, attestation), not the planning gate. Reqs 6/7/14
# now make `run` refuse a task with no accepted SOW, which is correct and is asserted separately.
# Bypass it here so p0 keeps testing what it exists to test -- and the bypass is recorded in the
# receipt, so a run without a plan stays auditable rather than indistinguishable from one with.
export FLEET_SOW_BYPASS=1
export FLEET_STATE="$(mktemp -d "${TMPDIR:-/tmp}/fleet-p0.XXXXXX")"   # C1: mktemp, never a digest-derived path
trap 'rm -rf "$FLEET_STATE"' EXIT
TARGET="$(mktemp -d "${TMPDIR:-/tmp}/fleet-target.XXXXXX")"
trap 'rm -rf "$FLEET_STATE" "$TARGET"' EXIT
( cd "$TARGET" && git init -q && printf 'fn main(){}\n' > main.rs && git add -A && git -c user.email=t@t -c user.name=t commit -qm init )

echo "== P0 acceptance =="
have || { echo "  FATAL: $FLEET not built"; exit 3; }   # exit 3 = ENVIRONMENT fault, distinct from agent failure (C23)

# --- A. the path completes -------------------------------------------------
OUT="$("$FLEET" run --task "add a --version flag" --repo "$TARGET" --agent stub 2>&1)"; RC=$?
[ $RC -eq 0 ] && ok "A1 fleet run exits 0" || no "A1 fleet run exits 0" "rc=$RC out=$(printf '%s' "$OUT" | tail -3)"
AID="$(printf '%s' "$OUT" | grep -oE 'artifact=[0-9a-f]{64}' | head -1 | cut -d= -f2)"
[ -n "$AID" ] && ok "A2 run prints an artifact id" || no "A2 run prints an artifact id"

# --- B. the artifact is FROZEN (21 2) --------------------------------------
APATH="$FLEET_STATE/artifacts/$AID"
[ -n "$AID" ] && [ -f "$APATH" ] && ok "B1 artifact exists in the store" || no "B1 artifact exists in the store"
if [ -f "$APATH" ]; then
  MODE="$(stat -f '%Lp' "$APATH" 2>/dev/null || stat -c '%a' "$APATH")"
  [ "$MODE" = "444" ] && ok "B2 artifact mode is 0444" || no "B2 artifact mode is 0444" "mode=$MODE"
  if printf 'x' >> "$APATH" 2>/dev/null; then no "B3 artifact is not writable" "append succeeded"; else ok "B3 artifact is not writable"; fi
  # B4 uses an IN-REPO oracle built against the real blake3 crate. It must never skip:
  # a check that cannot measure FAILS (corpus C4). The earlier version silently skipped when
  # `b3sum` was absent, and that vacuous pass hid a hand-rolled hash implementation.
  ORACLE="${B3ORACLE_BIN:-$ROOT/tests/tools/b3oracle/target/debug/b3oracle}"
  # build it on demand: a suite that fails on a VIRGIN CHECKOUT is a broken contract, and the
  # verifier caught exactly that (25/26 on first run, 26/26 after).
  [ -x "$ORACLE" ] || (cd "$ROOT/tests/tools/b3oracle" && cargo build -q >/dev/null 2>&1)
  if [ ! -x "$ORACLE" ]; then
    no "B4 artifact id == its content hash" "ORACLE MISSING - build tests/tools/b3oracle (this is a FAILURE, not a skip)"
  else
    ACTUAL="$("$ORACLE" "$APATH")"
    [ "$ACTUAL" = "$AID" ] && ok "B4 artifact id == its content hash (independent oracle)" \
      || no "B4 artifact id == its content hash" "oracle=$ACTUAL id=$AID"
  fi
fi

# --- C. attestation verifies, and detects tampering ------------------------
"$FLEET" attest verify "$AID" >/dev/null 2>&1 && ok "C1 attest verify exits 0" || no "C1 attest verify exits 0"
ATT="$FLEET_STATE/attestations/$AID.json"
if [ -f "$ATT" ]; then
  ok "C2 attestation written"
  python3 - "$ATT" "$AID" <<'PY' && ok "C3 in-toto shape: subject digest == artifact id, predicateType correct" || no "C3 in-toto shape"
import json,sys
a=json.load(open(sys.argv[1]))
assert a["_type"]=="https://in-toto.io/Statement/v1", a.get("_type")
assert a["predicateType"]=="https://fleet.local/DeliveryAttestation/v1", a.get("predicateType")
assert a["subject"][0]["digest"]["blake3"]==sys.argv[2], "subject digest != artifact id"
assert "oracle_independence" in a["predicate"]["elements"], "element 8 missing (NON-DROPPABLE)"
PY
  cp "$ATT" "$ATT.bak"; python3 -c "
import json,sys; p=sys.argv[1]; a=json.load(open(p))
a['subject'][0]['digest']['blake3']='0'*64; json.dump(a,open(p,'w'))" "$ATT"
  "$FLEET" attest verify "$AID" >/dev/null 2>&1; RC=$?
  [ $RC -eq 8 ] && ok "C4 tampered attestation -> exit 8" || no "C4 tampered attestation -> exit 8" "rc=$RC"
  mv "$ATT.bak" "$ATT"
else no "C2 attestation written"; fi

# --- D. the worker was never given a name for the ledger (02 2) ------------
"$FLEET" run --task "probe env" --repo "$TARGET" --agent env-probe >/dev/null 2>&1
LEAK="$(grep -rilE 'FLEET_STATE|ledger' "$FLEET_STATE/runs" 2>/dev/null | head -1)"
[ -z "$LEAK" ] && ok "D1 no ledger path leaked into a worker env record" || no "D1 no ledger path leaked" "$LEAK"

# --- E. ledger integrity, incl. under concurrency (S3) ---------------------
"$FLEET" ledger verify >/dev/null 2>&1 && ok "E1 ledger chain verifies" || no "E1 ledger chain verifies"
for i in $(seq 1 20); do "$FLEET" ledger append --event note --body "{\"i\":$i}" >/dev/null 2>&1 & done; wait
"$FLEET" ledger verify >/dev/null 2>&1 && ok "E2 chain valid after 20 concurrent appends" || no "E2 chain valid after 20 concurrent appends"
N="$("$FLEET" ledger count 2>/dev/null || echo 0)"
[ "$N" -ge 20 ] && ok "E3 no rows lost (count=$N)" || no "E3 no rows lost" "count=$N"
U="$("$FLEET" ledger dump 2>/dev/null | python3 -c "
import sys,json
rows=[json.loads(l) for l in sys.stdin if l.strip()]
print(len({r['prev_hash'] for r in rows}), len(rows))" 2>/dev/null || echo '0 1')"
set -- $U; [ "$1" = "$2" ] && ok "E4 no prev_hash reuse (THE assertion the row count misses)" || no "E4 no prev_hash reuse" "distinct=$1 rows=$2"

# --- F. refusals are recorded (S1) -----------------------------------------
BEFORE="$("$FLEET" ledger count 2>/dev/null || echo 0)"
"$FLEET" run --task "" --repo "$TARGET" --agent stub >/dev/null 2>&1; RC=$?
AFTER="$("$FLEET" ledger count 2>/dev/null || echo 0)"
[ $RC -eq 7 ] && ok "F1 empty task refused with exit 7" || no "F1 empty task refused with exit 7" "rc=$RC"
[ "$AFTER" -gt "$BEFORE" ] && ok "F2 the refusal WROTE A RECEIPT (S1: 4 refusals -> 0 receipts)" || no "F2 refusal wrote a receipt"

# --- G. the product may not hand-roll what a crate owns (A1), nor ship twice (A4) -----
LIVE_MANIFEST="$(find "$ROOT/keel" -name Cargo.toml -not -path '*/target/*' -exec grep -l '^\[package\]' {} + 2>/dev/null | head -1)"
if [ -n "$LIVE_MANIFEST" ]; then
  RUNTIME_DEPS="$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f' "$LIVE_MANIFEST")"
  printf '%s' "$RUNTIME_DEPS" | grep -q '^blake3' \
    && ok "G1 blake3 is a RUNTIME dependency, not hand-rolled" \
    || no "G1 blake3 is a RUNTIME dependency, not hand-rolled" "A1: the shipping crate reimplements a maintained primitive"
else no "G1 blake3 is a RUNTIME dependency" "no package manifest found"; fi

PERM="$(grep -rlE 'rotate_right|wrapping_add' "$ROOT/keel" --include='*.rs' 2>/dev/null | grep -v '/target/' | head -3)"
[ -z "$PERM" ] && ok "G2 no hand-rolled hash permutation in the tree" \
  || no "G2 no hand-rolled hash permutation in the tree" "$PERM"

NMAIN="$(find "$ROOT/keel" -name 'main.rs' -not -path '*/target/*' | wc -l | tr -d ' ')"
[ "$NMAIN" -eq 1 ] && ok "G3 exactly one implementation (A4: one implementation per concept)" \
  || no "G3 exactly one implementation" "found $NMAIN main.rs — A4/O3 violation"

# --- H. THE HUMAN SURFACE. Added after driving the CLI as a first-time user found that EVERY
#        user-facing command exited 7 with ZERO output while the gate wall was 8/9 green.
#        This is the "127 tests green while the flagship view rendered nothing" shape: the suite
#        tested the machine path and never the human path.
for c in "--help" "--version" "doctor"; do
  # zsh does NOT word-split unquoted $c — pass explicitly (caught by corpus H1)
  OUT="$("$FLEET" "$c" 2>&1)"; RC=$?
  B=$(printf '%s' "$OUT" | wc -c | tr -d ' ')
  if [ "$RC" -eq 0 ] && [ "$B" -gt 40 ]; then ok "H:$c exits 0 with real output ($B bytes)"
  else no "H:$c exits 0 with real output" "exit=$RC bytes=$B"; fi
done
# a bare invocation must TEACH, not refuse silently
OUT="$("$FLEET" 2>&1)"; B=$(printf '%s' "$OUT" | wc -c | tr -d ' ')
[ "$B" -gt 40 ] && ok "H:bare invocation prints usage" || no "H:bare invocation prints usage" "bytes=$B"
# every module built must be REACHABLE from the CLI, or it is dead code wearing a green build
for c in "ratchet:show" "console:--help"; do
  a1="${c%%:*}"; a2="${c##*:}"
  OUT="$("$FLEET" "$a1" "$a2" 2>&1)"; RC=$?
  [ "$RC" -ne 7 ] && ok "H:'$c' is reachable (not a blanket refusal)" || no "H:'$c' is reachable" "exit 7 = unrouted"
done

# every subcommand must EXPLAIN a refusal, never refuse silently (D12 recurred in new code)
for sc in oracle adjudicate ratchet graph mcp; do
  OUT="$("$FLEET" "$sc" 2>&1)"; B=$(printf '%s' "$OUT" | wc -c | tr -d ' ')
  [ "$B" -gt 10 ] && ok "H:'$sc' explains its refusal ($B bytes)" || no "H:'$sc' explains its refusal" "silent refusal, $B bytes"
done

# D21: an exit code with no message is not a diagnostic. `run` requires FLEET_STATE; it must SAY so.
ENVOUT="$(env -u FLEET_STATE "$FLEET" run --task t --repo "$TARGET" --agent stub 2>&1)"; ERC=$?
[ $ERC -eq 3 ] && ok "I1 missing FLEET_STATE exits 3" || no "I1 missing FLEET_STATE exits 3" "rc=$ERC"
EB=$(printf '%s' "$ENVOUT" | wc -c | tr -d ' ')
case "$ENVOUT" in
  *FLEET_STATE*) [ "$EB" -ge 60 ] && ok "I2 env fault explains itself ($EB bytes)" || no "I2 env fault explains itself" "only $EB bytes" ;;
  *) no "I2 env fault explains itself" "FLEET_STATE unmentioned" ;;
esac

# D22: doctor is the command you run BECAUSE the environment is broken. It must never refuse.
DOUT="$(env -u FLEET_STATE "$FLEET" doctor 2>&1)"; DB=$(printf '%s' "$DOUT" | wc -c | tr -d ' ')
case "$DOUT" in
  *FLEET_STATE*checks*|*checks*) ok "I3 doctor still reports with FLEET_STATE unset ($DB bytes)" ;;
  *) no "I3 doctor still reports with FLEET_STATE unset" "$(printf '%s' "$DOUT" | head -1)" ;;
esac

echo "== $PASS passed, $FAIL failed =="
[ "$FAIL" -eq 0 ]
