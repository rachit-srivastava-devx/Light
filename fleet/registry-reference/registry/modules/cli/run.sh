#!/usr/bin/env bash
# run.sh — the one path: a task in, clarifying questions out, then role agents each walking an SDLC.
#
# Every piece of this already existed and NOTHING chained them: intake could ask, roles were
# declared, dispatch could spawn, the pipeline had eight states, review could take a verdict — and
# there was no command that ran them in order. That is why fleet had 37 components and zero completed
# end-to-end runs. This is the chain.
#
#   ./fleet run "TASK"              stage 1: plan, then REFUSE with clarifying questions (exit 7)
#   ./fleet run --answer ID "TEXT"  answer one clarification
#   ./fleet run --continue          author and show the SOW; after acceptance, fan out role agents
#   ./fleet run --accept-sow         record explicit human SOW acceptance, then continue to fan-out
#   ./fleet run --status            where the current task is
#
# WHOSE IS WHAT, because this was gotten wrong twice:
#   * the PROCESS is fleet's — the gates, the roles, the refusals, the ordering. Identical for anyone.
#   * the EXECUTION is the user's — their repo, their `claude`/`codex` login, their git identity.
#     fleet brings no credentials and never authors a commit.
#
# The human gate is absolute: this never pushes, merges, or opens a PR. It ends by telling you what
# to verify by hand, because "tested and verified manually" is a step a harness cannot do for you.
set -uo pipefail

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
# shellcheck source=../../lib/err.sh
. "$D/registry/lib/err.sh" 2>/dev/null || { echo "run: registry/lib/err.sh missing" >&2; exit 3; }

TARGET="$D"; MODE=plan; ANSWER_ID=""; ANSWER_TEXT=""; DRY=0; JSON=0
# Exit 9 is local to this CLI: the SOW is validated and presented, but execution is intentionally
# paused until a human explicitly runs --accept-sow. The normal fleet refusal table remains 0-8.
SOW_REVIEW_EXIT=9
STATE_DIR="${FLEET_RUN_STATE:-$D/var/run}"

usage() { sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
while [ $# -gt 0 ]; do
  case "$1" in
    --target)   need_val --target "${2-}";  TARGET="$(cd "$2" && pwd)"; shift 2 ;;
    --answer)   need_val --answer "${2-}";  MODE=answer; ANSWER_ID="$2"; ANSWER_TEXT="${3-}"; shift 2; [ $# -gt 0 ] && shift ;;
    --continue) MODE="continue"; shift ;;
    --accept-sow) MODE="accept-sow"; shift ;;
    --status)   MODE=status;   shift ;;
    --dry-run)  DRY=1; shift ;;
    --json)     JSON=1; export JSON; shift ;;
    -h|--help)  usage; exit 0 ;;
    -*)         die "$ERR_USAGE" unknown_flag "unknown flag '$1'" "see --help" ;;
    *)          TASK="${TASK:-$1}"; shift ;;
  esac
done
mkdir -p "$STATE_DIR"

INTAKE="$D/registry/services/verify/intake.sh"
SOW_AUTHOR="$D/registry/features/intake/sow-author.sh"
ROLES="$D/registry/features/roles/roles.sh"
REVIEW="$D/registry/services/review/review.sh"
PIPELINE="$D/registry/services/sdlc/pipeline.sh"
SM="$D/registry/features/statemachine/statemachine.sh"
DISPATCH="$D/registry/services/dispatch/dispatch.sh"
for f in "$INTAKE" "$SOW_AUTHOR" "$ROLES" "$REVIEW" "$PIPELINE" "$DISPATCH"; do
  [ -f "$f" ] || die "$ERR_TOOL_MISSING" missing_component "$f not found" "the tiered tree may have moved; check registry/"
done

# Every stage writes its receipt into the run's own ledger, never the production chain. A measurement
# run that appends to ledger/RECEIPTS.jsonl is how 776 rows once broke it permanently at line 229.
RUN_LEDGER="$STATE_DIR/receipts.jsonl"
export FLEET_LEDGER="${FLEET_LEDGER:-$RUN_LEDGER}"
export FLEET_INTAKE_STATE="${FLEET_INTAKE_STATE:-$STATE_DIR/intake.json}"

# Build the builder's brief from the intake artifacts. The planning stages already produced
# everything an implementer needs — the SOW says what and what-not, atomic.tsv names each leaf with
# its acceptance test, the challenge register says where it will go wrong — and none of it reached
# the agent. `state_code` only ever checked that a CLI existed; the actual coding work was left to
# "the calling agent", which meant nobody.
#
# `owns:` is load-bearing: dispatch refuses a write outside it, and a brief with no owns: line cannot
# be ownership-checked at all.
build_builder_brief() {
  local dest="$1" istate="$2" leaf_paths
  # Own the paths the atomic leaves actually name as outputs, falling back to the repo's source dirs.
  leaf_paths="$(awk -F '\t' 'NR>1 && $2=="feature" {print $1}' "$istate/atomic.tsv" 2>/dev/null | paste -sd, - )"
  {
    printf '# COMPONENT: implement the accepted SOW\n'
    printf 'role: implementation\n'
    # A trailing slash makes it a PREFIX; without it path_is_owned does an EXACT match, so `src`
    # matched only a file literally named "src" and every real write under it was a violation.
    printf 'owns: src/, tests/\n'
    printf 'requirements: %s\n\n' "${leaf_paths:-see atomic.tsv}"
    printf '## The accepted Statement of Work\n\n'
    cat "$istate/sow.md" 2>/dev/null
    printf '\n## Atomic leaves — build each to its stated acceptance\n\n'
    printf 'Each row is independently buildable. Its acceptance column is the test that must pass.\n\n'
    cat "$istate/atomic.tsv" 2>/dev/null
    printf '\n## Known challenges — these are where this task has failed before\n\n'
    printf 'Each cites a real row from this project failure corpus. Do not rediscover them.\n\n'
    cat "$istate/challenges.tsv" 2>/dev/null
    printf '\n## Answered clarifications — decisions already made, do not relitigate\n\n'
    cat "$istate/clarifications.technical.tsv" 2>/dev/null
    cat "$istate/clarifications.business.tsv" 2>/dev/null
    printf '\n## Hard rules\n\n'
    printf -- '- Write ONLY inside the owns: set above. A write outside it is refused.\n'
    printf -- '- Do not push, merge, or open a PR. That gate belongs to the human.\n'
    printf -- '- Every leaf must satisfy its acceptance column, not merely compile.\n'
  } >"$dest"
}

sow_digest() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$FLEET_INTAKE_STATE/sow.md" | awk '{print $1}'
  else
    sha256sum "$FLEET_INTAKE_STATE/sow.md" | awk '{print $1}'
  fi
}

sow_is_accepted() {
  local current recorded
  [ -s "$STATE_DIR/sow.accepted.sha256" ] || return 1
  current="$(sow_digest)"; recorded="$(cat "$STATE_DIR/sow.accepted.sha256" 2>/dev/null)"
  [ -n "$current" ] && [ "$current" = "$recorded" ]
}

show_sow_for_review() {
  printf '%s\n' '== SOW ready: human review required ==' "  path: $FLEET_INTAKE_STATE/sow.md" "  acceptance: ./fleet run --accept-sow" '  request restatement:'
  awk '
    $0 == "## Request restatement" {inside=1; next}
    /^## / && inside {exit}
    inside {print "    " $0}
  ' "$FLEET_INTAKE_STATE/sow.md"
  printf '%s\n' "  refusing to fan out with exit $SOW_REVIEW_EXIT (SOW_READY_AWAITING_REVIEW)"
}

run_sow_author() {
  local author_out="$STATE_DIR/sow-author.out" author_ec
  bash "$SOW_AUTHOR" "$FLEET_INTAKE_STATE" >"$author_out" 2>&1
  author_ec=$?
  cat "$author_out"
  [ "$author_ec" -eq 0 ] || {
    printf 'run: SOW authoring failed (exit %s); role fan-out was not started\n' "$author_ec" >&2
    exit "$author_ec"
  }
}

case "$MODE" in
  status)
    printf 'run[1]{target,intake,roles,stage}:\n'
    printf '  %s,%s,%s,%s\n' "$TARGET" \
      "$( [ -f "$FLEET_INTAKE_STATE" ] && echo open || echo none )" \
      "$(bash "$ROLES" list 2>/dev/null | wc -l | tr -d ' ')" \
      "$( [ -f "$STATE_DIR/plan.tsv" ] && echo fanned-out || echo planning )"
    exit 0 ;;

  answer)
    [ -n "$ANSWER_TEXT" ] || die "$ERR_USAGE" no_answer_text "--answer needs an ID and text" './fleet run --answer scope "the API layer only"'
    bash "$INTAKE" answer "$ANSWER_ID" "$ANSWER_TEXT"
    ec=$?
    [ "$ec" -eq 0 ] || exit "$ec"
    bash "$INTAKE" gate >/dev/null 2>&1
    gec=$?
    if [ "$gec" -eq 0 ]; then
      echo "intake satisfied -- run './fleet run --continue' to fan out the role agents" >&2
    else
      echo "still blocked; './fleet run --status' shows what is open" >&2
    fi
    exit 0 ;;

  accept-sow)
    [ -s "$FLEET_INTAKE_STATE/sow.md" ] || die "$ERR_AMBIGUOUS" sow_missing 'no generated SOW is available to accept' "run ./fleet run --continue first"
    bash "$INTAKE" gate >/dev/null 2>&1
    accept_gate_ec=$?
    [ "$accept_gate_ec" -eq 0 ] || die "$ERR_AMBIGUOUS" sow_not_ready 'the generated intake artifacts are not gate-ready' 'run ./fleet run --continue and inspect the validator failure'
    sow_digest_value="$(sow_digest)"
    [ -n "$sow_digest_value" ] || die "$ERR_GENERIC" sow_hash_failed 'could not hash the SOW acceptance boundary' 'install shasum or sha256sum and retry'
    printf '%s\n' "$sow_digest_value" >"$STATE_DIR/.sow.accepted.sha256.tmp.$$" && mv "$STATE_DIR/.sow.accepted.sha256.tmp.$$" "$STATE_DIR/sow.accepted.sha256" || die "$ERR_GENERIC" sow_acceptance_write_failed 'could not record SOW acceptance' 'check the run state directory permissions'
    printf 'SOW accepted: %s\n' "$FLEET_INTAKE_STATE/sow.md"
    if [ "$DRY" -eq 1 ]; then
      exec "$0" --target "$TARGET" --dry-run --continue
    else
      exec "$0" --target "$TARGET" --continue
    fi
    ;;

  plan)
    [ -n "${TASK:-}" ] || die "$ERR_USAGE" no_task "a task description is required" './fleet run "add rate limiting to the API"'
    echo "== stage 1: plan =="
    # The plan comes BEFORE the questions, deliberately. A fixed questionnaire asked of every request
    # is not understanding: a clarification must be traceable to a gap in the plan.
    bash "$INTAKE" ask "$TASK"
    iec=$?
    echo
    if [ "$iec" -eq 7 ]; then
      echo "== stage 1 verdict: BLOCKED (exit 7) =="
      echo "  fleet will not write code against an ambiguous request. Answer the open questions:"
      echo "    ./fleet run --answer <id> \"your answer\""
      echo "  then:"
      echo "    ./fleet run --continue"
      exit 7
    fi
    echo "intake did not block -- proceed with './fleet run --continue'" >&2
    exit 0 ;;

  continue)
    if ! sow_is_accepted; then
      run_sow_author
      bash "$INTAKE" gate >/dev/null 2>&1
      authored_gate_ec=$?
      [ "$authored_gate_ec" -eq 0 ] || die "$ERR_AMBIGUOUS" sow_not_ready 'SOW artifacts were produced but the intake gate is still blocked' 'inspect sow-author.out and the validator output, then fix the rejected draft'
      show_sow_for_review
      exit "$SOW_REVIEW_EXIT"
    fi

    bash "$INTAKE" gate >/dev/null 2>&1
    gec=$?
    if [ "$gec" -ne 0 ]; then
      die "$ERR_AMBIGUOUS" intake_open "intake is still blocked (exit $gec)" "answer the open questions: ./fleet run --answer <id> \"text\""
    fi

    echo "== stage 2: role fan-out =="
    # Roles are not labels. Each declares the gate it owns and the authority that reviews it, and a
    # role whose gate cannot FAIL is rejected by roles.sh rather than documented.
    : >"$STATE_DIR/plan.tsv"
    bash "$ROLES" list 2>/dev/null | while IFS=$'\t' read -r rid gate reviewer; do
      [ -n "$rid" ] || continue
      bash "$ROLES" check "$rid" >/dev/null 2>&1
      rc=$?
      if [ "$rc" -ne 0 ]; then
        printf '  %-12s SKIPPED  role check failed (exit %s)\n' "$rid" "$rc"
        continue
      fi
      printf '%s\t%s\t%s\n' "$rid" "$gate" "$reviewer" >>"$STATE_DIR/plan.tsv"
      printf '  %-12s gate=%-26s reviewer=%s\n' "$rid" "$gate" "$reviewer"
    done
    [ -s "$STATE_DIR/plan.tsv" ] || die "$ERR_INVARIANT" no_roles "no role passed its own check" "run: bash registry/features/roles/roles.sh list"

    echo
    echo "== stage 3: per-role SDLC =="
    echo "  states: graph edge-cases code test quality security perf maintenance"
    fails=0; passes=0
    while IFS=$'\t' read -r rid gate reviewer; do
      [ -n "$rid" ] || continue
      out="$STATE_DIR/sdlc-$rid.out"
      if [ "$DRY" -eq 1 ]; then
        printf '  %-12s DRY-RUN  (would walk 8 states over %s)\n' "$rid" "$TARGET"
        continue
      fi
      # The builder is the only role that WRITES. It dispatches a real agent into the target with a
      # brief assembled from the accepted plan; every other role verifies.
      if [ "$gate" = implementation ]; then
        brief="$STATE_DIR/brief-$rid.md"
        build_builder_brief "$brief" "$FLEET_INTAKE_STATE"
        FLEET_LEDGER="$STATE_DIR/receipts-$rid.jsonl" FLEET_INTAKE_STATE="$FLEET_INTAKE_STATE" \
          FLEET_REMAINING_MICRO_USD="${FLEET_REMAINING_MICRO_USD:-4000000}" \
          timeout 900 bash "$DISPATCH" run --harness codex --model codex-default \
            --role implementation --budget 40000 --brief "$brief" --target "$TARGET" \
            --timeout 600 >"$STATE_DIR/dispatch-$rid.out" 2>&1
        dec=$?
        printf '  %-12s dispatch exit=%s  brief=%s bytes\n' "$rid" "$dec" "$(wc -c <"$brief" | tr -d ' ')"
        landing="$(sed -n 's/^work_landed: //p' "$STATE_DIR/dispatch-$rid.out" 2>/dev/null | tail -1)"
        [ -n "$landing" ] && printf '  %-12s landed: %s\n' "$rid" "$landing" || printf '  %-12s landed: no commit report\n' "$rid"
      fi
      FLEET_LEDGER="$STATE_DIR/receipts-$rid.jsonl" FLEET_INTAKE_STATE="$FLEET_INTAKE_STATE" \
        timeout 900 bash "$PIPELINE" run --repo "$TARGET" >"$out" 2>&1
      pec=$?
      # A state that is genuinely inapplicable is not a pass and not a failure. Collapsing the three
      # into two is how a gate reads green while having evaluated nothing.
      na="$(grep -c 'result=not-applicable' "$out" 2>/dev/null | tr -d '\n')"; na="${na:-0}"
      rf="$(grep -c 'result=fail' "$out" 2>/dev/null | tr -d '\n')"; rf="${rf:-0}"
      ms="$(grep -c 'result=missing' "$out" 2>/dev/null | tr -d '\n')"; ms="${ms:-0}"
      if [ "$pec" -eq 0 ]; then passes=$((passes+1)); else fails=$((fails+1)); fi
      printf '  %-12s exit=%-2s fail=%s missing=%s n/a=%s  %s\n' "$rid" "$pec" "$rf" "$ms" "$na" "${out#"$D"/}"

      # Stage 4, per role: the output goes to its reviewing authority. A `revise` verdict returns the
      # work to a named state; it does not restart the cycle.
      # This passed the GATE id where a ROLE id is required, and the literal string "review" as the
      # reviewer role. review.sh rejected all of it with exit 6 ("role 'ambiguity' is not
      # registered"), and `|| true` swallowed it — so the reviewer never ran ONCE, for any role, in
      # any run, and zero review receipts were ever written. A swallowed error is not a review.
      # Advance the worker through its OWN lifecycle before submitting. review.sh refused with
      # "worker 'builder' is at 'new'; submission requires verify" — the state machine enforcing
      # order, which is the point of it. An illegal transition is refused, so this cannot fake a
      # state it did not reach: the SDLC result decides verify_pass vs verify_fail.
      if [ -f "$SM" ] && [ "$gate" = implementation ]; then
        for ev in start discover_complete specify_complete design_complete plan_complete; do
          FLEET_LEDGER="$STATE_DIR/receipts-$rid.jsonl" timeout 60 bash "$SM" transition "$rid" "$ev" \
            >>"$STATE_DIR/sm-$rid.out" 2>&1 || true
        done
        # STOP at `verify`. Submission happens AT verify, not after it: `verify_pass` advances to
        # `review`, and submitting from there was refused with "is at 'review'; submission requires
        # verify". The pass/fail transition is recorded AFTER the reviewer has the work.
        printf '  %-12s lifecycle: -> verify\n' "$rid"
      fi

      # review.sh submit carries `work_output`, and the role registry says only `builder` may produce
      # that. The other four roles produce requirements, contracts, verdicts — different artifact
      # kinds this service does not take. Submitting every role's output as work_output was my error;
      # the refusal (exit 6, role_cannot_produce) was the registry being right.
      if [ "$gate" != implementation ]; then
        printf '  %-12s review: n/a (%s produces %s, not work_output)\n' "$rid" "$rid" "$gate"
        printf '%s\t%s\tnot-work-output\n' "$rid" "$reviewer" >>"$STATE_DIR/reviews.tsv"
      elif [ "$reviewer" = human ]; then
        # A human authority cannot be automated. Say so; do not fake a verdict.
        printf '  %-12s review: AWAITING HUMAN (%s) -- read %s\n' "$rid" "$reviewer" "${out#"$D"/}"
        printf '%s\t%s\tawaiting-human\n' "$rid" "$reviewer" >>"$STATE_DIR/reviews.tsv"
      else
        FLEET_LEDGER="$STATE_DIR/receipts-$rid.jsonl" timeout 120 bash "$REVIEW" submit \
          --task-id "run-$(basename "$STATE_DIR")" --worker-id "$rid" --worker-role "$rid" \
          --reviewer-id "$reviewer" --reviewer-role "$reviewer" --output "$out" >"$STATE_DIR/review-$rid.out" 2>&1
        rvc=$?
        printf '  %-12s review: submitted to %s exit=%s\n' "$rid" "$reviewer" "$rvc"
        printf '%s\t%s\t%s\n' "$rid" "$reviewer" "$rvc" >>"$STATE_DIR/reviews.tsv"
        if [ "$pec" -eq 0 ]; then sev=verify_pass; else sev=verify_fail; fi
        FLEET_LEDGER="$STATE_DIR/receipts-$rid.jsonl" timeout 60 bash "$SM" transition "$rid" "$sev" \
          >>"$STATE_DIR/sm-$rid.out" 2>&1
        printf '  %-12s lifecycle: -> %s (exit %s)\n' "$rid" "$sev" "$?"
      fi
    done <"$STATE_DIR/plan.tsv"

    echo
    echo "== verdict =="
    echo "  roles passing SDLC: $passes   failing: $fails"
    echo
    echo "== what fleet did NOT do, and you must =="
    echo "  * nothing was pushed, merged, or opened as a PR. That gate is absolute."
    echo "  * MANUAL VERIFICATION is still yours. Green states are the floor, never the bar:"
    echo "    127 tests here were green while the flagship view rendered nothing at all."
    echo "  * for anything a human looks at, look at it. Run the app, open the page, read the output."
    [ "$fails" -eq 0 ] || exit 1
    exit 0 ;;
esac
