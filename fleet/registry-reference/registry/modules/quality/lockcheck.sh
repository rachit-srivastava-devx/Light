#!/usr/bin/env bash
# lockcheck.sh — tamper-evident acceptance-check authority chain.
# The receipt ledger is the source of truth: draft -> adversarial ratify -> human
# sign. A ratification cannot pass without an explicit wrong-code result supplied
# by the verifier (FLEET_RATIFY_RESULT or FLEET_RATIFY_REPORT). This tool does not
# execute arbitrary code, identify a human, or infer that a model tried anything.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/toon.sh"
. "$D/registry/lib/receipt.sh"
. "$D/registry/lib/err.sh"
STATE="${FLEET_LOCK_STATE:-$D/state/lockcheck}"
RECEIPTS="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
NOW="${FLEET_NOW:-}"
JSON=0
FULL=0
LOCK_HELD=0
FLEET_LEDGER="$RECEIPTS"
trap '[ "$LOCK_HELD" -eq 1 ] && rmdir "$STATE/.lock" 2>/dev/null || true' EXIT HUP INT TERM

usage() {
  printf '%s\n' \
    'lockcheck.sh [--json] [--full] [--now ISO] [draft|ratify|sign|verify|status]' \
    '  draft FILE                         register a four-check draft' \
    '  ratify FILE --by MODEL             record explicit adversarial result' \
    '  sign FILE                           record the human signature' \
    '  verify FILE                         recompute the signed content hash' \
    '  status                              show one row per registered check file'
}

now_iso() {
  [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ
}

require_paths() {
  require_bin jq 'install jq and retry'
  require_bin shasum 'install shasum and retry'
}

acquire_lock() {
  mkdir -p "$STATE"
  local n=0
  while ! mkdir "$STATE/.lock" 2>/dev/null; do
    n=$((n+1))
    [ "$n" -ge 100 ] && die "$ERR_GENERIC" lockcheck_locked \
      'could not acquire lockcheck state lock' 'retry after the concurrent writer finishes'
    sleep 0.1
  done
  LOCK_HELD=1
}

valid_model() {
  case "${1-}" in
    ''|*[!A-Za-z0-9._-]*) return 1 ;;
    *) return 0 ;;
  esac
}

canonical_file() {
  local f="$1"
  [ -f "$f" ] || die "$ERR_USAGE" check_file_missing \
    'check file is not a regular file' 'pass an existing acceptance-check file'
  (cd "$(dirname "$f")" && printf '%s/%s' "$(pwd -P)" "$(basename "$f")")
}

hash_file() {
  shasum -a 256 "$1" | awk '{print $1}'
}

chain_guard() {
  FLEET_LEDGER="$RECEIPTS" receipt_verify_chain >/dev/null 2>&1
  local ec=$?
  [ "$ec" -eq 0 ] || die "$ERR_CHAIN" receipt_chain_broken \
    'receipt chain is broken; no lock transition is allowed' \
    'repair the receipt ledger before retrying'
}

append_event() {
  local event="$1" path="$2" gen="${3-}" ver="${4-}" ec="${5:-0}" h
  h="$(FLEET_LEDGER="$RECEIPTS" receipt_append "$(now_iso)" "$event" lockcheck "$path" "$gen" "$ver" "$ec" 0 0 0)"
  local append_ec=$?
  [ "$append_ec" -eq 0 ] || die "$append_ec" receipt_append_failed \
    'could not append the lock receipt' 'check the ledger path and its lock'
  printf '%s' "$h"
}

draft_model() {
  awk -F': ' '/^drafting_model: /{print $2; exit}' "$1"
}

draft_hash() {
  jq -r -s --arg p "$1" \
    'map(select(.component=="lockcheck" and .task_id==$p and (.event|startswith("lock_draft:"))))|last|(.event|split(":")[1]) // empty' \
    "$RECEIPTS" 2>/dev/null
}

draft_gen() {
  jq -r -s --arg p "$1" \
    'map(select(.component=="lockcheck" and .task_id==$p and (.event|startswith("lock_draft:"))))|last|(.event|split(":")[2]) // empty' \
    "$RECEIPTS" 2>/dev/null
}

ratify_data() {
  jq -r -s --arg p "$1" \
    'map(select(.component=="lockcheck" and .task_id==$p and (.event|startswith("lock_ratify:"))))|last|if . then (.event|split(":")|[.[1],.[2],.[3]]|@tsv) else empty end' \
    "$RECEIPTS" 2>/dev/null
}

sign_hash() {
  jq -r -s --arg p "$1" \
    'map(select(.component=="lockcheck" and .task_id==$p and (.event|startswith("lock_sign:"))))|last|(.event|split(":")[1]) // empty' \
    "$RECEIPTS" 2>/dev/null
}

registered_file() {
  [ -s "$RECEIPTS" ] && jq -s -e --arg p "$1" \
    'any(.[]; .component=="lockcheck" and .task_id==$p and (.event|startswith("lock_draft:")))' \
    "$RECEIPTS" >/dev/null 2>&1
}

validate_draft() {
  local f="$1" model count
  model="$(draft_model "$f")"
  valid_model "$model" || die "$ERR_PARSE" drafting_model_missing \
    'draft must contain drafting_model: MODEL' 'add a stable drafting_model header'
  count="$(awk '/^check_[0-9]+: /{n++; seen[$1]++} END{bad=0;for(k in seen)if(seen[k]!=1)bad=1;if(n!=4||bad)exit 1;print n}' "$f" 2>/dev/null)"
  local count_ec=$?
  [ "$count_ec" -eq 0 ] && [ "$count" = 4 ] || die "$ERR_PARSE" checks_unparseable \
    'draft must contain exactly four unique check_N lines' 'write check_1 through check_4 exactly once'
}

draft_cmd() {
  local file="$1" path h old
  path="$(canonical_file "$file")"
  case "$path" in
    *'|'*|*$'\n'*) die "$ERR_USAGE" unsafe_check_path \
      'check path contains a receipt delimiter' 'use a path without pipe or newline characters' ;;
  esac
  validate_draft "$path"
  h="$(hash_file "$path")"
  chain_guard
  acquire_lock
  old="$(draft_hash "$path")"
  if [ -n "$old" ] && [ "$old" = "$h" ]; then
    if [ "$JSON" -eq 1 ]; then
      jq -cn --arg file "$path" '{ok:true,changed:false,status:"draft",file:$file}'
    else
      toon_preamble lockcheck 'acceptance-check draft registration' "$(now_iso)"
      toon_open check 1 'file,status'
      toon_row "$path" draft
      toon_next "lockcheck.sh ratify $path --by DIFFERENT_MODEL"
    fi
    return 0
  fi
  [ -n "$(sign_hash "$path")" ] && die "$ERR_INVARIANT" locked_check_changed \
    'a locked check cannot be replaced by a new draft' 'create a new check file and run the authority chain again'
  append_event "lock_draft:$h:$(draft_model "$path")" "$path" "$(draft_model "$path")" '' 0 >/dev/null
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg file "$path" --arg hash "$h" \
      '{ok:true,changed:true,status:"draft",file:$file,hash:$hash}'
  else
    toon_preamble lockcheck 'acceptance-check draft registration' "$(now_iso)"
    toon_open check 1 'file,status,hash'
    toon_row "$path" draft "$h"
    toon_next "lockcheck.sh ratify $path --by DIFFERENT_MODEL"
  fi
}

ratify_cmd() {
  local file="$1" by="$2" path h dh dm result outcome
  path="$(canonical_file "$file")"
  valid_model "$by" || die "$ERR_USAGE" invalid_model \
    'ratifying model must be a stable name' 'use letters, digits, dot, underscore, or hyphen'
  chain_guard
  registered_file "$path" || die "$ERR_USAGE" draft_missing \
    'check file has not been drafted' 'run lockcheck.sh draft FILE first'
  h="$(hash_file "$path")"
  dh="$(draft_hash "$path")"
  [ "$h" = "$dh" ] || die "$ERR_INVARIANT" draft_drifted \
    'check file changed after draft registration' 'draft the unchanged file again before ratifying'
  dm="$(draft_gen "$path")"
  invariant_i1 "$dm" "$by"
  local i1_ec=$?
  [ "$i1_ec" -eq 0 ] || exit "$ERR_INVARIANT"
  result="${FLEET_RATIFY_RESULT:-}"
  if [ -z "$result" ] && [ -n "${FLEET_RATIFY_REPORT:-}" ]; then
    [ -f "$FLEET_RATIFY_REPORT" ] || die "$ERR_PARSE" ratify_report_missing \
      'ratification report file is missing' 'write a report with outcome: wrong_code_rejected'
    result="$(awk -F': ' '/^outcome: /{print $2;exit}' "$FLEET_RATIFY_REPORT")"
  fi
  case "$result" in
    wrong_code_rejected|rejected|pass|passed) outcome=passed ;;
    wrong_code_passed|accepted|fail|failed) outcome=failed ;;
    *) outcome=unreported ;;
  esac
  append_event "lock_ratify:$h:$by:$outcome" "$path" "$dm" "$by" \
    "$([ "$outcome" = passed ] && printf 0 || printf 6)" >/dev/null
  [ "$outcome" = passed ] || die "$ERR_PARSE" ratification_failed \
    'ratification did not prove that wrong code fails the checks' \
    'have the different model actively try wrong code and report outcome: wrong_code_rejected'
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg file "$path" --arg by "$by" \
      '{ok:true,status:"ratified",file:$file,ratified_by:$by}'
  else
    toon_preamble lockcheck 'adversarial ratification' "$(now_iso)"
    toon_open check 1 'file,status,ratifier'
    toon_row "$path" ratified "$by"
    toon_next "lockcheck.sh sign $path"
  fi
}

human_signer() {
  local identity identity_ec
  identity="$(git -C "$D" config --get user.email 2>/dev/null)"
  identity_ec=$?
  if [ "$identity_ec" -eq 0 ] && [ -n "$identity" ]; then
    printf 'human:%s' "$identity"
  else
    printf 'unverified-human'
  fi
}

sign_cmd() {
  local file="$1" path h dh rd signed dm signer
  path="$(canonical_file "$file")"
  chain_guard
  registered_file "$path" || die "$ERR_USAGE" draft_missing \
    'check file has not been drafted' 'run lockcheck.sh draft FILE first'
  h="$(hash_file "$path")"
  dh="$(draft_hash "$path")"
  [ "$h" = "$dh" ] || die "$ERR_INVARIANT" draft_drifted \
    'check file changed after draft registration' 'draft the unchanged file again before signing'
  signed="$(sign_hash "$path")"
  if [ -n "$signed" ] && [ "$signed" = "$h" ]; then
    toon_preamble lockcheck 'human signature' "$(now_iso)"
    toon_open check 1 'file,status'
    toon_row "$path" LOCKED
    toon_next "lockcheck.sh verify $path"
    return 0
  fi
  rd="$(ratify_data "$path")"
  [ -n "$rd" ] || die "$ERR_INVARIANT" ratification_missing \
    'signature refused: adversarial ratification is missing' \
    'ratify with a different model before signing'
  IFS='	' read -r dm rd_model rd_outcome <<EOF
$rd
EOF
  [ "$dm" = "$h" ] && [ "$rd_outcome" = passed ] || die "$ERR_INVARIANT" ratification_missing \
    'signature refused: the current draft is not successfully ratified' \
    'ratify the unchanged file with wrong_code_rejected before signing'
  signer="$(human_signer)"
  append_event "lock_sign:$h:$signer" "$path" "$dm" human 0 >/dev/null
  toon_preamble lockcheck 'human signature' "$(now_iso)"
  toon_open check 1 'file,status'
  toon_row "$path" LOCKED
  toon_next "lockcheck.sh verify $path"
}

verify_cmd() {
  local file="$1" path h sh
  path="$(canonical_file "$file")"
  chain_guard
  sh="$(sign_hash "$path")"
  [ -n "$sh" ] || die "$ERR_INVARIANT" not_locked \
    'check file has no human signature' 'complete ratification and sign before verify'
  h="$(hash_file "$path")"
  if [ "$h" != "$sh" ]; then
    append_event "lock_verify:$h:drifted" "$path" '' human 6 >/dev/null
    die "$ERR_INVARIANT" check_drifted "LOCKED check file drifted: $path" \
      'restore the signed bytes or start a new authority chain'
  fi
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg file "$path" --arg hash "$h" \
      '{ok:true,status:"LOCKED",file:$file,hash:$hash}'
  else
    toon_preamble lockcheck 'tamper-evident check verification' "$(now_iso)"
    toon_open check 1 'file,status,hash'
    toon_row "$path" LOCKED "$h"
    toon_next 'lockcheck.sh status'
  fi
}

status_cmd() {
  local tmp rows path dh dm rd rh ro sh h st reason n=0
  chain_guard
  tmp="$STATE/.paths.tmp.$$"
  rows="$STATE/.rows.tmp.$$"
  mkdir -p "$STATE"
  : > "$tmp"
  : > "$rows"
  [ -s "$RECEIPTS" ] && jq -r \
    'select(.component=="lockcheck" and (.event|startswith("lock_draft:")))|.task_id' \
    "$RECEIPTS" 2>/dev/null | awk '!seen[$0]++' > "$tmp" || true
  if [ -s "$tmp" ]; then
    while IFS= read -r path; do
      [ -n "$path" ] || continue
      n=$((n+1))
      dh="$(draft_hash "$path")"
      dm="$(draft_gen "$path")"
      rd="$(ratify_data "$path")"
      rh=""
      ro=""
      rd_model=""
      if [ -n "$rd" ]; then
        IFS='	' read -r rh rd_model ro <<EOF
$rd
EOF
      fi
      sh="$(sign_hash "$path")"
      h=""
      [ -f "$path" ] && h="$(hash_file "$path")"
      st=draft
      reason='awaiting ratification'
      [ -n "$h" ] && [ "$h" != "$dh" ] && { st=DRIFTED; reason='content hash differs from draft'; }
      [ "$st" = draft ] && [ -n "$rh" ] && [ "$rh" = "$dh" ] && [ "$ro" = passed ] && {
        st=ratified
        reason='wrong code rejected'
      }
      [ "$st" = ratified ] && [ -n "$sh" ] && [ "$sh" = "$dh" ] && {
        st=LOCKED
        reason='human signature present'
      }
      [ -n "$sh" ] && [ -n "$h" ] && [ "$sh" != "$h" ] && {
        st=DRIFTED
        reason='content hash differs from signature'
      }
      printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$path" "$st" "$dh" "$rd_model" "$sh" "$reason" >> "$rows"
    done < "$tmp"
  fi
  if [ "$JSON" -eq 1 ]; then
    jq -Rn '[inputs|split("\t")|{file:.[0],status:.[1],draft_hash:.[2],ratifier:.[3],signed_hash:.[4],reason:.[5]}]' < "$rows"
  else
    toon_preamble lockcheck 'acceptance-check lock status' "$(now_iso)"
    if [ "$n" -eq 0 ]; then
      toon_empty checks
    else
      if [ "$FULL" -eq 1 ]; then
        toon_open checks "$n" 'file,status,draft_hash,ratifier,signed_hash,reason'
      else
        toon_open checks "$n" 'file,status,draft_hash'
      fi
      while IFS='	' read -r path st dh rd_model sh reason; do
        if [ "$FULL" -eq 1 ]; then
          toon_row "$path" "$st" "$dh" "$rd_model" "$sh" "$reason"
        else
          toon_row "$path" "$st" "$dh"
        fi
      done < "$rows"
    fi
    toon_next 'lockcheck.sh verify FILE' 'lockcheck.sh draft FILE'
  fi
  rm -f "$tmp" "$rows"
}

main() {
  local cmd=""
  require_paths
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --help|-h) usage; return 0 ;;
      --json) need_val --json "${2-}"; JSON=1; shift ;;
      --full) need_val --full "${2-}"; FULL=1; shift ;;
      --now) need_val --now "${2-}"; NOW="$2"; shift 2 ;;
      --*) reject_unknown_flag "$1" ;;
      *) cmd="$1"; shift; break ;;
    esac
  done
  case "$cmd" in
    '') status_cmd ;;
    status) [ "$#" -eq 0 ] || reject_unknown_flag "$1"; status_cmd ;;
    draft) [ "$#" -eq 1 ] || die "$ERR_USAGE" draft_args 'draft requires FILE' 'use lockcheck.sh draft FILE'; draft_cmd "$1" ;;
    ratify) [ "$#" -eq 3 ] && [ "$2" = --by ] || die "$ERR_USAGE" ratify_args 'ratify requires FILE --by MODEL' 'use lockcheck.sh ratify FILE --by MODEL'; ratify_cmd "$1" "$3" ;;
    sign) [ "$#" -eq 1 ] || die "$ERR_USAGE" sign_args 'sign requires FILE' 'use lockcheck.sh sign FILE'; sign_cmd "$1" ;;
    verify) [ "$#" -eq 1 ] || die "$ERR_USAGE" verify_args 'verify requires FILE' 'use lockcheck.sh verify FILE'; verify_cmd "$1" ;;
    *) die "$ERR_USAGE" unknown_subcommand "unknown subcommand '$cmd'" 'run lockcheck.sh --help' ;;
  esac
}

main "$@"
