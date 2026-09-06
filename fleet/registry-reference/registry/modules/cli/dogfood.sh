#!/usr/bin/env bash
# dogfood.sh — run fleet on a real prompt and measure what a USER actually experiences.
#
# Not "do the tests pass". The question is: hand fleet a vague request, and is the thing that comes
# back worth the wait and the tokens? So this measures the four things the owner named, plus the one
# behaviour fleet claims as its thesis.
#
#   1. LATENCY TO FIRST DECISION — wall-clock until fleet says something actionable. Not total
#      runtime: a user judges a tool by when it first tells them something.
#   2. INTER-STAGE LATENCY — the gap between consecutive outputs. A tool that emits everything after
#      four silent minutes feels broken even at the same total.
#   3. READ RATIO — bytes emitted vs bytes a human would plausibly read. A 40 KB report where the
#      decision is 300 bytes has a read ratio near zero, and the rest is cost with no reader.
#   4. AMBIGUITY BEHAVIOUR — given a deliberately vague prompt, does fleet ASK, or does it guess?
#      Requirement 6 says ask. Exit 7 is the refusal code. Guessing is the failure.
#
# HONEST LIMITS, stated because a metric whose limits are hidden is a claim:
#   * "bytes a human would read" is a HEURISTIC — the decision line, the questions, and any
#     error/remedy. It is a proxy, labelled as one, and the raw byte counts are recorded so the
#     ratio can be recomputed under a different definition.
#   * latency includes model queue time, which varies by hour. One run is an anecdote; --repeat N
#     gives a spread. Nothing here reports a single sample as a measurement.
#   * this measures INTAKE, which is what fleet gates on. It does not measure end-to-end build
#     quality; registry/features/gate/gate.sh and the bats suites do that, and this does not pretend otherwise.
set -uo pipefail

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$D" || exit 3
# shellcheck source=../../registry/lib/err.sh
. "$D/registry/lib/err.sh" 2>/dev/null || { echo "dogfood: registry/lib/err.sh missing" >&2; exit 3; }

PROMPT=""; REPEAT=1; RUN_DIR=""; JSON=0
usage() {
  cat <<'EOF'
dogfood.sh --prompt "TEXT" [--repeat N] [--json]
  Runs fleet's intake on PROMPT and records user-facing effectiveness metrics.
  Writes var/runs/dogfood/<stamp>/ and appends a row to var/runs/dogfood/runs.tsv
EOF
}
while [ $# -gt 0 ]; do
  case "$1" in
    --prompt) need_val --prompt "${2-}"; PROMPT="$2"; shift 2 ;;
    --repeat) need_val --repeat "${2-}"; REPEAT="$2"; shift 2 ;;
    --json)   JSON=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "$ERR_USAGE" unknown_flag "unknown flag '$1'" "see --help" ;;
  esac
done
[ -n "$PROMPT" ] || die "$ERR_USAGE" no_prompt "a prompt is required" 'dogfood.sh --prompt "..."'
case "$REPEAT" in ''|*[!0-9]*) die "$ERR_USAGE" bad_repeat "--repeat must be an integer" "e.g. --repeat 3" ;; esac

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="$D/var/runs/dogfood/$STAMP"
mkdir -p "$RUN_DIR"
TSV="$D/var/runs/dogfood/runs.tsv"
[ -f "$TSV" ] || printf '# stamp\titer\tstage\texit\twall_ms\tbytes_out\tbytes_readable\tread_pct\tasked\n' >"$TSV"

# Monotonic-ish milliseconds. `date +%s%3N` is GNU-only; python3 is already a hard dependency.
now_ms() { python3 -c 'import time; print(int(time.monotonic()*1000))'; }

# The readable-byte heuristic. A human reads the verdict, the questions, and the remedy — not the
# TOON table headers, not the banner, not the echoed input.
readable_bytes() {
  # Non-readable = the TOON envelope only: `tool:`, `description:`, `generatedAt:`, and the
  # `name[N]{col,col}:` column header. Everything else is content a human reads.
  grep -avE '^(tool|description|generatedAt):|^[a-z_]+\[[0-9]+\](\{[^}]*\})?:$' "$1" 2>/dev/null \
    | wc -c | tr -d ' '
}

echo "== dogfood =="
echo "  prompt : ${PROMPT:0:96}"
echo "  repeat : $REPEAT"
echo "  out    : $RUN_DIR"
echo

total_asked=0
for i in $(seq 1 "$REPEAT"); do
  echo "-- iteration $i --"
  prev_end=""
  # `ask` takes the intent; `blueprint`, `summary` and `status` take none. Passing the prompt to a
  # no-arg subcommand measured an unknown-flag error and called it a stage.
  for stage in ask status blueprint summary; do
    out="$RUN_DIR/i${i}.${stage}.out"
    arg=""; [ "$stage" = ask ] && arg="$PROMPT"
    t0="$(now_ms)"
    # Sandbox the ledger: a measurement run must never touch the production chain. That mistake put
    # 776 rows into it once and broke it permanently.
    FLEET_LEDGER="$RUN_DIR/ledger.jsonl" timeout 300 bash "$D/registry/services/verify/intake.sh" "$stage" ${arg:+"$arg"} >"$out" 2>&1
    ec=$?
    t1="$(now_ms)"
    wall=$(( t1 - t0 ))
    bytes=$(wc -c <"$out" | tr -d ' ')
    read_b="$(readable_bytes "$out")"; [ -n "$read_b" ] || read_b=0
    pct=0; [ "$bytes" -gt 0 ] && pct=$(( read_b * 100 / bytes ))
    # exit 7 = ERR_AMBIGUITY = fleet refused and asked. That is the CORRECT answer to a vague prompt.
    asked=no; [ "$ec" -eq 7 ] && { asked=yes; total_asked=$((total_asked+1)); }
    gap="-"
    [ -n "$prev_end" ] && gap=$(( t0 - prev_end ))
    prev_end="$t1"
    printf '  %-10s exit=%-2s wall=%6sms gap=%8s bytes=%6s readable=%5s (%s%%) asked=%s\n' \
      "$stage" "$ec" "$wall" "$gap" "$bytes" "$read_b" "$pct" "$asked"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$STAMP" "$i" "$stage" "$ec" "$wall" "$bytes" "$read_b" "$pct" "$asked" >>"$TSV"
  done
done

echo
echo "== verdict =="
# The thesis check. Requirement 6: clear ambiguity FIRST. A vague prompt that produces a plan
# instead of a question is fleet failing its own contract, and this says so rather than scoring it.
if [ "$total_asked" -gt 0 ]; then
  echo "  ambiguity gate: FIRED ($total_asked of $((REPEAT * 4)) stages refused with exit 7)"
  echo "  -> fleet asked instead of guessing. This is requirement 6 holding."
else
  echo "  ambiguity gate: DID NOT FIRE"
  echo "  -> fleet produced output for a deliberately vague prompt without asking. Requirement 6"
  echo "     says clear ambiguity first; this is a contract failure, not a fast path."
fi
awk -F'\t' '!/^#/ {n++; w+=$5; b+=$6; r+=$7} END {
  if (n) printf "\n  mean wall %.0fms over %d stages | total emitted %d B | readable %d B (%.0f%%)\n", w/n, n, b, r, (b? r*100/b : 0)
}' "$TSV"
echo
echo "  read ratio is a HEURISTIC (verdict + questions + remedy lines / total). Raw byte counts are"
echo "  in $TSV so it can be recomputed under a different definition of 'readable'."
[ "$JSON" -eq 1 ] && jq -cn --arg dir "$RUN_DIR" --arg asked "$total_asked" '{dogfood:{dir:$dir,stages_asked:($asked|tonumber)}}'
exit 0
