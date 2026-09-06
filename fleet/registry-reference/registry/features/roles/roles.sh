#!/usr/bin/env bash
# roles.sh — role entities and the failable-gate admission rule.
# Features are atomic: this adapter depends only on registry/lib/ and its own registry.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
. "$ROOT/registry/lib/err.sh"
REGISTRY="${FLEET_ROLE_REGISTRY:-$ROOT/registry/features/roles/registry.json}"

usage() {
  printf '%s\n' \
    'roles.sh list' \
    'roles.sh show <role-id>' \
    'roles.sh check <role-id>' \
    'roles.sh check-file <manifest.json>'
}

require_runtime() { require_bin jq 'install jq and retry'; }

manifest_check() {
  local file="$1" id gate failure exit_code
  [ -f "$file" ] || die "$ERR_USAGE" manifest_missing "role manifest not found: $file" 'pass a JSON role manifest'
  jq -e . "$file" >/dev/null 2>&1 || die "$ERR_PARSE" manifest_unparseable "role manifest is not valid JSON: $file" 'write a JSON object and retry'
  id="$(jq -er '.id // empty' "$file" 2>/dev/null)" || die "$ERR_INVARIANT" role_invalid 'role id is required' 'add a non-empty id'
  gate="$(jq -er '.gate.id // empty' "$file" 2>/dev/null)" || die "$ERR_INVARIANT" role_rejected "role '$id' rejected: no failable gate is declared" 'declare gate.id, gate.failure, and a non-zero gate.exit_code'
  failure="$(jq -er '.gate.failure // empty' "$file" 2>/dev/null)" || die "$ERR_INVARIANT" role_rejected "role '$id' rejected: gate.failure is empty" 'declare the condition that blocks progress'
  [ -n "$failure" ] || die "$ERR_INVARIANT" role_rejected "role '$id' rejected: gate.failure is empty" 'declare the condition that blocks progress'
  exit_code="$(jq -er '.gate.exit_code' "$file" 2>/dev/null)" || die "$ERR_INVARIANT" role_rejected "role '$id' rejected: gate.exit_code is missing" 'declare the closed exit code for the gate'
  case "$exit_code" in 0|*[!0-9]*) die "$ERR_INVARIANT" role_rejected "role '$id' rejected: gate.exit_code must be a non-zero integer" 'use an exit code from registry/lib/err.sh' ;; esac
  [ "$exit_code" -gt 0 ] && [ "$exit_code" -le 8 ] || die "$ERR_INVARIANT" role_rejected "role '$id' rejected: gate.exit_code '$exit_code' is not a failable closed code" 'use 1-8 from registry/lib/err.sh'
  jq -e '(.may_produce|type=="array") and (.must_consume|type=="array") and (.cycle_states|type=="array") and (.reviewer|type=="string" and length>0)' "$file" >/dev/null 2>&1 || die "$ERR_INVARIANT" role_invalid "role '$id' is missing produces, consumes, cycle_states, or reviewer" 'declare the complete role contract'
  printf 'role_valid id=%s gate=%s exit=%s reviewer=%s\n' "$id" "$gate" "$exit_code" "$(jq -r '.reviewer' "$file")"
}

check_registry() {
  local role="$1" found tmp
  require_runtime
  [ -f "$REGISTRY" ] || die "$ERR_NOTOOL" role_registry_missing "role registry not found: $REGISTRY" 'restore registry/features/roles/registry.json'
  jq -e 'type=="array"' "$REGISTRY" >/dev/null 2>&1 || die "$ERR_PARSE" role_registry_unparseable 'role registry is not a JSON array' 'repair the role registry'
  tmp="$(mktemp "${TMPDIR:-/tmp}/fleet-role.XXXXXX")"
  jq -e --arg role "$role" '.[] | select(.id==$role)' "$REGISTRY" >"$tmp" 2>/dev/null
  found=$?
  if [ "$found" -ne 0 ]; then
    rm -f "$tmp"
    die "$ERR_USAGE" unknown_role "unknown role '$role'" 'run roles.sh list'
  fi
  manifest_check "$tmp"
  rm -f "$tmp"
}

list_roles() {
  require_runtime
  jq -r '.[] | [.id, .gate.id, .reviewer] | @tsv' "$REGISTRY"
}

show_role() {
  local role="$1"
  require_runtime
  jq -e --arg role "$role" '.[] | select(.id==$role)' "$REGISTRY" 2>/dev/null || die "$ERR_USAGE" unknown_role "unknown role '$role'" 'run roles.sh list'
}

[ "$#" -gt 0 ] || { usage; exit "$ERR_USAGE"; }
COMMAND="$1"; shift
case "$COMMAND" in
  list) [ "$#" -eq 0 ] || die "$ERR_USAGE" list_args 'list takes no arguments' 'run roles.sh list'; list_roles ;;
  show|check)
    [ "$#" -eq 1 ] || die "$ERR_USAGE" role_arg "$COMMAND requires exactly <role-id>" "run roles.sh $COMMAND <role-id>"
    if [ "$COMMAND" = show ]; then show_role "$1"; else check_registry "$1"; fi
    ;;
  check-file)
    [ "$#" -eq 1 ] || die "$ERR_USAGE" manifest_arg 'check-file requires exactly <manifest.json>' 'run roles.sh check-file <manifest.json>'
    require_runtime; manifest_check "$1"
    ;;
  -h|--help) usage ;;
  *) die "$ERR_USAGE" unknown_command "unknown command '$COMMAND'" 'run roles.sh --help' ;;
esac
