#!/usr/bin/env bash
# watch.sh — thin fleet adapter onto firstmate's zero-token watcher and wake scripts.
# It delegates arming, wake draining, and watcher execution; it never reimplements supervision.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/err.sh"
. "$D/registry/lib/receipt.sh"

FM_BIN="${FLEET_FIRSTMATE_BIN:-$D/vendor/firstmate/bin}"
FM_HOME="${FLEET_FIRSTMATE_HOME:-$D/var/fleet/firstmate-home}"
LEDGER="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
TASK_ID="${FLEET_TASK_ID:-watcher}"
NOW="${FLEET_NOW:-}"

usage() {
  printf '%s\n' \
    'watch.sh arm [--restart]' \
    'watch.sh drain [--ack-through SEQUENCE --recovery-generation GENERATION]' \
    'watch.sh watch'
}

now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }

require_firstmate() {
  local script
  [ -d "$FM_BIN" ] || die "$ERR_NOTOOL" firstmate_missing \
    "firstmate bin directory not found at $FM_BIN" 'restore vendor/firstmate/bin'
  for script in fm-watch-arm.sh fm-wake-drain.sh fm-watch.sh; do
    [ -x "$FM_BIN/$script" ] || die "$ERR_NOTOOL" firstmate_incomplete \
      "firstmate watcher script missing: $script" 'restore the vendored firstmate watcher'
  done
  mkdir -p "$FM_HOME"
}

record_receipt() {
  local event="$1" ec="$2" rc
  FLEET_LEDGER="$LEDGER" receipt_append "$(now_iso)" "$event" watcher "$TASK_ID" \
    not-applicable not-applicable "$ec" 0 0 0 >/dev/null
  rc=$?
  [ "$rc" -eq 0 ] || die "$ERR_GENERIC" receipt_failed \
    'watcher receipt could not be recorded' 'repair the temporary FLEET_LEDGER and retry'
}

require_firstmate
cmd="${1:-}"
[ -n "$cmd" ] && shift
args=()
case "$cmd" in
  arm)
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --restart) args+=(--restart); shift;;
        --help|-h) usage; exit "$ERR_OK";;
        --*) reject_unknown_flag "$1";;
        *) reject_unknown_flag "$1";;
      esac
    done
    script=fm-watch-arm.sh; event=watch_arm;;
  drain)
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --ack-through) need_val --ack-through "${2-}"; args+=(--ack-through "$2"); shift 2;;
        --recovery-generation) need_val --recovery-generation "${2-}"; args+=(--recovery-generation "$2"); shift 2;;
        --help|-h) usage; exit "$ERR_OK";;
        --*) reject_unknown_flag "$1";;
        *) reject_unknown_flag "$1";;
      esac
    done
    script=fm-wake-drain.sh; event=watch_drain;;
  watch)
    [ "$#" -eq 0 ] || reject_unknown_flag "$1"
    script=fm-watch.sh; event=watch_run;;
  ''|-h|--help) usage; exit "$ERR_OK";;
  *) reject_unknown_flag "$cmd";;
esac

if [ "${#args[@]}" -eq 0 ]; then
  FM_HOME="$FM_HOME" "$FM_BIN/$script"
else
  FM_HOME="$FM_HOME" "$FM_BIN/$script" "${args[@]}"
fi
ec=$?
record_receipt "$event" "$ec"
exit "$ec"
