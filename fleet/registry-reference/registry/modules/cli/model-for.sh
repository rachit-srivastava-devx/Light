#!/usr/bin/env bash
# model-for.sh — compatibility roles plus the evidence-backed learning router.
# The Python adapter reads the real receipt/work evidence and writes only the
# derived router record under state/router; it never writes ledger/RECEIPTS.jsonl.
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/err.sh"
require_bin python3 'install python3 and retry'

# Enforce the shared missing-value contract before argparse sees the arguments.
validate_values() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --kind|--size|--specified|--web|--deterministic|--ledger|--work-dir|--state|--ccusage-json)
        need_val "$1" "${2-}"; shift 2;;
      *) shift;;
    esac
  done
}
validate_values "$@"
exec python3 "$D/registry/modules/cli/model_router.py" "$@"
