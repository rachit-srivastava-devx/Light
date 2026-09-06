#!/usr/bin/env bash
# verify.sh — the verification service. It composes independent feature adapters:
# memory correction enforcement, the no-mistakes gate, and optional security scanning.
# It never imports another service; dispatch and measurement remain separate services.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
# shellcheck source=../../registry/lib/err.sh
. "$ROOT/registry/lib/err.sh"

usage() {
  printf '%s\n' \
    'verify.sh --intent TEXT [--diff PATH|-] [--scan-root PATH] [--skip-scan]' \
    '  runs the memory correction guard, gate adapter, and optional security scan'
}

INTENT=""
DIFF="-"
SCAN_ROOT="$ROOT"
SKIP_SCAN=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --intent) need_val --intent "${2-}"; INTENT="$2"; shift 2 ;;
    --diff) need_val --diff "${2-}"; DIFF="$2"; shift 2 ;;
    --scan-root) need_val --scan-root "${2-}"; SCAN_ROOT="$2"; shift 2 ;;
    --skip-scan) SKIP_SCAN=1; shift ;;
    --help|-h) usage; exit "$ERR_OK" ;;
    --*) reject_unknown_flag "$1" ;;
    *) reject_unknown_flag "$1" ;;
  esac
done

[ -n "$INTENT" ] || die "$ERR_USAGE" intent_missing \
  'verification requires --intent' 'pass --intent "what changed and why"'

DIFF_FILE="$DIFF"
if [ "$DIFF" = "-" ]; then
  DIFF_FILE="${TMPDIR:-/tmp}/fleet-verify-diff.$$"
  git -C "$ROOT" diff --no-ext-diff --unified=0 >"$DIFF_FILE"
  git_ec=$?
  [ "$git_ec" -eq 0 ] || die "$ERR_GENERIC" diff_failed 'could not obtain the repository diff' 'run from a Git worktree'
fi
cleanup() { [ "$DIFF" = "-" ] && rm -f "$DIFF_FILE"; }
trap cleanup EXIT HUP INT TERM

"$ROOT/registry/features/memory/memory-check.sh" --diff "$DIFF_FILE" </dev/null
memory_ec=$?
[ "$memory_ec" -eq 0 ] || exit "$memory_ec"

FLEET_LEDGER="${FLEET_LEDGER:-${TMPDIR:-/tmp}/fleet-verify-$$.jsonl}" \
  "$ROOT/registry/features/gate/gate.sh" run --intent "$INTENT" </dev/null
gate_ec=$?
[ "$gate_ec" -eq 0 ] || exit "$gate_ec"

if [ "$SKIP_SCAN" -eq 0 ]; then
  FLEET_LEDGER="${FLEET_LEDGER:-${TMPDIR:-/tmp}/fleet-verify-$$.jsonl}" \
    "$ROOT/registry/features/scan/scan.sh" "$SCAN_ROOT" </dev/null
  scan_ec=$?
  [ "$scan_ec" -eq 0 ] || exit "$scan_ec"
fi

exit "$ERR_OK"
