#!/usr/bin/env bash
# ratchet.sh — monotonic quality measurements backed by a protected external store.
# Features are atomic: this adapter depends only on registry/lib/ and never imports another feature.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
. "$ROOT/registry/lib/err.sh"
. "$ROOT/registry/lib/receipt.sh"

METRICS='bats_pass_count mutation_kill_rate a11y_pass corpus_caught_count scan_findings'

usage() {
  printf '%s\n' \
    'ratchet.sh check --store /protected/path/high-water.json --measurements measurements.json' \
    'ratchet.sh accept --store /protected/path/high-water.json --task-id TASK --measurements measurements.json'
}

require_runtime() { require_bin jq 'install jq and retry'; require_bin shasum 'install shasum and retry'; }

store_path() {
  local store="$1" dir root_abs
  case "$store" in /*) ;; *) die "$ERR_USAGE" store_not_absolute 'ratchet store must be an absolute path outside the checkout' 'pass --store /protected/path/high-water.json' ;; esac
  dir="$(dirname "$store")"
  mkdir -p "$dir"
  root_abs="$ROOT"
  case "$dir/" in
    "$root_abs/"*|"$root_abs") die "$ERR_INVARIANT" ratchet_store_in_tree \
      'ratchet high-water store must live outside the checkout so the failing change cannot lower it' \
      'configure --store in a CODEOWNER-protected external path' ;;
  esac
  RATCHET_STORE_PATH="$store"
}

measurements_json() {
  local file="$1" out
  [ -f "$file" ] || die "$ERR_USAGE" measurements_missing "measurements file not found: $file" 'write the five required integer measurements'
  out="$(jq -cer '
    . as $m |
    if (([$m.bats_pass_count, $m.mutation_kill_rate, $m.a11y_pass, $m.corpus_caught_count, $m.scan_findings] | all(.[]; type=="number" and floor==. and .>=0)) and ($m.mutation_kill_rate <= 100)) then
      {bats_pass_count:$m.bats_pass_count, mutation_kill_rate:$m.mutation_kill_rate, a11y_pass:$m.a11y_pass, corpus_caught_count:$m.corpus_caught_count, scan_findings:$m.scan_findings}
    else empty end
  ' "$file" 2>/dev/null)"
  [ -n "$out" ] || die "$ERR_PARSE" measurements_unparseable \
    'measurements must contain five non-negative integer metrics; mutation_kill_rate must be 0-100' \
    'write valid JSON with bats_pass_count, mutation_kill_rate, a11y_pass, corpus_caught_count, scan_findings'
  MEASUREMENTS_JSON="$out"
}

baseline_metric() {
  local store="$1" metric="$2"
  [ -s "$store" ] || { printf '%s' ''; return 0; }
  jq -er --arg metric "$metric" '.metrics[$metric] // empty' "$store" 2>/dev/null || die "$ERR_PARSE" ratchet_store_unparseable "ratchet store is not valid high-water JSON: $store" 'repair the protected policy store'
}

check_measurements() {
  local store="$1" metrics="$2" metric old new
  for metric in $METRICS; do
    old="$(baseline_metric "$store" "$metric")"
    [ -n "$old" ] || continue
    new="$(printf '%s' "$metrics" | jq -r --arg metric "$metric" '.[$metric]')"
    case "$metric" in
      scan_findings)
        [ "$new" -le "$old" ] || die "$ERR_INVARIANT" ratchet_regression "ratchet refused regression: metric '$metric' increased from $old to $new" 'reduce findings before acceptance' ;;
      *)
        [ "$new" -ge "$old" ] || die "$ERR_INVARIANT" ratchet_regression "ratchet refused regression: metric '$metric' fell from $old to $new" 'meet or exceed the recorded high-water mark' ;;
    esac
  done
}

check_store_shape() {
  local store="$1"
  [ -s "$store" ] || return 0
  jq -e '.version==1 and (.metrics|type=="object") and (.accepted|type=="array")' "$store" >/dev/null 2>&1 || die "$ERR_PARSE" ratchet_store_unparseable "ratchet store has the wrong shape: $store" 'repair the protected policy store'
}

check_gate() {
  local store="$1" measurements="$2" metrics
  require_runtime
  store_path "$store"
  store="$RATCHET_STORE_PATH"
  check_store_shape "$store"
  measurements_json "$measurements"
  metrics="$MEASUREMENTS_JSON"
  check_measurements "$store" "$metrics"
  if [ -s "$store" ]; then
    printf 'ratchet_pass store=%s metrics=%s\n' "$store" "$metrics"
  else
    printf 'ratchet_pass store=%s baseline=none metrics=%s\n' "$store" "$metrics"
  fi
}

accept_gate() {
  local store="$1" task="$2" measurements="$3" metrics candidate tmp ts h receipt_ec mv_ec
  require_runtime
  [ -n "$task" ] || die "$ERR_USAGE" task_missing '--task-id is required for acceptance' 'pass --task-id TASK'
  store_path "$store"
  store="$RATCHET_STORE_PATH"
  check_store_shape "$store"
  measurements_json "$measurements"
  metrics="$MEASUREMENTS_JSON"
  check_measurements "$store" "$metrics"
  ts="${FLEET_NOW:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
  if [ -s "$store" ]; then
    candidate="$(jq -c --arg task "$task" --arg ts "$ts" --argjson metrics "$metrics" \
      '.metrics=$metrics | .accepted += [{task_id:$task,ts:$ts,metrics:$metrics}]' "$store")"
  else
    candidate="$(jq -cn --arg task "$task" --arg ts "$ts" --argjson metrics "$metrics" \
      '{version:1,metrics:$metrics,accepted:[{task_id:$task,ts:$ts,metrics:$metrics}]}')"
  fi
  tmp="$(mktemp "${store}.XXXXXX")"
  printf '%s\n' "$candidate" >"$tmp"
  h="$(FLEET_LEDGER="${FLEET_LEDGER:-${TMPDIR:-/tmp}/fleet-ratchet.jsonl}" \
    receipt_append "$ts" ratchet_accept ratchet "$task" \
    "${FLEET_GENERATOR_MODEL:-not-applicable}" "${FLEET_VERIFIER_MODEL:-not-applicable}" 0 0 0 0)"
  receipt_ec=$?
  if [ "$receipt_ec" -ne 0 ]; then
    rm -f "$tmp"
    die "$receipt_ec" receipt_append_failed 'ratchet passed but its acceptance receipt could not be recorded' 'repair the receipt ledger and retry'
  fi
  mv "$tmp" "$store"
  mv_ec=$?
  [ "$mv_ec" -eq 0 ] || die "$ERR_GENERIC" ratchet_store_write_failed 'acceptance receipt exists but high-water store update failed' 'repair the protected store before accepting more work'
  printf 'ratchet_accept task=%s store=%s receipt=%s metrics=%s\n' "$task" "$store" "$h" "$metrics"
}

parse_args() {
  local command="$1"; shift
  STORE="${FLEET_RATCHET_STORE:-}"
  TASK=""
  MEASUREMENTS=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --store) need_val --store "${2-}"; STORE="$2"; shift 2 ;;
      --task-id) need_val --task-id "${2-}"; TASK="$2"; shift 2 ;;
      --measurements) need_val --measurements "${2-}"; MEASUREMENTS="$2"; shift 2 ;;
      -h|--help) usage; exit "$ERR_OK" ;;
      --*) reject_unknown_flag "$1" ;;
      *) die "$ERR_USAGE" unexpected_arg "unexpected argument '$1'" 'run ratchet.sh --help' ;;
    esac
  done
  [ -n "$STORE" ] || die "$ERR_USAGE" store_missing '--store is required' 'pass an external high-water path'
  [ -n "$MEASUREMENTS" ] || die "$ERR_USAGE" measurements_missing '--measurements is required' 'pass a JSON measurements file'
  case "$command" in
    check) check_gate "$STORE" "$MEASUREMENTS" ;;
    accept) accept_gate "$STORE" "$TASK" "$MEASUREMENTS" ;;
    *) die "$ERR_USAGE" unknown_command "unknown command '$command'" 'run ratchet.sh --help' ;;
  esac
}

[ "$#" -gt 0 ] || { usage; exit "$ERR_USAGE"; }
case "$1" in check|accept) parse_args "$@" ;; -h|--help) usage ;; *) die "$ERR_USAGE" unknown_command "unknown command '$1'" 'run ratchet.sh --help' ;; esac
