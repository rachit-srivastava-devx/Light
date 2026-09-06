#!/usr/bin/env bash
# shellcheck disable=SC2034
# meter.sh — quota evidence and the fail-closed I2 budget gate (dispatch pre-flight admission
# control). Token/cost REPORTING moved to registry/features/telemetry/telemetry.sh (Phoenix + tiktoken own that now —
# see ADOPT.md "agent telemetry"); this file keeps only the budget-gate boundary route.sh,
# dispatch.sh, and detect.sh hard-depend on, which has no Phoenix/OTel equivalent.
#
# Assumptions: quota-axi emits the JSON shape observed by the fleet contracts; token and money
# values are non-negative integers at the gate boundary. The remaining money runway is supplied
# explicitly by FLEET_REMAINING_RUNWAY_MICRO_USD (or --remaining); quota providers expose reset
# metadata, not money runway.
# Does not handle: a filesystem without atomic mkdir locking (receipt.sh owns that
# limitation), or inferring a dollar runway from a percentage quota.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/toon.sh"
. "$D/registry/lib/receipt.sh"
. "$D/registry/lib/err.sh"

NOW="${FLEET_NOW:-}"
JSON=0
FULL=0
CMD=""

usage() {
  printf '%s\n' \
    'meter.sh [--json] [--full] [--now ISO] [quota|gate|next-round|compress|reported|count]' \
    '  (none)                         this help text' \
    '  quota                          normalized quota-axi provider windows' \
    '  gate --cost MICRO [--remaining MICRO]  fail-closed I2 budget gate' \
    '  next-round --remaining MICRO --burn-rate MICRO_PER_MIN  schedule timestamp' \
    '  compress ARGS...                passthrough to registry/features/context/context.sh (count|compress|check)' \
    '  reported                       passthrough to registry/features/metering/meter.sh (ccusage)' \
    '  count ARGS...                  passthrough to registry/features/metering/meter.sh (tiktoken)'
}

now_iso() {
  if [ -n "$NOW" ]; then
    printf '%s' "$NOW"
  else
    date -u +%Y-%m-%dT%H:%M:%SZ
  fi
}

iso_epoch() {
  local value="${1-}" stripped out
  stripped="${value%%.*}"
  stripped="${stripped%Z}"
  out="$(date -j -u -f '%Y-%m-%dT%H:%M:%S' "$stripped" +%s 2>/dev/null || true)"
  if [ -z "$out" ]; then
    out="$(date -u -d "$value" +%s 2>/dev/null || true)"
  fi
  case "$out" in ''|*[!0-9-]*) return 1 ;; esac
  printf '%s' "$out"
}

epoch_iso() {
  local epoch="$1" out
  out="$(date -u -r "$epoch" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)"
  if [ -z "$out" ]; then
    out="$(date -u -d "@$epoch" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)"
  fi
  [ -n "$out" ] || return 1
  printf '%s' "$out"
}

is_uint() {
  case "${1-}" in ''|*[!0-9]*) return 1 ;; esac
  return 0
}

is_int() {
  case "${1-}" in ''|*[!0-9-]*|-) return 1 ;; esac
  return 0
}

usd_to_micro() {
  local raw="${1-}" value intpart fracpart frac6 seventh i carry result
  raw="${raw#\$}"
  raw="${raw//,/}"
  case "$raw" in ''|*-*|*[^0-9.]*|*.*.*) return 1 ;; esac
  intpart="${raw%%.*}"
  fracpart=""
  [ "$raw" = "${raw%%.*}" ] || fracpart="${raw#*.}"
  [ -n "$intpart" ] || intpart=0
  while [ "${intpart#0}" != "$intpart" ] && [ "${#intpart}" -gt 1 ]; do intpart="${intpart#0}"; done
  case "$intpart" in ''|*[!0-9]*) return 1 ;; esac
  case "$fracpart" in ''|*[!0-9]*) [ -z "$fracpart" ] || return 1 ;; esac
  frac6="${fracpart}000000"
  frac6="${frac6%?????????}"
  # The parameter expansion above is intentionally avoided for portability when
  # a shell has unusual glob behavior; take exactly six characters explicitly.
  frac6="${fracpart}000000"
  frac6="${frac6:0:6}"
  while [ "${#frac6}" -lt 6 ]; do frac6="${frac6}0"; done
  seventh="${fracpart:6:1}"
  [ -n "$seventh" ] || seventh=0
  result=$((10#$intpart * 1000000 + 10#$frac6))
  case "$seventh" in 5|6|7|8|9) result=$((result+1)) ;; esac
  printf '%s' "$result"
}

load_ccusage() {
  if [ -n "${FLEET_CCUSAGE_JSON:-}" ]; then
    [ -f "$FLEET_CCUSAGE_JSON" ] || die "$ERR_USAGE" fixture_missing "ccusage fixture not found: $FLEET_CCUSAGE_JSON" "set FLEET_CCUSAGE_JSON to a readable JSON file"
    cat "$FLEET_CCUSAGE_JSON"
    return 0
  fi
  require_bin ccusage 'install ccusage and retry'
  local output ec
  output="$(ccusage blocks --active --json 2>&1)"; ec=$?
  [ "$ec" -eq 0 ] || die "$ERR_GENERIC" ccusage_failed "ccusage failed: $output" "run ccusage blocks --active --json"
  printf '%s' "$output"
}

load_quota() {
  if [ -n "${FLEET_QUOTA_JSON:-}" ]; then
    [ -f "$FLEET_QUOTA_JSON" ] || die "$ERR_USAGE" fixture_missing "quota fixture not found: $FLEET_QUOTA_JSON" "set FLEET_QUOTA_JSON to a readable JSON file"
    cat "$FLEET_QUOTA_JSON"
    return 0
  fi
  require_bin quota-axi 'install quota-axi and retry'
  local output ec
  output="$(quota-axi --json 2>&1)"; ec=$?
  if [ "$ec" -ne 0 ]; then
    jq -cn --arg reason "quota-axi failed: $output" '{generatedAt:"",providers:[{provider:"codex",windows:[],state:{status:"error",error:$reason}}]}'
    return 0
  fi
  printf '%s' "$output"
}

validate_json_object() {
  local payload="$1" label="$2"
  printf '%s' "$payload" | jq -e 'type == "object"' >/dev/null 2>&1 || \
    die "$ERR_PARSE" "${label}_unparseable" "$label did not return a JSON object" "run $label in JSON mode and inspect the raw output"
}

cc_block() {
  local payload="$1" block
  block="$(printf '%s' "$payload" | jq -c '.blocks[0] // empty' 2>/dev/null || true)"
  printf '%s' "$block"
}

cc_number() {
  local block="$1" path="$2" value type
  type="$(printf '%s' "$block" | jq -r "$path | type" 2>/dev/null || true)"
  [ "$type" = number ] || return 1
  value="$(printf '%s' "$block" | jq -r "$path" 2>/dev/null || true)"
  is_uint "$value" || return 1
  printf '%s' "$value"
}

cc_signed_number() {
  local block="$1" path="$2" value type
  type="$(printf '%s' "$block" | jq -r "$path | type" 2>/dev/null || true)"
  [ "$type" = number ] || return 1
  value="$(printf '%s' "$block" | jq -r "$path" 2>/dev/null || true)"
  is_int "$value" || return 1
  printf '%s' "$value"
}

cc_cost_micro() {
  local block="$1" raw
  raw="$(printf '%s' "$block" | jq -r '.costUSD // empty' 2>/dev/null || true)"
  [ -n "$raw" ] || { printf '0'; return 0; }
  usd_to_micro "$raw" || return 1
}

cc_window_metrics() {
  local block="$1" now="$2" total used_in used_out rem_min elapsed total_sec rem_sec pct burn end start now_epoch end_epoch
  used_in="$(cc_number "$block" '.tokenCounts.inputTokens')" || return 1
  used_out="$(cc_number "$block" '.tokenCounts.outputTokens')" || return 1
  total="$(cc_number "$block" '.totalTokens')" || return 1
  rem_min="$(cc_signed_number "$block" '.projection.remainingMinutes')" || return 1
  elapsed="$(cc_number "$block" '.elapsedSeconds // empty' 2>/dev/null || true)"
  if [ -z "$elapsed" ]; then
    start="$(printf '%s' "$block" | jq -r '.startTime // empty' 2>/dev/null || true)"
    end="$(printf '%s' "$block" | jq -r '.endTime // empty' 2>/dev/null || true)"
    start_epoch="$(iso_epoch "$start" 2>/dev/null || true)"
    end_epoch="$(iso_epoch "$end" 2>/dev/null || true)"
    [ -n "$start_epoch" ] && [ -n "$end_epoch" ] || return 1
    total_sec=$((end_epoch-start_epoch))
    rem_sec=$((rem_min*60))
    elapsed=$((total_sec-rem_sec))
  else
    rem_sec=$((rem_min*60)); total_sec=$((elapsed+rem_sec))
    end="$(printf '%s' "$block" | jq -r '.endTime // empty' 2>/dev/null || true)"
    end_epoch="$(iso_epoch "$end" 2>/dev/null || true)"
  fi
  now_epoch="$(iso_epoch "$now" 2>/dev/null || true)"
  [ -n "$now_epoch" ] || return 1
  if [ "$total_sec" -le 0 ]; then
    total_sec=$rem_sec
    [ "$total_sec" -gt 0 ] || total_sec=1
  fi
  if [ "$elapsed" -le 0 ]; then burn=0; else burn=$((total*60/elapsed)); fi
  if [ "$rem_sec" -lt 0 ]; then
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$used_in" "$used_out" "$total" "0" "$burn" "negative_remaining" "$end" "$elapsed" "$rem_sec"
    return 0
  else
    pct=$((rem_sec*100/total_sec))
    [ "$pct" -le 100 ] || pct=100
  fi
  if [ -n "$end_epoch" ] && [ "$now_epoch" -gt "$end_epoch" ]; then
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$used_in" "$used_out" "$total" "unknown" "$burn" "reset_mid_read" "$end" "$elapsed" "$rem_sec"
  else
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$used_in" "$used_out" "$total" "$pct" "$burn" "ok" "$end" "$elapsed" "$rem_sec"
  fi
}

quota_rows() {
  local payload="$1"
  printf '%s' "$payload" | jq -r '
    (.providers // [])[] |
    . as $p |
    if ((.windows // []) | length) == 0 then
      [$p.provider // "unknown", "none", "unknown", (($p.state.status // "unknown") + ":" + ($p.state.error // "no windows")), ($p.state.error // "no windows")] | @tsv
    else
      .windows[] | [$p.provider // "unknown", (.id // .label // "unknown"), ((.percentRemaining // "unknown")|tostring), ($p.state.status // "ok"), (.resetsAt // "unknown")] | @tsv
    end' 2>/dev/null
}

quota_window() {
  local payload="$1" provider="${FLEET_GATE_PROVIDER:-codex}"
  printf '%s' "$payload" | jq -r --arg p "$provider" '
    [(.providers // [])[] | select(.provider == $p) | (.windows // [])[] |
      {id:(.id // .label // "unknown"), percent:(.percentRemaining // "unknown"), reset:(.resetsAt // "unknown")}]
    | sort_by((.percent | if type == "number" then . else 101 end)) | .[0] // empty
    | [.id, (.percent|tostring), .reset] | @tsv' 2>/dev/null
}

emit_missing_report() {
  local source="$1" remedy="$2" ts
  ts="$(now_iso)"
  toon_preamble meter "source unavailable" "$ts"
  toon_open attention 1 'source,reason,remedy'
  toon_row "$source" "binary missing" "$remedy"
  toon_next "$remedy"
  die "$ERR_NOTOOL" tool_missing "required source '$source' not found on PATH" "$remedy"
}

report_quota() {
  local quota="$1" ts="$2" rows count=0 line provider window pct status reset
  rows="$(quota_rows "$quota")"
  if [ "$JSON" -eq 1 ]; then printf '%s\n' "$quota"; return 0; fi
  toon_preamble meter "normalized quota-axi evidence" "$ts"
  if [ -z "$rows" ]; then toon_empty quota; else
    count="$(printf '%s\n' "$rows" | wc -l | tr -d ' ')"
    toon_open quota "$count" 'provider,window,remaining_pct,status'
    while IFS='	' read -r provider window pct status reset; do
      [ -n "${provider:-}" ] || continue
      toon_row "$provider" "$window" "$pct" "$status"
      [ "$FULL" -eq 1 ] && printf '  reset[%s]: %s\n' "$provider/$window" "$reset"
    done <<EOF
$rows
EOF
  fi
  toon_next 'run meter.sh gate --cost MICRO --remaining MICRO before dispatch'
}

gate_receipt() {
  local ts="$1" ec="$2" cost="$3" tid="${FLEET_TASK_ID:-meter-gate}" gen="${FLEET_GENERATOR_MODEL:-meter-cfo}" ver="${FLEET_VERIFIER_MODEL:-quota-axi}"
  receipt_append "$ts" budget_gate meter "$tid" "$gen" "$ver" "$ec" 0 0 0 >/dev/null || return 1
  return 0
}

run_gate() {
  local cost="" remaining="" quota ts window id pct reset state reason
  while [ $# -gt 0 ]; do
    case "$1" in
      --cost) need_val --cost "${2-}"; cost="${2:-}"; shift 2 ;;
      --remaining) need_val --remaining "${2-}"; remaining="${2:-}"; shift 2 ;;
      --now) need_val --now "${2-}"; NOW="${2:-}"; shift 2 ;;
      --json) JSON=1; shift ;;
      --full) FULL=1; shift ;;
      -h|--help) usage; return 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  is_uint "$cost" || die "$ERR_USAGE" invalid_cost '--cost must be a non-negative integer micro-USD value' 'meter.sh gate --cost 250000 --remaining 1000000'
  invariant_i1 "${FLEET_GENERATOR_MODEL:-meter-cfo}" "${FLEET_VERIFIER_MODEL:-quota-axi}" >/dev/null 2>&1 || die "$ERR_INVARIANT" invariant_i1 'generator_model and verifier_model must differ' 'set distinct FLEET_GENERATOR_MODEL and FLEET_VERIFIER_MODEL values'
  if [ -z "$remaining" ]; then remaining="${FLEET_REMAINING_RUNWAY_MICRO_USD:-${FLEET_REMAINING_MICRO_USD:-}}"; fi
  is_uint "$remaining" || { gate_receipt "$(now_iso)" "$ERR_BUDGET" "$cost" || true; die "$ERR_BUDGET" runway_missing 'remaining money runway is not configured' 'set FLEET_REMAINING_RUNWAY_MICRO_USD or pass --remaining MICRO'; }
  ts="$(now_iso)"
  quota="$(load_quota)"
  validate_json_object "$quota" quota-axi
  window="$(quota_window "$quota")"
  if [ -z "$window" ]; then
    gate_receipt "$ts" "$ERR_BUDGET" "$cost" || die "$ERR_GENERIC" receipt_failed 'could not append the budget refusal receipt' 'repair FLEET_LEDGER and retry'
    die "$ERR_BUDGET" quota_window_missing 'no measurable limiting quota window was returned' 'restore quota-axi authentication and connectivity'
  fi
  IFS='	' read -r id pct reset <<EOF
$window
EOF
  state="$(printf '%s' "$quota" | jq -r --arg p "${FLEET_GATE_PROVIDER:-codex}" '[.providers[]? | select(.provider==$p) | .state.status // "unknown"] | .[0] // "unknown"' 2>/dev/null || printf unknown)"
  if [ "$state" = stale ] || [ "$state" = error ] || [ "$state" = auth_required ]; then
    gate_receipt "$ts" "$ERR_BUDGET" "$cost" || die "$ERR_GENERIC" receipt_failed 'could not append the quota refusal receipt' 'repair FLEET_LEDGER and retry'
    die "$ERR_BUDGET" quota_not_fresh "limiting window ${FLEET_GATE_PROVIDER:-codex}/$id is $state and resets at $reset" 'restore quota-axi freshness before dispatch'
  fi
  if ! is_int "$pct"; then
    gate_receipt "$ts" "$ERR_PARSE" "$cost" || true
    die "$ERR_PARSE" quota_percent_unparseable "limiting window ${FLEET_GATE_PROVIDER:-codex}/$id has non-integer remaining percent" 'inspect quota-axi --json'
  fi
  if [ "$pct" -lt 0 ]; then
    gate_receipt "$ts" "$ERR_BUDGET" "$cost" || true
    die "$ERR_BUDGET" quota_exhausted "limiting window ${FLEET_GATE_PROVIDER:-codex}/$id has negative remaining quota and resets at $reset" 'wait for the quota reset'
  fi
  if invariant_i2 "$cost" "$remaining" >/dev/null 2>&1; then
    gate_receipt "$ts" 0 "$cost" || die "$ERR_GENERIC" receipt_failed 'budget passed but receipt append failed' 'repair FLEET_LEDGER; do not dispatch'
    if [ "$JSON" -eq 1 ]; then jq -cn --arg window "${FLEET_GATE_PROVIDER:-codex}/$id" --arg reset "$reset" --argjson cost "$cost" --argjson remaining "$remaining" '{decision:"allow",cost_micro_usd:$cost,remaining_micro_usd:$remaining,limiting_window:$window,resets_at:$reset}'; else
      toon_preamble meter "budget gate" "$ts"; toon_open gate 1 'decision,cost_micro_usd,remaining_micro_usd,window'; toon_row allow "$cost" "$remaining" "${FLEET_GATE_PROVIDER:-codex}/$id"; toon_next 'dispatch only after this allow result';
    fi
    return 0
  fi
  reason="projected cost $cost exceeds remaining runway $remaining; limiting window ${FLEET_GATE_PROVIDER:-codex}/$id resets at $reset"
  gate_receipt "$ts" "$ERR_BUDGET" "$cost" || die "$ERR_GENERIC" receipt_failed 'could not append the budget refusal receipt' 'repair FLEET_LEDGER and retry'
  die "$ERR_BUDGET" budget_refused "$reason" 'run meter.sh next-round before dispatch'
}

run_next_round() {
  local remaining="" burn="" now epoch delay safe
  while [ $# -gt 0 ]; do
    case "$1" in
      --remaining) need_val --remaining "${2-}"; remaining="${2:-}"; shift 2 ;;
      --burn-rate) need_val --burn-rate "${2-}"; burn="${2:-}"; shift 2 ;;
      --now) need_val --now "${2-}"; NOW="${2:-}"; shift 2 ;;
      --json) JSON=1; shift ;;
      -h|--help) usage; return 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  is_uint "$remaining" || die "$ERR_USAGE" invalid_remaining '--remaining must be a non-negative integer micro-USD value' 'meter.sh next-round --remaining 1000000 --burn-rate 1000'
  is_uint "$burn" || die "$ERR_USAGE" invalid_burn_rate '--burn-rate must be a non-negative integer micro-USD/min value' 'meter.sh next-round --remaining 1000000 --burn-rate 1000'
  now="$(now_iso)"; epoch="$(iso_epoch "$now" 2>/dev/null || true)"
  [ -n "$epoch" ] || die "$ERR_USAGE" invalid_now "now is not ISO-8601: $now" 'pass --now 2026-08-22T00:00:00Z'
  if [ "$burn" -eq 0 ]; then delay=0; else delay=$((remaining/burn*60 + (remaining%burn)*60/burn)); fi
  safe="$(epoch_iso $((epoch+delay)) 2>/dev/null || true)"
  [ -n "$safe" ] || die "$ERR_GENERIC" timestamp_failed 'could not format the next-round timestamp' 'use a supported date implementation'
  if [ "$JSON" -eq 1 ]; then jq -cn --arg safe "$safe" --argjson delay "$delay" --argjson remaining "$remaining" --argjson burn "$burn" '{safe_start_at:$safe,seconds_from_now:$delay,remaining_micro_usd:$remaining,burn_micro_usd_per_min:$burn}'; else
    toon_preamble meter "next round schedule" "$now"; toon_open next_round 1 'safe_start_at,seconds_from_now,remaining_micro_usd,burn_micro_usd_per_min'; toon_row "$safe" "$delay" "$remaining" "$burn"; toon_next 'schedule the next round at safe_start_at';
  fi
}

while [ $# -gt 0 ]; do
  case "$1" in
    --json) JSON=1; shift ;;
    --full) FULL=1; shift ;;
    --now) need_val --now "${2-}"; NOW="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    quota|gate|next-round|compress|reported|count) CMD="$1"; shift; break ;;
    *) reject_unknown_flag "$1" ;;
  esac
done

case "$CMD" in
  '') usage ;;
  quota)
    command -v quota-axi >/dev/null 2>&1 || [ -n "${FLEET_QUOTA_JSON:-}" ] || emit_missing_report quota-axi 'install quota-axi and retry'
    _quota="$(load_quota)"; validate_json_object "$_quota" quota-axi; report_quota "$_quota" "$(now_iso)" ;;
  gate) run_gate "$@" ;;
  next-round) run_next_round "$@" ;;
  compress)
    # registry/features/context/context.sh (tiktoken + LLMLingua-2) replaced bin/compress.sh; it has no --json mode,
    # so JSON is not forwarded here. See ADOPT.md §2 "prompt compression".
    exec "$D/registry/features/context/context.sh" "$@" ;;
  reported|count)
    exec "$D/registry/features/metering/meter.sh" "$CMD" "$@" ;;
esac
