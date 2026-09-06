#!/usr/bin/env bash
# err.sh — structured errors + exit-code table enforcement. Source, do not execute.
# Assumes: bash 3.2 (macOS). No associative arrays, no mapfile.
# Does NOT handle: localisation, stderr redirection policy of the caller.
# shellcheck disable=SC2034
set -u

# --- the closed exit-code table (CONTRACT.md is the SSOT; this mirrors it) ---
# 0 ok | 1 generic | 2 usage/unknown-flag | 3 upstream tool missing
# 4 upstream output unparseable | 5 budget/quota refused | 6 invariant violated
# 7 ambiguity unresolved | 8 receipt chain broken
ERR_OK=0; ERR_GENERIC=1; ERR_USAGE=2; ERR_NOTOOL=3; ERR_PARSE=4
ERR_BUDGET=5; ERR_INVARIANT=6; ERR_AMBIGUOUS=7; ERR_CHAIN=8

err_code_valid() {
  case "${1:-}" in 0|1|2|3|4|5|6|7|8) return 0 ;; *) return 1 ;; esac
}

# Keep refusal reporting off the error path's 10-second lock retry budget. This is a
# best-effort preflight only: receipt_append remains the authority for the actual write.
_err_receipt_can_try() {
  local f d parent lock
  f="${FLEET_LEDGER:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/ledger/RECEIPTS.jsonl}"
  d="$(dirname "$f")"
  while [ ! -e "$d" ]; do
    parent="$(dirname "$d")"
    [ "$parent" != "$d" ] || return 1
    d="$parent"
  done
  [ -d "$d" ] && [ -w "$d" ] || return 1
  lock="${f}.lock"
  [ ! -e "$lock" ] || return 1
}

# Best effort by design: a broken ledger must never change the refusal's exit code or
# recurse through die(). Load receipt.sh from this file's location so callers do not
# need to remember the dependency.
_err_receipt() {
  local event="$1" code="$2" mcode="$3" ts receipt_ec err_dir receipt_file
  if ! command -v receipt_append >/dev/null 2>&1; then
    err_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd)" || return 0
    receipt_file="$err_dir/receipt.sh"
    [ -r "$receipt_file" ] || return 0
    . "$receipt_file" >/dev/null 2>/dev/null || return 0
  fi
  command -v receipt_append >/dev/null 2>&1 || return 0
  _err_receipt_can_try || return 0
  ts="${FLEET_NOW:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
  if receipt_append "$ts" "$event" fleet "$mcode" not-applicable not-applicable "$code" 0 0 0 \
    >/dev/null 2>/dev/null; then
    receipt_ec=0
  else
    receipt_ec=$?
  fi
  # receipt_ec is deliberately observed, but never returned to the caller.
  : "$receipt_ec"
  return 0
}

# die <exit_code> <machine_code> <human message> [remedy]
# Emits a TOON error block on stderr, exits with the code. Refuses codes outside the table.
die() {
  local code="${1:-1}" mcode="${2:-unknown}" msg="${3:-unspecified failure}" remedy="${4:-none}"
  # A literal " in msg/remedy would break the quoted TOON field, so downgrade it to '.
  # The replacement MUST come from a variable: \' on the replacement side of ${v//p/r} is
  # emitted literally as backslash-quote (bash 3.2 and 5.x alike), and a bare ' there is a
  # syntax error in bash 3.2 -- both leave the row wrong. See tests/foundation.test.sh.
  local sq="'"
  if ! err_code_valid "$code"; then
    _err_receipt refusal:contract_violation 1 contract_violation
    printf 'error[1]{code,machine,message,remedy}:\n  1,contract_violation,"die() called with exit code %s outside the table","use 0-8 from CONTRACT.md"\n' "${code//\"/$sq}" >&2
    exit 1
  fi
  _err_receipt "refusal:$mcode" "$code" "$mcode"
  printf 'error[1]{code,machine,message,remedy}:\n' >&2
  printf '  %s,%s,"%s","%s"\n' "$code" "$mcode" "${msg//\"/$sq}" "${remedy//\"/$sq}" >&2
  exit "$code"
}

# require_bin <name> <install hint>  -> exit 3 when absent
require_bin() {
  local bin="${1:?require_bin needs a name}" hint="${2:-install it}"
  command -v "$bin" >/dev/null 2>&1 || die "$ERR_NOTOOL" tool_missing "required tool '$bin' not found on PATH" "$hint"
}

# reject_unknown_flag <flag> — AXI principle 6: fail loud, never ignore
reject_unknown_flag() {
  die "$ERR_USAGE" unknown_flag "unknown flag '${1:-}'" "run with --help for the flag list"
}

# need_val <flag> <value> — the fix for a systemic hang.
#
# bash's `shift 2` SILENTLY NO-OPS when fewer than 2 args remain. Combined with the ubiquitous
# `while [ $# -gt 0 ]; do case "$1" in --flag) X="$2"; shift 2;; ...` idiom, a flag passed without
# its value leaves $# unchanged and the loop spins forever. Measured: `compress2.sh apply --level`
# burned 17+ CPU-minutes and IGNORED SIGTERM — in a client's CI that is a wedged runner needing
# manual intervention, which is strictly worse than a crash.
# The idiom appears 125 times across 32 files. Call this before every `shift 2`.
need_val() {
  local flag="${1:?flag}" val="${2-__FLEET_MISSING__}"
  if [ "$val" = "__FLEET_MISSING__" ] || [ -z "$val" ]; then
    die "$ERR_USAGE" missing_flag_value "flag '$flag' requires a value" "pass a value: $flag <value>"
  fi
  case "$val" in --*) die "$ERR_USAGE" missing_flag_value \
    "flag '$flag' was followed by another flag ('$val'), not a value" "pass a value: $flag <value>" ;; esac
}
