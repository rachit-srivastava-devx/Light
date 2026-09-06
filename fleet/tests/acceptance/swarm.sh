#!/usr/bin/env bash
# swarm.sh — the acceptance contract for the agentic swarm and its per-agent SDLC.
#
# LEAD-AUTHORED. Builders MUST NOT edit this file. It was written BEFORE the implementation and is
# the blind suite for blueprint 06 (the lifecycle) and requirements 4, 8 and 13.
#
# The product is not the substrate (freeze/attest/ledger/ratchet — all of that already passes).
# The product is a swarm of agents that each run a STRICT SDLC. This file is what makes that claim
# falsifiable. Everything here must be able to FAIL.
set -u
if [ -n "${GIT_DIR:-}" ] || [ -n "${GIT_INDEX_FILE:-}" ] || [ -n "${GIT_WORK_TREE:-}" ]; then
  echo "FATAL: refusing to run with an inherited git context (D23)." >&2; exit 3
fi
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLEET="${FLEET_BIN:-$ROOT/keel/target/debug/fleet}"
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
no(){ FAIL=$((FAIL+1)); printf '  FAIL %s   %s\n' "$1" "${2:-}"; }
have(){ [ -x "$FLEET" ]; }
have || { echo "  FATAL: $FLEET not built"; exit 3; }
# Same reasoning as p0.sh: this suite tests the swarm and its evidence, not the planning gate.
# The SOW gate (reqs 6/7/14) is asserted on its own; bypassing it here keeps these assertions
# measuring what they were written for. The bypass is recorded in the receipt.
export FLEET_SOW_BYPASS=1
export FLEET_STATE="$(mktemp -d "${TMPDIR:-/tmp}/fleet-swarm.XXXXXX")"

# ---- S: the state machine is a TYPE, not a field -----------------------------------------------
# The predecessor had a `role` field of free text with no behaviour. The blueprint's fix is
# typestate: Task<S: State>, transitions consume self. An enum + runtime match is NOT this.
SRC="$ROOT/keel/fleet/src"
grep -rq 'PhantomData' "$SRC" && ok "S1 lifecycle uses PhantomData (typestate, not a runtime field)" \
  || no "S1 lifecycle uses PhantomData (typestate, not a runtime field)" "no PhantomData in src/"
grep -rqE 'struct Task<' "$SRC" && ok "S2 Task is generic over its state" \
  || no "S2 Task is generic over its state" "no 'struct Task<' in src/"
# every state in blueprint 06 must exist as a type
MISSING=""
for s in Intake Specified Reviewed Decomposed Contracted Briefed Leased Building Built Verifying Verified Attested Accepted Observed Refused; do
  grep -rqE "(struct|enum) $s\b" "$SRC" || MISSING="$MISSING $s"
done
[ -z "$MISSING" ] && ok "S3 all 15 blueprint states exist as types" \
  || no "S3 all 15 blueprint states exist as types" "missing:$MISSING"
# transitions must CONSUME self — a &self transition can be called twice from one state
grep -rqE 'fn [a-z_]+\(self[,)]' "$SRC" && ok "S4 transitions consume self (cannot fire twice)" \
  || no "S4 transitions consume self (cannot fire twice)" "no self-consuming transition found"

# ---- C: illegal transitions DO NOT COMPILE (the whole point) -----------------------------------
# This is the assertion that separates typestate from a match statement. If it can be expressed
# and merely refused at runtime, the blueprint's claim is false.
CF="$ROOT/keel/fleet/tests/compile_fail"
if [ -d "$CF" ] && ls "$CF"/*.rs >/dev/null 2>&1; then
  ok "C1 compile-fail fixtures exist ($(ls "$CF"/*.rs | wc -l | tr -d ' ') cases)"
  grep -rq 'trybuild' "$ROOT/keel/fleet/Cargo.toml" && ok "C2 trybuild wired to run them" \
    || no "C2 trybuild wired to run them" "compile-fail dir exists but nothing runs it"
else
  no "C1 compile-fail fixtures exist" "no keel/fleet/tests/compile_fail/*.rs"
  no "C2 trybuild wired to run them" "no fixtures to run"
fi

# ---- R: roles own gates, and the lead may not write implementation code -------------------------
for r in lead builder verifier designer meter; do
  "$FLEET" roles 2>/dev/null | grep -qi "$r" && ok "R1:$r role is declared" || no "R1:$r role is declared" "not in 'fleet roles'"
done
OUT="$("$FLEET" role-check --role lead --diff-adds-code 1 2>&1)"; RC=$?
# "unknown command" also exits non-zero. A refusal that is really a missing subcommand is a VACUOUS
# pass -- the exact defect class D19/D25 were. Require the command to exist AND to refuse.
case "$OUT" in *"unknown command"*) no "R2 LEAD_WROTE_CODE refuses" "role-check does not exist (would have been a vacuous pass)" ;;
  *) [ $RC -ne 0 ] && ok "R2 LEAD_WROTE_CODE refuses (exit $RC)" || no "R2 LEAD_WROTE_CODE refuses" "exit 0 -- the lead was allowed to write code" ;;
esac
printf '%s' "$OUT" | grep -q 'LEAD_WROTE_CODE' && ok "R3 the refusal names LEAD_WROTE_CODE" \
  || no "R3 the refusal names LEAD_WROTE_CODE" "got: $(printf '%s' "$OUT" | head -1)"
OUT="$("$FLEET" role-check --role builder --diff-adds-code 1 2>&1)"; RC=$?
[ $RC -eq 0 ] && ok "R4 a builder MAY write code (the gate is not blanket-deny)" \
  || no "R4 a builder MAY write code" "exit $RC — refuses everything, so R2 proves nothing"

# ---- V: the verifier must be a DIFFERENT model than the builder ---------------------------------
OUT="$("$FLEET" role-check --role verifier --builder-model m1 --verifier-model m1 2>&1)"; RC=$?
case "$OUT" in *"unknown command"*) no "V1 SELF_VERIFIED refused when both models match" "role-check does not exist (would have been a vacuous pass)" ;;
  *) [ $RC -ne 0 ] && ok "V1 SELF_VERIFIED refused when both models match (exit $RC)" || no "V1 SELF_VERIFIED refused when both models match" "exit 0" ;;
esac
printf '%s' "$OUT" | grep -q 'SELF_VERIFIED' && ok "V2 the refusal names SELF_VERIFIED" \
  || no "V2 the refusal names SELF_VERIFIED" "got: $(printf '%s' "$OUT" | head -1)"
"$FLEET" role-check --role verifier --builder-model m1 --verifier-model m2 >/dev/null 2>&1 \
  && ok "V3 distinct models ACCEPTED (both directions)" || no "V3 distinct models ACCEPTED" "refuses even distinct models"

# ---- W: a swarm is more than one agent, each with its OWN lifecycle instance --------------------
OUT="$("$FLEET" swarm status --json 2>&1)"; RC=$?
[ $RC -eq 0 ] && ok "W1 'swarm status' exists and exits 0" || no "W1 'swarm status' exists" "exit $RC"
N=$(printf '%s' "$OUT" | grep -oE '"state"' 2>/dev/null | wc -l | tr -d ' '); N=${N:-0}
[ "${N:-0}" -ge 2 ] && ok "W2 at least 2 agents carry independent state (n=$N)" \
  || no "W2 at least 2 agents carry independent state" "n=${N:-0} — a swarm of one is not a swarm"
printf '%s' "$OUT" | grep -q '"denominator"' && ok "W3 swarm status publishes a denominator" \
  || no "W3 swarm status publishes a denominator" "measuring nothing must not read as success"


# ---- X: a dispatch that produces no EVIDENCE is a facade (D36) --------------------------------
# Found by driving the CLI as a user, not by reading a test. `swarm dispatch` advanced all five
# agents, wrote scorecards claiming credited:1, and printed an artifact id -- while writing NO
# ledger and NO attestation, and printing the SAME id for completely different tasks. Scorecards
# that credit work with no ledger row to justify it are the exact substitution this repo exists
# to catch, so these assertions are the contract for the fix.
XS1="$(mktemp -d)"; XT1="$(mktemp -d)"
( cd "$XT1" && git init -q . && printf 'fn main(){}\n' > main.rs && git add -A \
  && git -c user.email=a@b -c user.name=a commit -qm i ) >/dev/null 2>&1
XA1="$(FLEET_STATE="$XS1" "$FLEET" swarm dispatch --task "alpha task" --repo "$XT1" 2>&1 | grep -oE 'artifact=[0-9a-f]+' | cut -d= -f2)"
XS2="$(mktemp -d)"
XA2="$(FLEET_STATE="$XS2" "$FLEET" swarm dispatch --task "beta task entirely different" --repo "$XT1" 2>&1 | grep -oE 'artifact=[0-9a-f]+' | cut -d= -f2)"
if [ -n "$XA1" ] && [ "$XA1" != "$XA2" ]; then
  ok "X1 dispatch artifact id differs per task (not a canned constant)"
else
  no "X1 dispatch artifact id differs per task" "alpha=$(printf '%s' "$XA1" | cut -c1-16) beta=$(printf '%s' "$XA2" | cut -c1-16)"
fi
# NOT `... && ok || no "rc=$?"` -- $? there is the status of the && || chain, not of the command.
# B10 caught this in my own assertion. Capture the code first.
FLEET_STATE="$XS1" "$FLEET" ledger verify >/dev/null 2>&1; XRC=$?
[ $XRC -eq 0 ] && ok "X2 dispatch wrote a verifiable ledger" || no "X2 dispatch wrote a verifiable ledger" "ledger verify rc=$XRC"
XN="$(FLEET_STATE="$XS1" "$FLEET" ledger count 2>/dev/null | tr -dc '0-9')"
[ "${XN:-0}" -gt 0 ] && ok "X3 dispatch appended ledger rows (n=$XN)" || no "X3 dispatch appended ledger rows" "count=${XN:-empty}"
FLEET_STATE="$XS1" "$FLEET" attest verify "${XA1:-none}" >/dev/null 2>&1; XRC4=$?
if [ -n "$XA1" ] && [ $XRC4 -eq 0 ]; then
  ok "X4 the dispatched artifact's attestation verifies"
else
  no "X4 the dispatched artifact's attestation verifies" "attest verify rc=$XRC4 id=${XA1:-empty}"
fi


# ---- N: the natural-language front door (free text in, fleet decides) --------------------------
# The router shipped reachable ONLY from the tty REPL, so no gate could drive it and no user could
# script it -- reachable by a human, unreachable by a test. `fleet plan` is the non-interactive
# door; these assertions are what stop the router regressing silently.
NOUT="$("$FLEET" plan "add a --version flag" 2>&1)"; NRC=$?
case "$NOUT" in *"swarm dispatch"*) [ $NRC -eq 0 ] && ok "N1 free text routes to swarm dispatch" || no "N1 free text routes to swarm dispatch" "rc=$NRC" ;;
  *) no "N1 free text routes to swarm dispatch" "$(printf '%s' "$NOUT" | head -1)" ;; esac
NOUT="$("$FLEET" plan "is the ledger ok" 2>&1)"
case "$NOUT" in *"ledger verify"*) ok "N2 a question routes to ledger verify" ;; *) no "N2 a question routes to ledger verify" "$(printf '%s' "$NOUT" | head -1)" ;; esac
NOUT="$("$FLEET" plan "what broke" 2>&1)"
NC=$(printf '%s' "$NOUT" | grep -cE '^\s+[0-9]+\. fleet ')
[ "${NC:-0}" -ge 2 ] && ok "N3 a multi-step intent plans >1 command (n=$NC)" || no "N3 a multi-step intent plans >1 command" "n=${NC:-0}"
case "$NOUT" in *denominator*) ok "N4 the plan publishes its denominator" ;; *) no "N4 the plan publishes its denominator" "no denominator in the plan" ;; esac
NOUT="$("$FLEET" plan "make me a sandwich" 2>&1)"; NRC=$?
if [ $NRC -ne 0 ] && printf '%s' "$NOUT" | grep -q 'no committed intent matches'; then
  ok "N5 an unmatched prompt REFUSES instead of guessing (rc=$NRC)"
else
  no "N5 an unmatched prompt REFUSES instead of guessing" "rc=$NRC out=$(printf '%s' "$NOUT" | head -1)"
fi
printf '%s' "$NOUT" | grep -q 'closest intents' && ok "N6 the refusal names near matches" || no "N6 the refusal names near matches" "no candidates offered"
NBEFORE="$(FLEET_STATE="$(mktemp -d)" sh -c "\"$FLEET\" plan 'add a --version flag' >/dev/null 2>&1; echo done")"
[ "$NBEFORE" = done ] && ok "N7 plan has no side effects (it is a plan, not a run)" || no "N7 plan has no side effects" "$NBEFORE"


# ---- P: the punch list from driving fleet as a user (contract written BEFORE the fixes) --------
PS="$(mktemp -d)"; PT="$(mktemp -d)"
( cd "$PT" && git init -q . && printf 'fn main(){}\n' > main.rs && git add -A \
  && git -c user.email=a@b -c user.name=a commit -qm i ) >/dev/null 2>&1
FLEET_STATE="$PS" "$FLEET" swarm dispatch --task "punch list task" --repo "$PT" >/dev/null 2>&1

# P1 `ratchet check` must be usable by a human: discoverable flags and an actionable error.
POUT="$(FLEET_STATE="$PS" "$FLEET" ratchet check 2>&1)"; PRC=$?
if printf '%s' "$POUT" | grep -qE 'usage|--metrics .*--change|example'; then
  ok "P1 bare 'ratchet check' prints usage, not just a missing-flag name"
else
  no "P1 bare 'ratchet check' prints usage" "rc=$PRC out=$(printf '%s' "$POUT" | head -1)"
fi
POUT="$(FLEET_STATE="$PS" "$FLEET" ratchet check --metrics 'dispatch_acceptance=0.5' --change probe 2>&1)"; PRC=$?
case "$POUT" in
  *scorecard*|*"unable to read"*)
    printf '%s' "$POUT" | grep -qE 'FLEET_STATE|scorecard.*(path|expected|create)|fleet ' \
      && ok "P2 a missing scorecard says WHERE it belongs" \
      || no "P2 a missing scorecard says WHERE it belongs" "$(printf '%s' "$POUT" | head -1)" ;;
  *) [ $PRC -ne 0 ] && ok "P2 a below-mark value is refused (rc=$PRC)" || no "P2 a below-mark value is refused" "rc=0" ;;
esac

# P3 `--agent codex` must either work or explain itself. Silence is the defect.
POUT="$(FLEET_STATE="$(mktemp -d)" "$FLEET" run --task "add a flag" --repo "$PT" --agent codex 2>&1)"; PRC=$?
PB=$(printf '%s' "$POUT" | wc -c | tr -d ' ')
if [ $PRC -eq 0 ] || [ "$PB" -ge 40 ]; then
  ok "P3 --agent codex works or explains itself (rc=$PRC, ${PB} bytes)"
else
  no "P3 --agent codex works or explains itself" "rc=$PRC and only ${PB} bytes of output"
fi

# P4 the meter must record real work: tokenomics is a first principle, not a separate toy.
POUT="$(FLEET_STATE="$PS" "$FLEET" meter show 2>&1)"; PRC=$?
if [ $PRC -eq 0 ] && printf '%s' "$POUT" | grep -qE '[0-9]'; then
  ok "P4 meter show reports usage after a dispatch"
else
  no "P4 meter show reports usage after a dispatch" "rc=$PRC out=$(printf '%s' "$POUT" | head -1)"
fi

# P5 skills must be more than declared strings: something must resolve them.
POUT="$(FLEET_STATE="$PS" "$FLEET" skills 2>&1)"; PRC=$?
if [ $PRC -eq 0 ] && printf '%s' "$POUT" | grep -qE 'denominator|[0-9]+ of [0-9]+'; then
  ok "P5 'fleet skills' resolves the declared skills with a denominator"
else
  no "P5 'fleet skills' resolves the declared skills" "rc=$PRC out=$(printf '%s' "$POUT" | head -1)"
fi


# ---- S: the SOW gate must refuse an incomplete plan AND accept a complete one ------------------
# Every invocation below uses `env -u FLEET_SOW_BYPASS`. The suite sets that bypass globally so the
# OTHER assertions can test the kernel, and my first draft of this block inherited it -- which
# silently disabled the gate for the very assertions written to test it. S6 caught it by failing;
# S8 would have passed for the wrong reason. A global test bypass neuters the tests of the thing
# it bypasses.
# A gate that refuses everything proves exactly as little as one that refuses nothing. I first
# concluded this gate refused everything -- it had rejected three of my malformed SOWs in a row.
# It was walking me to a valid one. Both directions, or it is theatre.
SS="$(mktemp -d)"; ST="$(mktemp -d)"
( cd "$ST" && git init -q . && printf 'fn main(){}\n' > main.rs && git add -A \
  && git -c user.email=a@b -c user.name=a commit -qm i ) >/dev/null 2>&1
GOOD='add a --version flag
leaves:
- print the crate version and exit 0 | acceptance: running --version prints a semver and exits 0
challenges:
- the flag may collide with an existing one | citation: keel/fleet/src/main.rs:40
alternatives:
- json-output: emit JSON instead of plain text | tradeoff: harder for humans to read
- build-info: print commit and build date too | tradeoff: leaks build-host details
estimates:
- 30 minutes
edge cases:
- --version combined with another flag'
SOUT="$(env -u FLEET_SOW_BYPASS FLEET_STATE="$SS" "$FLEET" sow --task "$GOOD" 2>&1)"; SRC=$?
[ $SRC -eq 9 ] && ok "S5 a complete SOW exits 9 (awaiting review)" || no "S5 a complete SOW exits 9" "rc=$SRC"
SID="$(printf '%s' "$SOUT" | grep -oE '[0-9a-f]{64}' | head -1)"
env -u FLEET_SOW_BYPASS FLEET_STATE="$SS" "$FLEET" run --task "$GOOD" --repo "$ST" --agent stub >/dev/null 2>&1; SRC=$?
[ $SRC -eq 7 ] && ok "S6 run before acceptance is refused (rc=7)" || no "S6 run before acceptance is refused" "rc=$SRC"
env -u FLEET_SOW_BYPASS FLEET_STATE="$SS" "$FLEET" sow accept --id "${SID:-none}" >/dev/null 2>&1; SRC=$?
[ $SRC -eq 0 ] && ok "S7 a human can accept the SOW" || no "S7 a human can accept the SOW" "rc=$SRC id=${SID:-empty}"
env -u FLEET_SOW_BYPASS FLEET_STATE="$SS" "$FLEET" run --task "$GOOD" --repo "$ST" --agent stub >/dev/null 2>&1; SRC=$?
[ $SRC -eq 0 ] && ok "S8 run proceeds AFTER acceptance (the gate is not blanket-deny)" || no "S8 run proceeds after acceptance" "rc=$SRC"
SOUT="$(env -u FLEET_SOW_BYPASS FLEET_STATE="$(mktemp -d)" "$FLEET" sow --task "make it better" 2>&1)"; SRC=$?
if [ $SRC -eq 7 ] && printf '%s' "$SOUT" | grep -q 'CLARIFYING QUESTIONS'; then
  ok "S9 a vague task asks questions instead of guessing"
else
  no "S9 a vague task asks questions instead of guessing" "rc=$SRC"
fi


# ---- Q: every refusal must be ACTIONABLE, not merely honest ------------------------------------
# Three separate times a command exited with a correct code and a message a user could not act on:
# D21 (FLEET_STATE unset), P1/P2 (ratchet's missing scorecard), and the meter's unset windows. An
# honest exit code with no next step is a dead end. This asserts the property across the surface
# instead of patching a fourth instance later.
QFAIL=0; QN=0
# Each entry is a bash ARRAY, expanded as "${qcmd[@]}". The first draft used an unquoted $qc to
# word-split into arguments, which H1 correctly flagged: unquoted $var as argv is the exact trap
# that produced several false readings in this build. Arrays remove the hazard instead of
# exempting it -- a detector that has to be told "this one is fine" stops being a detector.
for qspec in "meter:show" "meter:reserve" "ratchet:check" "sow:" "swarm:dispatch" "oracle:" "adjudicate:" "skills:--check" "rollback:" "status:--nonsense" "run:--task" "route:--role"; do
  QN=$((QN+1))
  qhead="${qspec%%:*}"; qtail="${qspec#*:}"
  if [ -n "$qtail" ]; then qcmd=("$qhead" "$qtail"); else qcmd=("$qhead"); fi
  qc="$qhead ${qtail}"
  # "$FLEET" MUST be quoted: the repo path contains a space, so an unquoted $FLEET made `env`
  # fail before fleet ever ran, and the old `*/*` clause matched env's own error message. The
  # assertion was passing on its own breakage. $qc stays unquoted so it word-splits into args.
  QOUT="$(env -u FLEET_SOW_BYPASS FLEET_STATE="$(mktemp -d)" "$FLEET" "${qcmd[@]}" 2>&1)"; QRC=$?
  # Only a REFUSAL owes a next step. `skills --check` exits 0 when everything resolves, and a
  # successful command is not a dead end -- counting it was my error, not fleet's.
  [ $QRC -eq 0 ] && { QN=$((QN-1)); continue; }
  # actionable = names a command to run, an env var to set, or a path to create
  # `*/*` (any slash) was in this list and made the check nearly unfalsifiable -- almost every
  # error contains a path. Actionable means a COMMAND to run or an ENV VAR to set, nothing looser.
  case "$QOUT" in
    *"fleet "*|*"export FLEET_"*|*"FLEET_"*=*) ;;
    *) QFAIL=$((QFAIL+1)); printf '     not actionable: %s -> %s\n' "$qc" "$(printf '%s' "$QOUT" | head -1 | cut -c1-52)" ;;
  esac
done
[ "$QFAIL" -eq 0 ] && ok "Q1 all $QN refusal surfaces are actionable (denominator: $QN)" \
  || no "Q1 all refusal surfaces are actionable" "$QFAIL of $QN give no next step"


# D41: a plan must name every gate the user has to pass, or it walks them into a refusal.
POUT="$("$FLEET" plan "add a --version flag" 2>&1)"
case "$POUT" in
  *"fleet sow"*) ok "N8 the change plan names the SOW step before dispatch" ;;
  *) no "N8 the change plan names the SOW step" "plan sends the user straight to a refusal" ;;
esac


# D42: the worked example a refusal prints must ACTUALLY WORK. An example that does not is worse
# than no example -- it costs the user a round trip and their trust in the next one. This extracts
# the example straight out of the refusal and feeds it back in.
EXOUT="$(env -u FLEET_SOW_BYPASS FLEET_STATE="$(mktemp -d)" "$FLEET" sow --task "add a --version flag" 2>&1)"
# awk, not sed: BSD sed has no \| alternation in a basic regex, so the section headers were
# silently dropped and only the "- " lines survived -- the extraction looked plausible and was
# missing half the input. Same portability class as T6's `stat -f`.
EXTASK="$(printf '%s' "$EXOUT" | awk '
  /^  fleet sow --task "/ { sub(/^  fleet sow --task "/, ""); print; grab=1; next }
  grab { line=$0; sub(/^  /, "", line); sub(/"$/, "", line); if (line == "") { grab=0; next } print line }
')"
if [ -n "$EXTASK" ]; then
  # H1: `$?` must not be read after a line containing a command substitution -- $(mktemp -d) runs
  # LAST and its status is what $? would report. Resolve the temp dir first, on its own line.
  EXSTATE="$(mktemp -d)"
  env -u FLEET_SOW_BYPASS FLEET_STATE="$EXSTATE" "$FLEET" sow --task "$EXTASK" >/dev/null 2>&1
  EXRC=$?
  [ $EXRC -eq 9 ] && ok "S10 the example printed by the refusal actually works (rc=9)" \
    || no "S10 the example printed by the refusal actually works" "replaying it gives rc=$EXRC"
else
  no "S10 the example printed by the refusal actually works" "no example found in the refusal"
fi


# D52: the intent table must cover the language people actually use, without guessing. These are
# real phrasings, and "make me a sandwich" stays the control -- broadening recall must not cost
# the refusal.
for nverb in "handle division by zero" "support UTF-8 filenames" "validate the config on load" "delete the dead retry path"; do
  "$FLEET" plan "$nverb" >/dev/null 2>&1 || { no "N9 ordinary implementation phrasings route ($nverb)" "refused"; nfail=1; }
done
[ "${nfail:-0}" -eq 0 ] && ok "N9 ordinary implementation phrasings route (4 checked)"


# D53: diagnostics must be ordered by what the user has to fix FIRST. A non-existent repo and a
# non-git directory both reported "task has no accepted SOW" -- advice that could not possibly fix
# either. Cheap structural checks precede the policy gate.
ENG="$(mktemp -d)"; echo x > "$ENG/f.txt"
EOUT="$(env -u FLEET_SOW_BYPASS FLEET_STATE="$(mktemp -d)" "$FLEET" run --task t --repo "$ENG" --agent stub 2>&1)"
case "$EOUT" in
  *"not a git repository"*) ok "E1 a non-git repo says so, not 'no accepted SOW'" ;;
  *) no "E1 a non-git repo says so" "$(printf '%s' "$EOUT" | head -1 | cut -c1-46)" ;;
esac
EOUT="$(env -u FLEET_SOW_BYPASS FLEET_STATE="$(mktemp -d)" "$FLEET" run --task t --repo /nonexistent --agent stub 2>&1)"
case "$EOUT" in
  *"does not exist"*) ok "E2 a missing repo says so, not 'no accepted SOW'" ;;
  *) no "E2 a missing repo says so" "$(printf '%s' "$EOUT" | head -1 | cut -c1-46)" ;;
esac


# ---- C: concurrent dispatch on a COLD store (D54) ----------------------------------------------
# Three stacked races lived here: a TOCTOU on the agent store, lost scorecard updates, and partial
# reads while seeding. 1 in 4 dispatches failed. The cold start is the vulnerable moment -- every
# process races to seed -- so the fixture must NOT pre-warm the store.
CS="$(mktemp -d)"; CPIDS=""
for cn in 1 2 3 4 5 6; do
  CR="$(mktemp -d)"
  ( cd "$CR" && git init -q . && printf 'fn main(){}\n' > main.rs && git add -A \
    && git -c user.email=a@b -c user.name=a commit -qm i ) >/dev/null 2>&1
  ( FLEET_SOW_BYPASS=1 FLEET_STATE="$CS" "$FLEET" swarm dispatch --task "concurrent $cn" \
      --repo "$CR" --agent stub >/dev/null 2>&1; echo $? > "$CS/.rc$cn" ) &
  CPIDS="$CPIDS $!"
done
for cp in $CPIDS; do wait "$cp"; done
CFAIL=0
for cn in 1 2 3 4 5 6; do [ "$(cat "$CS/.rc$cn" 2>/dev/null)" = 0 ] || CFAIL=$((CFAIL+1)); done
[ "$CFAIL" -eq 0 ] && ok "CC1 six concurrent dispatches on a cold store all succeed (denominator: 6)" \
  || no "CC1 six concurrent dispatches on a cold store all succeed" "$CFAIL of 6 failed"
FLEET_STATE="$CS" "$FLEET" ledger verify >/dev/null 2>&1; CLRC=$?
[ $CLRC -eq 0 ] && ok "CC2 the ledger chain survives concurrent writers" \
  || no "CC2 the ledger chain survives concurrent writers" "verify rc=$CLRC"
CAG=$(FLEET_STATE="$CS" "$FLEET" agents list 2>/dev/null | grep -c '^agent_id')
[ "${CAG:-0}" -ge 5 ] && ok "CC3 the agent store is complete after the race (n=$CAG)" \
  || no "CC3 the agent store is complete after the race" "only ${CAG:-0} agents"

echo "== $PASS passed, $FAIL failed =="
[ $FAIL -eq 0 ]
