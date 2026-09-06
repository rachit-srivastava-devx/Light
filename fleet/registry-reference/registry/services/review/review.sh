#!/usr/bin/env bash
# review.sh — higher-authority review service.
# Services compose features; this service calls roles, statemachine, and ratchet adapters.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
. "$ROOT/registry/lib/err.sh"
. "$ROOT/registry/lib/receipt.sh"

ROLES="$ROOT/registry/features/roles/roles.sh"
SM="$ROOT/registry/features/statemachine/statemachine.sh"
RATCHET="$ROOT/registry/features/ratchet/ratchet.sh"
LEDGER="${FLEET_LEDGER:-${TMPDIR:-/tmp}/fleet-review.jsonl}"

usage() {
  printf '%s\n' \
    'review.sh submit --task-id TASK --worker-id ID --worker-role ROLE --reviewer-id ID --reviewer-role ROLE --output FILE' \
    'review.sh verdict --task-id TASK --worker-id ID --reviewer-id ID --reviewer-role ROLE --verdict accept|revise|reject --reason TEXT [--measurements FILE --store PATH]'
}

require_runtime() {
  require_bin jq 'install jq and retry'
  require_bin node 'install node and retry; review uses console/server/sdlc.mjs for the retry policy'
  require_bin shasum 'install shasum and retry'
}

retry_limit() {
  local limit node_ec
  limit="$(node --input-type=module -e 'import(process.argv[1]).then(m => process.stdout.write(String(m.RETRY_POLICY.maxAttemptsPerTask)))' "$ROOT/console/server/sdlc.mjs" 2>/dev/null)"
  node_ec=$?
  [ "$node_ec" -eq 0 ] || die "$ERR_PARSE" retry_policy_unreadable 'could not read RETRY_POLICY.maxAttemptsPerTask from console/server/sdlc.mjs' 'repair the retry policy source'
  case "$limit" in ''|*[!0-9]*) die "$ERR_PARSE" retry_policy_unparseable 'RETRY_POLICY.maxAttemptsPerTask is not a positive integer' 'set the policy to a positive integer' ;; esac
  [ "$limit" -gt 0 ] || die "$ERR_PARSE" retry_policy_unparseable 'RETRY_POLICY.maxAttemptsPerTask must be positive' 'set the policy to a positive integer'
  printf '%s' "$limit"
}

role_json() {
  local role="$1" out ec
  out="$("$ROLES" show "$role" 2>/dev/null)"
  ec=$?
  [ "$ec" -eq 0 ] || die "$ec" role_invalid "role '$role' is not registered" 'run registry/features/roles/roles.sh list'
  printf '%s' "$out"
}

validate_review_contract() {
  local worker_role="$1" reviewer_role="$2" worker reviewer expected
  worker="$(role_json "$worker_role")"
  reviewer="$(role_json "$reviewer_role")"
  expected="$(printf '%s' "$worker" | jq -r '.reviewer')"
  [ "$expected" = "$reviewer_role" ] || die "$ERR_INVARIANT" wrong_authority \
    "role '$worker_role' must be reviewed by '$expected', not '$reviewer_role'" \
    'use the reviewing authority declared by the role entity'
  printf '%s' "$worker" | jq -e '(.may_produce|index("work_output")) != null' >/dev/null 2>&1 || die "$ERR_INVARIANT" role_cannot_produce "role '$worker_role' cannot produce work_output" 'use a role with the declared output capability'
  printf '%s' "$reviewer" | jq -e '(.must_consume|index("work_output")) != null and (.cycle_states|index("review")) != null' >/dev/null 2>&1 || die "$ERR_INVARIANT" reviewer_cycle_missing "reviewer role '$reviewer_role' cannot consume work_output through review" 'declare the reviewer cycle in the role entity'
}

state_of() {
  local agent="$1" out ec state
  out="$(FLEET_LEDGER="$LEDGER" "$SM" replay "$agent" 2>&1)"
  ec=$?
  [ "$ec" -eq 0 ] || exit "$ec"
  state="$(printf '%s' "$out" | sed -n 's/.* state=\([^ ]*\).*/\1/p')"
  [ -n "$state" ] || die "$ERR_PARSE" state_unparseable "could not parse state for agent '$agent'" 'run statemachine.sh replay directly'
  printf '%s' "$state"
}

transition() {
  local agent="$1" event="$2" out ec
  out="$(FLEET_LEDGER="$LEDGER" "$SM" transition "$agent" "$event" 2>&1)"
  ec=$?
  [ "$ec" -eq 0 ] || { printf '%s\n' "$out" >&2; exit "$ec"; }
}

append_review_receipt() {
  local event="$1" task="$2" ts h ec
  ts="${FLEET_NOW:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
  h="$(FLEET_LEDGER="$LEDGER" receipt_append "$ts" "$event" review "$task" not-applicable not-applicable 0 0 0 0)"
  ec=$?
  [ "$ec" -eq 0 ] || die "$ec" receipt_append_failed 'review decision could not be recorded' 'repair the receipt ledger and retry'
  printf '%s' "$h"
}

attempts_for() {
  local task="$1" f="$LEDGER"
  [ -s "$f" ] || { printf '0'; return 0; }
  jq -s -r --arg task "$task" '[.[] | select(.component=="review" and .task_id==$task and (.event|startswith("review_submitted")))] | length' "$f"
}

reviewer_cycle_to_review() {
  local reviewer="$1" state
  state="$(state_of "$reviewer")"
  case "$state" in
    new) transition "$reviewer" start; state=discover ;;
  esac
  case "$state" in
    discover) transition "$reviewer" discover_complete; state=specify ;;
  esac
  case "$state" in
    specify) transition "$reviewer" specify_complete; state=design ;;
  esac
  case "$state" in
    design) transition "$reviewer" design_complete; state=plan ;;
  esac
  case "$state" in
    plan) transition "$reviewer" plan_complete; state=verify ;;
  esac
  case "$state" in
    verify) transition "$reviewer" verify_pass; state=review ;;
  esac
  [ "$state" = review ] || die "$ERR_INVARIANT" reviewer_cycle_invalid "reviewer '$reviewer' is at '$state', not review" 'replay the reviewer cycle and repair the task'
}

submit() {
  local task="$1" worker="$2" worker_role="$3" reviewer="$4" reviewer_role="$5" output="$6" limit attempts worker_state reviewer_state
  validate_review_contract "$worker_role" "$reviewer_role"
  [ "$worker" != "$reviewer" ] || die "$ERR_INVARIANT" self_review 'worker and reviewer must be different agent identities' 'use a separate reviewer agent'
  [ -s "$output" ] || die "$ERR_USAGE" output_missing "submitted output is missing or empty: $output" 'write the worker output before submitting it'
  worker_state="$(state_of "$worker")"
  [ "$worker_state" = verify ] || die "$ERR_INVARIANT" output_not_ready "worker '$worker' is at '$worker_state'; submission requires verify" 'complete the worker cycle through plan_complete first'
  reviewer_state="$(state_of "$reviewer")"
  case "$reviewer_state" in new|plan) ;; *) die "$ERR_INVARIANT" reviewer_not_ready "reviewer '$reviewer' is at '$reviewer_state'; expected new or plan" 'finish or reset the reviewer cycle with a new identity' ;; esac
  limit="$(retry_limit)"
  attempts="$(attempts_for "$task")"
  [ "$attempts" -lt "$limit" ] || die "$ERR_INVARIANT" review_exhausted "review loop for task '$task' is exhausted at $attempts/$limit attempts" 'escalate instead of submitting another attempt'
  transition "$worker" verify_pass
  if [ "$reviewer_state" = new ]; then transition "$reviewer" start; fi
  append_review_receipt "review_submitted:$output" "$task" >/dev/null
  printf 'review_submitted task=%s worker=%s reviewer=%s attempt=%s/%s\n' "$task" "$worker" "$reviewer" "$((attempts + 1))" "$limit"
}

verdict() {
  local task="$1" worker="$2" reviewer="$3" reviewer_role="$4" verdict_name="$5" reason="$6" measurements="$7" store="$8"
  local worker_state reviewer_state attempts limit
  [ -n "$reason" ] || die "$ERR_USAGE" reason_missing '--reason is required for every verdict' 'state why the output is accepted, revised, or rejected'
  role_json "$reviewer_role" >/dev/null
  worker_state="$(state_of "$worker")"
  reviewer_state="$(state_of "$reviewer")"
  [ "$worker_state" = review ] || die "$ERR_INVARIANT" worker_not_in_review "worker '$worker' is at '$worker_state'; verdict requires review" 'submit the verified output first'
  reviewer_cycle_to_review "$reviewer"
  case "$verdict_name" in
    accept)
      [ -n "$measurements" ] && [ -n "$store" ] || die "$ERR_USAGE" ratchet_inputs_missing 'accept requires --measurements and --store' 'supply the measured quality bar and protected store'
      FLEET_LEDGER="$LEDGER" "$RATCHET" check --store "$store" --measurements "$measurements" >/dev/null
      transition "$worker" review_accept
      transition "$reviewer" review_accept
      FLEET_LEDGER="$LEDGER" "$RATCHET" accept --store "$store" --task-id "$task" --measurements "$measurements" >/dev/null
      append_review_receipt "review_verdict_accept:$reason" "$task" >/dev/null
      printf 'verdict=accept task=%s reason=%s\n' "$task" "$reason"
      ;;
    revise)
      attempts="$(attempts_for "$task")"
      limit="$(retry_limit)"
      if [ "$attempts" -ge "$limit" ]; then
        transition "$worker" review_exhausted
        transition "$reviewer" review_exhausted
        append_review_receipt "review_escalated:$reason" "$task" >/dev/null
        printf 'escalated task=%s attempts=%s/%s reason=%s\n' "$task" "$attempts" "$limit" "$reason"
        exit "$ERR_INVARIANT"
      fi
      [ "${REENTER_STATE:-plan}" = plan ] || die "$ERR_USAGE" named_state_only 'revise re-enters at named state plan in the declared graph' 'pass --reenter-state plan'
      transition "$worker" review_revise
      transition "$reviewer" review_revise
      append_review_receipt "review_verdict_revise:$reason" "$task" >/dev/null
      printf 'verdict=revise task=%s reentered_state=plan reason=%s attempt=%s/%s\n' "$task" "$reason" "$attempts" "$limit"
      ;;
    reject)
      transition "$worker" review_reject
      transition "$reviewer" review_reject
      append_review_receipt "review_verdict_reject:$reason" "$task" >/dev/null
      printf 'verdict=reject task=%s reason=%s\n' "$task" "$reason"
      ;;
    *) die "$ERR_USAGE" invalid_verdict "unknown verdict '$verdict_name'" 'use accept, revise, or reject' ;;
  esac
}

parse() {
  local command="$1"; shift
  local task="" worker="" worker_role="" reviewer="" reviewer_role="" output="" verdict_name="" reason="" measurements="" store=""
  REENTER_STATE=plan
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --task-id) need_val --task-id "${2-}"; task="$2"; shift 2 ;;
      --worker-id) need_val --worker-id "${2-}"; worker="$2"; shift 2 ;;
      --worker-role) need_val --worker-role "${2-}"; worker_role="$2"; shift 2 ;;
      --reviewer-id) need_val --reviewer-id "${2-}"; reviewer="$2"; shift 2 ;;
      --reviewer-role) need_val --reviewer-role "${2-}"; reviewer_role="$2"; shift 2 ;;
      --output) need_val --output "${2-}"; output="$2"; shift 2 ;;
      --verdict) need_val --verdict "${2-}"; verdict_name="$2"; shift 2 ;;
      --reason) need_val --reason "${2-}"; reason="$2"; shift 2 ;;
      --measurements) need_val --measurements "${2-}"; measurements="$2"; shift 2 ;;
      --store) need_val --store "${2-}"; store="$2"; shift 2 ;;
      --reenter-state) need_val --reenter-state "${2-}"; REENTER_STATE="$2"; shift 2 ;;
      -h|--help) usage; exit "$ERR_OK" ;;
      --*) reject_unknown_flag "$1" ;;
      *) die "$ERR_USAGE" unexpected_arg "unexpected argument '$1'" 'run review.sh --help' ;;
    esac
  done
  [ -n "$task" ] && [ -n "$worker" ] && [ -n "$reviewer" ] || die "$ERR_USAGE" review_ids_missing 'task, worker, and reviewer ids are required' 'pass --task-id, --worker-id, and --reviewer-id'
  require_runtime
  case "$command" in
    submit) [ -n "$worker_role" ] && [ -n "$reviewer_role" ] && [ -n "$output" ] || die "$ERR_USAGE" submit_args 'submit requires worker/reviewer roles and output' 'pass --worker-role, --reviewer-role, and --output'; submit "$task" "$worker" "$worker_role" "$reviewer" "$reviewer_role" "$output" ;;
    verdict) [ -n "$reviewer_role" ] && [ -n "$verdict_name" ] || die "$ERR_USAGE" verdict_args 'verdict requires reviewer role and verdict' 'pass --reviewer-role and --verdict'; verdict "$task" "$worker" "$reviewer" "$reviewer_role" "$verdict_name" "$reason" "$measurements" "$store" ;;
    *) die "$ERR_USAGE" unknown_command "unknown command '$command'" 'run review.sh --help' ;;
  esac
}

[ "$#" -gt 0 ] || { usage; exit "$ERR_USAGE"; }
case "$1" in submit|verdict) parse "$@" ;; -h|--help) usage ;; *) die "$ERR_USAGE" unknown_command "unknown command '$1'" 'run review.sh --help' ;; esac
