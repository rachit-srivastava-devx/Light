#!/usr/bin/env bash
# budget.sh — atomic, keyless I2 admission primitive for service callers.
# Quota observation belongs to registry/services/measure; this feature only answers the deterministic
# question every dispatcher needs: projected spend <= declared remaining runway.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
# shellcheck source=../../registry/lib/err.sh
. "$ROOT/registry/lib/err.sh"
# shellcheck source=../../registry/lib/receipt.sh
. "$ROOT/registry/lib/receipt.sh"

usage() { printf '%s\n' 'budget.sh gate --cost MICRO --remaining MICRO'; }
cost=""; remaining=""; json=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    gate) shift ;;
    --cost) need_val --cost "${2-}"; cost="$2"; shift 2 ;;
    --remaining) need_val --remaining "${2-}"; remaining="$2"; shift 2 ;;
    --json) json=1; shift ;;
    --help|-h) usage; exit "$ERR_OK" ;;
    --*) reject_unknown_flag "$1" ;;
    *) reject_unknown_flag "$1" ;;
  esac
done

case "$cost" in ''|*[!0-9]*) die "$ERR_USAGE" invalid_cost '--cost must be a non-negative integer' 'pass --cost MICRO' ;; esac
case "$remaining" in ''|*[!0-9]*) die "$ERR_USAGE" invalid_remaining '--remaining must be a non-negative integer' 'pass --remaining MICRO' ;; esac

if invariant_i2 "$cost" "$remaining" >/dev/null 2>&1; then
  if [ "$json" -eq 1 ]; then
    jq -cn --argjson cost "$cost" --argjson remaining "$remaining" \
      '{decision:"allow",cost_micro_usd:$cost,remaining_micro_usd:$remaining}'
  else
    printf 'budget[1]{decision,cost_micro_usd,remaining_micro_usd}:\n  allow,%s,%s\n' "$cost" "$remaining"
  fi
  exit "$ERR_OK"
fi

if [ "$json" -eq 1 ]; then
  jq -cn --argjson cost "$cost" --argjson remaining "$remaining" \
    '{decision:"refuse",cost_micro_usd:$cost,remaining_micro_usd:$remaining}'
else
  printf 'budget[1]{decision,cost_micro_usd,remaining_micro_usd}:\n  refuse,%s,%s\n' "$cost" "$remaining"
fi
exit "$ERR_BUDGET"
