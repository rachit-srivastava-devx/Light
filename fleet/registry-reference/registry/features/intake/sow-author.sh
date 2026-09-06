#!/usr/bin/env bash
# sow-author.sh — dispatch the intake reasoning step, then install its four drafts through intake.sh.
#
# This file owns orchestration, not the planning judgment. The intake agent reads the recorded intent
# and answers, writes four draft files, and this stage submits each one to the existing validator in
# dependency order. A failed validator is reported as-is; this stage never retries or weakens a draft.
set -u

D="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
. "$D/registry/lib/err.sh"

INTAKE="$D/registry/features/intake/intake.sh"
DISPATCH="$D/registry/services/dispatch/dispatch.sh"
STATE="${1:-${FLEET_INTAKE_STATE:-$D/state/intake}}"
BUDGET="${FLEET_INTAKE_BUDGET:-40000}"
TIMEOUT="${FLEET_INTAKE_TIMEOUT:-900}"
BRIEF="$STATE/.sow-author-brief.md"
DISPATCH_LOG="$STATE/.sow-author-dispatch.out"
DRAFT_SOW="$STATE/.sow-author-draft.md"
DRAFT_ATOMIC="$STATE/.sow-author-atomic.tsv"
DRAFT_CHALLENGES="$STATE/.sow-author-challenges.tsv"
DRAFT_CLARIFICATIONS="$STATE/.sow-author-clarifications.tsv"

usage() {
  printf '%s\n' \
    'sow-author.sh [INTAKE_STATE_DIR]' \
    '  Dispatch the marked intake author, validate and install SOW, atomic, challenges, and clarifications.'
}

[ "$#" -le 1 ] || { usage >&2; exit "$ERR_USAGE"; }
[ -f "$INTAKE" ] || die "$ERR_TOOL_MISSING" missing_component "$INTAKE not found" 'check registry/features/intake/'
[ -f "$DISPATCH" ] || die "$ERR_TOOL_MISSING" missing_component "$DISPATCH not found" 'check registry/services/dispatch/'
mkdir -p "$STATE/answers" || die "$ERR_GENERIC" state_write_failed "could not create intake state directory: $STATE" 'check the state directory permissions'
[ -s "$STATE/intent.txt" ] || die "$ERR_AMBIGUOUS" no_intent 'intake authoring needs the recorded intent' 'run ./fleet run "task" first'

export FLEET_INTAKE_STATE="$STATE"

# A completed staged intake does not need another model call. This also makes a second `--continue`
# before human acceptance deterministic: it presents the same SOW instead of silently redrafting it.
GATE_OUT="$(bash "$INTAKE" --json gate 2>&1)"
GATE_EC=$?
if [ "$GATE_EC" -eq 0 ]; then
  printf 'sow-author: four intake artifacts already validate\n'
  exit 0
fi
case "$GATE_OUT" in
  *'"reason":"rubric"'*)
    printf 'sow-author: intake rubric is still blocked; no author dispatch was attempted\n%s\n' "$GATE_OUT" >&2
    exit "$ERR_AMBIGUOUS"
    ;;
esac

# The SOW-author dispatch is the "understand the need and push back" step: it restates the task,
# decomposes it, builds the challenge register, and drafts clarifications. That is planning
# judgment, not implementation (registry/modules/roles/intake-pm.md: model tier opus) -- so it
# must default to the LEAD model, not a worker. Resolved through fleet's own routing
# (registry/modules/cli/model-for.sh) rather than a second, hardcoded opinion of what "lead"
# means, so there is exactly one source of truth for that mapping.
#
# FLEET_INTAKE_HARNESS remains a deliberate override -- forcing codex on purpose (cost, or
# reproducing a past run) still works exactly as before. Only the DEFAULT changed.
#
# A router that cannot resolve must fail loudly, never fall back to a worker silently: that
# silent-downgrade is the exact defect this fixes, so the failure path must not re-introduce it
# by a different route.
if [ -n "${FLEET_INTAKE_HARNESS:-}" ]; then
  HARNESS="$FLEET_INTAKE_HARNESS"
  MODEL="${FLEET_INTAKE_MODEL:-codex-default}"
else
  # FLEET_INTAKE_MODEL_FOR_BIN: same fixture-injection idiom as FLEET_CODEX_BIN/FLEET_CLAUDE_BIN
  # in dispatch.sh -- lets a test point this at a stub to prove the "router unresolvable" failure
  # path without touching registry/modules/cli/model-for.sh itself or spending a token.
  MODEL_FOR="${FLEET_INTAKE_MODEL_FOR_BIN:-$D/registry/modules/cli/model-for.sh}"
  [ -f "$MODEL_FOR" ] || die "$ERR_NOTOOL" lead_router_missing \
    "$MODEL_FOR not found; cannot resolve the lead model for SOW authoring" \
    'set FLEET_INTAKE_HARNESS=codex to bypass routing, or restore registry/modules/cli/model-for.sh'
  LEAD_RAW="$(bash "$MODEL_FOR" lead 2>/dev/null)"; LEAD_EC=$?
  LEAD_RAW="$(printf '%s' "$LEAD_RAW" | tr -d '[:space:]')"
  [ "$LEAD_EC" -eq 0 ] && [ -n "$LEAD_RAW" ] || die "$ERR_NOTOOL" lead_model_unresolved \
    "model-for lead did not resolve a lead model (exit $LEAD_EC)" \
    'set FLEET_INTAKE_HARNESS=codex to force the worker model explicitly, or repair model-for.sh'
  case "$LEAD_RAW" in
    codex)             HARNESS=codex;  LEAD_MODEL=codex-default ;;
    opus|sonnet|haiku) HARNESS=claude; LEAD_MODEL="$LEAD_RAW" ;;
    claude)            HARNESS=claude; LEAD_MODEL=claude-default ;;
    *) die "$ERR_NOTOOL" lead_model_unrecognized \
         "model-for lead resolved an unrecognized model '$LEAD_RAW'" \
         'set FLEET_INTAKE_HARNESS explicitly to bypass routing' ;;
  esac
  MODEL="${FLEET_INTAKE_MODEL:-$LEAD_MODEL}"
fi

intent_hash="$(if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$STATE/intent.txt" | awk '{print $1}'; else sha256sum "$STATE/intent.txt" | awk '{print $1}'; fi)"
[ -n "$intent_hash" ] || die "$ERR_GENERIC" intent_hash_failed 'could not hash intent.txt for the author brief' 'install shasum or sha256sum and retry'

rm -f "$DRAFT_SOW" "$DRAFT_ATOMIC" "$DRAFT_CHALLENGES" "$DRAFT_CLARIFICATIONS" "$DISPATCH_LOG"
{
  printf '%s\n' '# COMPONENT: T50 intake SOW author' 'role: intake' 'sow-author-stage: true' "owns: $STATE" \
    'requirements: produce four validator-approved intake drafts; do not edit implementation code' '' \
    '## Recorded intent' ''
  cat "$STATE/intent.txt"
  printf '%s\n' '' "source_intent_hash: $intent_hash" '' '## Recorded answers'
  for answer in "$STATE"/answers/*.txt; do
    [ -f "$answer" ] || continue
    answer_id="$(basename "$answer" .txt)"
    printf '%s\n' "### $answer_id"
    cat "$answer"
    printf '\n'
  done
  printf '%s\n' \
    '## Required outputs' \
    "Write these four files in the intake state directory, with no prose-only substitute:" \
    "- $DRAFT_SOW" \
    "- $DRAFT_ATOMIC" \
    "- $DRAFT_CHALLENGES" \
    "- $DRAFT_CLARIFICATIONS" \
    '' \
    'The orchestrator will install each file through intake.sh. Do not call intake.sh yourself.' \
    'Read the intent and every recorded answer before drafting. The SOW must prove comprehension of this exact request.' \
    'Use the exact validator contracts: source_intent_hash, request, six non-empty SOW sections, measurable threshold, and explicit non-goals.' \
    'Use atomic.tsv with the exact eight-column header, feature leaves with parents=-, and only design_decision=none.' \
    'Use a real FAILURE-CORPUS:<row> or THREAD-LESSONS:<row> source and point affected_leaf at a feature id.' \
    'Make every clarification trace to sow:<text>, atomic:<leaf-id>, or challenge:<id>; avoid generic questions.' \
    'Use blocking=no with a concrete answer for residual questions so the deterministic code-start gate can open.' \
    'Do not use unresolved placeholders or design-decision words rejected by the validators.'
} > "$BRIEF" || die "$ERR_GENERIC" state_write_failed "could not write author brief: $BRIEF" 'check the intake state directory permissions'

FLEET_SOW_AUTHOR_STAGE=1 \
FLEET_INTAKE_STATE="$STATE" \
FLEET_REMAINING_MICRO_USD="${FLEET_REMAINING_MICRO_USD:-4000000}" \
  bash "$DISPATCH" run --harness "$HARNESS" --model "$MODEL" --role intake --budget "$BUDGET" \
    --brief "$BRIEF" --timeout "$TIMEOUT" --skip-intake-gate >"$DISPATCH_LOG" 2>&1
dispatch_ec=$?
cat "$DISPATCH_LOG"
[ "$dispatch_ec" -eq 0 ] || {
  printf 'sow-author: intake agent dispatch failed (exit %s); no validator retry was attempted\n' "$dispatch_ec" >&2
  exit "$dispatch_ec"
}

for draft in "$DRAFT_SOW" "$DRAFT_ATOMIC" "$DRAFT_CHALLENGES" "$DRAFT_CLARIFICATIONS"; do
  [ -s "$draft" ] || {
    printf 'sow-author: intake agent did not produce required draft: %s\n' "$draft" >&2
    exit "$ERR_PARSE"
  }
done

install_stage() {
  local stage="$1" draft="$2" output ec
  output="$(bash "$INTAKE" "$stage" "$draft" 2>&1)"
  ec=$?
  printf '%s\n' "$output"
  if [ "$ec" -ne 0 ]; then
    printf 'sow-author: validator rejected %s (%s) with exit %s\n%s\n' "$stage" "$draft" "$ec" "$output" >&2
    return "$ec"
  fi
}

install_stage sow "$DRAFT_SOW" || exit $?
install_stage atomic "$DRAFT_ATOMIC" || exit $?
install_stage challenges "$DRAFT_CHALLENGES" || exit $?
install_stage clarifications "$DRAFT_CLARIFICATIONS" || exit $?

printf 'sow-author: all four artifacts installed through intake.sh\n'
exit 0
