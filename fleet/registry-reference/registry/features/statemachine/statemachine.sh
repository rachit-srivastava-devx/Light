#!/usr/bin/env bash
# statemachine.sh — an append-only, receipt-backed state machine for one agent.
# Features are atomic: this adapter depends only on registry/lib/ and does not import another feature.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
. "$ROOT/registry/lib/err.sh"
. "$ROOT/registry/lib/receipt.sh"

usage() {
  printf '%s\n' \
    'statemachine.sh graph' \
    'statemachine.sh transition <agent-id> <event>' \
    'statemachine.sh replay <agent-id>' \
    'statemachine.sh validate <agent-id>' \
    '' \
    'events: start discover_complete specify_complete design_complete plan_complete' \
    '        verify_pass verify_fail review_accept review_revise review_reject' \
    '        deploy_complete observe_complete learn_complete failure_classified' \
    '        failure_fixed failure_escalate review_exhausted'
}

now_iso() {
  if [ -n "${FLEET_NOW:-}" ]; then printf '%s' "$FLEET_NOW"; else date -u +%Y-%m-%dT%H:%M:%SZ; fi
}

valid_agent() {
  case "${1:-}" in ''|*[!A-Za-z0-9_.-]*) return 1 ;; *) return 0 ;; esac
}

valid_event() {
  case "${1:-}" in
    start|discover_complete|specify_complete|design_complete|plan_complete|verify_pass|verify_fail|review_accept|review_revise|review_reject|deploy_complete|observe_complete|learn_complete|failure_classified|failure_fixed|failure_escalate|review_exhausted) return 0 ;;
    *) return 1 ;;
  esac
}

# The transition table is deliberately explicit. There is no default edge and no model hook.
next_state() {
  case "${1:-}:${2:-}" in
    new:start) printf '%s' discover ;;
    discover:discover_complete) printf '%s' specify ;;
    specify:specify_complete) printf '%s' design ;;
    design:design_complete) printf '%s' plan ;;
    plan:plan_complete) printf '%s' verify ;;
    verify:verify_pass) printf '%s' review ;;
    verify:verify_fail) printf '%s' failure ;;
    review:review_accept) printf '%s' deploy ;;
    review:review_revise) printf '%s' plan ;;
    review:review_reject) printf '%s' rejected ;;
    review:review_exhausted) printf '%s' escalated ;;
    deploy:deploy_complete) printf '%s' observe ;;
    observe:observe_complete) printf '%s' learn ;;
    learn:learn_complete) printf '%s' accepted ;;
    failure:failure_classified) printf '%s' diagnose ;;
    diagnose:failure_fixed) printf '%s' plan ;;
    diagnose:failure_escalate) printf '%s' escalated ;;
    *) return 1 ;;
  esac
}

ledger_file() { printf '%s' "${FLEET_LEDGER:-$ROOT/ledger/RECEIPTS.jsonl}"; }

require_runtime() {
  require_bin jq 'install jq and retry'
  require_bin shasum 'install shasum and retry'
}

history_error() {
  local agent="$1" detail="$2"
  die "$ERR_INVARIANT" impossible_history \
    "agent '$agent' has an impossible state history: $detail" \
    'repair the receipts or start a new agent id'
}

# Sets REPLAY_STATE and REPLAY_COUNT. This replays only successful transitions for this agent.
# A failed state-machine receipt is itself an impossible history: transition() never records one.
replay_agent() {
  local agent="$1" f row ev aid from event to expected bad
  REPLAY_STATE=new
  REPLAY_COUNT=0
  f="$(ledger_file)"
  [ -s "$f" ] || return 0

  bad="$(jq -r --arg a "$agent" 'select(.component=="statemachine" and .task_id==$a and (.event|startswith("agent_transition:")) and (.exit_code != 0)) | .event' "$f")"
  [ -z "$bad" ] || history_error "$agent" "a transition receipt has exit_code != 0 ($bad)"

  while IFS= read -r row || [ -n "$row" ]; do
    [ -n "$row" ] || continue
    ev="${row%%$'\t'*}"
    aid="${ev#agent_transition:}"; aid="${aid%%:*}"
    [ "$aid" = "$agent" ] || continue
    ev="${ev#agent_transition:$agent:}"
    from="${ev%%:*}"; ev="${ev#*:}"
    event="${ev%%:*}"; to="${ev#*:}"
    [ -n "$from" ] && [ -n "$event" ] && [ -n "$to" ] || history_error "$agent" 'malformed transition receipt'
    [ "$from" = "$REPLAY_STATE" ] || history_error "$agent" "expected from '$REPLAY_STATE', found '$from'"
    expected="$(next_state "$from" "$event" 2>/dev/null)" || history_error "$agent" "event '$event' is not legal from '$from'"
    [ "$expected" = "$to" ] || history_error "$agent" "event '$event' maps to '$expected', receipt claims '$to'"
    REPLAY_STATE="$to"
    REPLAY_COUNT=$((REPLAY_COUNT + 1))
  done < <(jq -r --arg a "$agent" 'select(.component=="statemachine" and .task_id==$a and (.event|startswith("agent_transition:"))) | [.event, .exit_code] | @tsv' "$f")
}

validate_agent() {
  local agent="$1" chain_ec
  valid_agent "$agent" || die "$ERR_USAGE" invalid_agent 'agent id must contain only letters, digits, dot, underscore, or hyphen' 'use validate <agent-id>'
  FLEET_LEDGER="$(ledger_file)" receipt_verify_chain >/dev/null 2>&1
  chain_ec=$?
  [ "$chain_ec" -eq 0 ] || exit "$chain_ec"
  replay_agent "$agent"
  printf 'valid_history agent=%s state=%s transitions=%s\n' "$agent" "$REPLAY_STATE" "$REPLAY_COUNT"
}

transition_agent() {
  local agent="$1" event="$2" current next append_ec h
  valid_agent "$agent" || die "$ERR_USAGE" invalid_agent 'agent id must contain only letters, digits, dot, underscore, or hyphen' 'use transition <agent-id> <event>'
  valid_event "$event" || die "$ERR_USAGE" invalid_event "unknown event '$event'" 'run statemachine.sh graph for the transition table'
  require_runtime
  validate_agent "$agent" >/dev/null
  current="$REPLAY_STATE"
  next="$(next_state "$current" "$event" 2>/dev/null)" || die "$ERR_INVARIANT" illegal_transition \
    "illegal transition for agent '$agent': state '$current' + event '$event' is not permitted" \
    'follow the declared state graph; do not skip a state'
  h="$(FLEET_LEDGER="$(ledger_file)" receipt_append "$(now_iso)" \
    "agent_transition:$agent:$current:$event:$next" statemachine "$agent" \
    "${FLEET_GENERATOR_MODEL:-not-applicable}" "${FLEET_VERIFIER_MODEL:-not-applicable}" 0 0 0 0)"
  append_ec=$?
  [ "$append_ec" -eq 0 ] || die "$append_ec" receipt_append_failed 'accepted transition could not be recorded' 'repair the receipt ledger and retry'
  printf 'transition agent=%s from=%s event=%s to=%s receipt=%s\n' "$agent" "$current" "$event" "$next" "$h"
}

graph() {
  cat <<'EOF'
state,event,next_state
new,start,discover
discover,discover_complete,specify
specify,specify_complete,design
design,design_complete,plan
plan,plan_complete,verify
verify,verify_pass,review
verify,verify_fail,failure
review,review_accept,deploy
review,review_revise,plan
review,review_reject,rejected
review,review_exhausted,escalated
deploy,deploy_complete,observe
observe,observe_complete,learn
learn,learn_complete,accepted
failure,failure_classified,diagnose
diagnose,failure_fixed,plan
diagnose,failure_escalate,escalated
EOF
}

[ "$#" -gt 0 ] || { usage; exit "$ERR_USAGE"; }
COMMAND="$1"; shift
case "$COMMAND" in
  graph) [ "$#" -eq 0 ] || die "$ERR_USAGE" graph_args 'graph takes no arguments' 'run statemachine.sh graph' ; graph ;;
  replay|validate)
    [ "$#" -eq 1 ] || die "$ERR_USAGE" agent_arg "$COMMAND requires exactly <agent-id>" "run statemachine.sh $COMMAND <agent-id>"
    require_runtime
    validate_agent "$1"
    ;;
  transition)
    [ "$#" -eq 2 ] || die "$ERR_USAGE" transition_args 'transition requires <agent-id> <event>' 'run statemachine.sh transition <agent-id> <event>'
    transition_agent "$1" "$2"
    ;;
  -h|--help) usage ;;
  *) die "$ERR_USAGE" unknown_command "unknown command '$COMMAND'" 'run statemachine.sh --help' ;;
esac
