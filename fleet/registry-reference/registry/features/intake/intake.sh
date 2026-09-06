#!/usr/bin/env bash
# intake.sh — deterministic, staged intake and code-start gate.
# A model or human may draft the four artifacts. This file only decides whether
# they exist, are linked to the request, and satisfy inspectable contracts.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/toon.sh"; . "$D/registry/lib/receipt.sh"; . "$D/registry/lib/err.sh"
STATE="${FLEET_INTAKE_STATE:-$D/state/intake}"
RECEIPTS="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
GENERATOR_MODEL="${FLEET_GENERATOR_MODEL:-intake-drafter}"
NOW="${FLEET_NOW:-}"; JSON=0; FULL=0; LOCK_HELD=0; FLEET_LEDGER="$RECEIPTS"

cleanup() { [ "$LOCK_HELD" -eq 1 ] && rmdir "$STATE/.lock" 2>/dev/null || true; LOCK_HELD=0; }
trap cleanup EXIT HUP INT TERM
usage() {
  printf '%s\n' \
    'intake.sh [--json] [--full] [--now ISO] [ask|answer|sow|atomic|challenges|clarifications|clarify|gate|blueprint|summary|status]' \
    '  ask INTENT                     open/reset the inspectable rubric' \
    '  answer ID TEXT                 settle one rubric question' \
    '  sow FILE                      install and validate a request-derived SOW' \
    '  atomic FILE                   install and validate tiered atomic.tsv' \
    '  challenges FILE               install corpus-backed challenge register' \
    '  clarifications FILE           install linked business/technical questions' \
    '  clarify ID TEXT               answer one blocking clarification' \
    '  gate                          report whether code may start (exit 7 when blocked)' \
    '  blueprint                     emit the ready, evidence-linked build blueprint' \
    '  summary                       emit the human sign-off summary' \
    '  status                        show rubric and stage state'
}
now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }
require_paths() {
  require_bin jq 'install jq and retry'
  if ! command -v shasum >/dev/null 2>&1 && ! command -v sha256sum >/dev/null 2>&1; then
    die "$ERR_NOTOOL" hash_missing 'need shasum or sha256sum' 'install a SHA-256 implementation and retry'
  fi
}
acquire_lock() {
  mkdir -p "$STATE/answers"; local n=0
  while ! mkdir "$STATE/.lock" 2>/dev/null; do
    n=$((n+1)); [ "$n" -ge 100 ] && die "$ERR_GENERIC" intake_locked 'could not acquire intake state lock' 'retry after the concurrent writer finishes'; sleep 0.1
  done
  LOCK_HELD=1
}
atomic_write() {
  local target="$1" tmp="$2" text="$3"
  printf '%s\n' "$text" > "$tmp" || die "$ERR_GENERIC" state_write_failed 'could not write intake state' 'check the state directory permissions'
  mv "$tmp" "$target" || die "$ERR_GENERIC" state_write_failed 'could not commit intake state' 'check the state directory permissions'
}
hash_file() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'; else sha256sum "$1" | awk '{print $1}'; fi; }
record_stage() { receipt_append "$(now_iso)" "intake_stage:$1" intake "$1" not-applicable not-applicable 0 0 0 0 >/dev/null 2>&1 || true; }
### T52 — question ids. This is the single static catalog answer() validates ids against; it is
### intentionally NOT a read of the live questions.tsv (question_line() below already does that
### per-session check). Keep this list in sync with every id emitted by write_questions().
valid_id() {
  case "${1-}" in
    scope|success|cli_flag|schema_migration|ui_view|api_endpoint|deletion|scale|tenancy|precision|auth|ownership|failure|unhappy|rename_refactor) return 0;;
    *) return 1;;
  esac
}
read_intent() { [ -s "$STATE/intent.txt" ] && cat "$STATE/intent.txt" || true; }
answer_path() { printf '%s/answers/%s.txt' "$STATE" "$1"; }
question_line() { awk -F '\t' -v wanted="$1" '$1==wanted{print; found=1} END{exit found?0:1}' "$STATE/questions.tsv" 2>/dev/null; }
question_answered() { [ -s "$(answer_path "$1")" ]; }
# has_marker: true when ERE `pattern` appears anywhere in `text`, case-insensitively.
has_marker() { local pattern="$1" text="$2"; printf '%s' "$text" | tr '[:upper:]' '[:lower:]' | grep -Eq -- "$pattern"; }
# first_match: the literal (lowercased) substring of `text` that `pattern` first matched, or empty.
first_match() { local pattern="$1" text="$2"; printf '%s' "$text" | tr '[:upper:]' '[:lower:]' | grep -oE -- "$pattern" 2>/dev/null | head -1; }

# emit_core <qfile> <id> <dimension> <question> <reason>
# Unconditional — written no matter what the intent says. Reserved ONLY for the two-item
# irreducible core in write_questions(); a derived question must go through emit_derived instead,
# so its trigger is always a real token match rather than a fixed label standing in for one.
emit_core() { printf '%s\t%s\t%s\tcore:%s\n' "$2" "$3" "$4" "$5" >> "$1"; }

# emit_derived <qfile> <intent_lc> <id> <dimension> <question> <pattern>
# Fires ONLY when <pattern> (a fixed ERE — no model call, no network call) matches the intent.
# No match => no row => no question. This positive-trigger shape is deliberate: the previous
# rubric asked a question whenever a topic marker was ABSENT, which fired for nearly every
# plain-English request (see T52 brief) and produced the identical fixed 11-question list
# PRINCIPLES.md #3 condemns. Asking only on PRESENCE means two differently-worded requests
# naturally produce different question sets instead of the same list every time, and the matched
# token is recorded as the trigger so a human can audit exactly why each question exists.
emit_derived() {
  local qfile="$1" intent_lc="$2" id="$3" dim="$4" question="$5" pattern="$6" token
  token="$(first_match "$pattern" "$intent_lc")"
  [ -n "$token" ] || return 0
  printf '%s\t%s\t%s\ttoken:%s\n' "$id" "$dim" "$question" "$token" >> "$qfile"
}

write_questions() {
  local intent="$1" intent_lc q="$STATE/.questions.tmp.$$"; : > "$q"
  intent_lc="$(printf '%s' "$intent" | tr '[:upper:]' '[:lower:]')"

  # --- irreducible core: asked of every request, regardless of what it is about. ----------------
  # The SOW that would let us trace a question to a specific gap does not exist yet at first
  # contact (sow-author.sh only runs once this rubric closes — see T52 "ordering problem"), so
  # these two cannot themselves be intent-derived. They are the deliberate, small, justified
  # exception, not a rubric in disguise:
  #   scope   - every request needs an explicit boundary or it will either creep past what was
  #             asked or under-deliver against it. True independent of domain.
  #   success - every request needs an observable, falsifiable "done" condition, or completion can
  #             never be verified later. The same invariant this repo's own verify gate enforces
  #             on itself (C17).
  emit_core "$q" scope   'scope boundary'   'What is explicitly in scope, and what boundary must not be crossed?' every_request_needs_a_stated_boundary
  emit_core "$q" success 'success criteria' 'What observable result proves this is done, including the acceptance threshold?' every_request_needs_a_done_condition

  # --- derived: only asked when the intent text itself gives a specific, inspectable reason. ----
  # Each row is a fixed keyword/shape -> question mapping. Add a row here and keep valid_id()'s
  # catalog in sync; do not make any of these fire on absence (see emit_derived's comment above).
  emit_derived "$q" "$intent_lc" cli_flag         'cli flag/option'     'Which exact flag/option name and default value, and does behaviour change for callers who omit it?' '--[a-z][a-z0-9_-]*|\bflags?\b|\boptions?\b|\bswitch\b'
  emit_derived "$q" "$intent_lc" schema_migration 'schema/migration'    'Is the schema/data migration reversible, what happens to in-flight rows during cutover, and is any downtime acceptable?' '\bmigrat[a-z]*\b|\bschema\b|\bbackfill\b|\bddl\b|\bcolumns?\b'
  emit_derived "$q" "$intent_lc" ui_view          'UI surface'          'Which breakpoints/themes (light/dark, mobile/desktop) and accessibility bar must this view meet?' '\bui\b|\bviews?\b|\bscreen\b|\bconsole\b|\bdashboard\b|\bcomponents?\b|\blayout\b|\bmap\b|\brender[a-z]*\b'
  emit_derived "$q" "$intent_lc" api_endpoint     'API contract'        'What is the exact request/response shape, and can this break an existing caller (is it versioned)?' '\bendpoints?\b|\broutes?\b|\bapi\b|\brequest\b|\bresponse\b|\bhttp\b'
  emit_derived "$q" "$intent_lc" deletion         'deletion/cleanup'    'Deletion/cleanup of what exactly - is it recoverable (soft-delete, backup) or permanent, and what must survive?' '\bdelete[a-z]*\b|\bremove[a-z]*\b|\bdrop\b|\bpurge[a-z]*\b|clean ?up'
  emit_derived "$q" "$intent_lc" scale            'scale'               'What peak users, throughput, latency, and growth must the design support?' '\bscale\b|\bthroughput\b|\brps\b|requests per|\busers\b|\blatency\b|\bp95\b|\bload\b|concurren[a-z]*'
  emit_derived "$q" "$intent_lc" tenancy          'tenancy'             'Is this single-tenant or multi-tenant, and what isolation boundary is required?' '\btenant[a-z]*\b|multi-tenant|single-tenant|isolat[a-z]*'
  emit_derived "$q" "$intent_lc" precision        'money and precision' 'Which money, token, and count values require integer representation, and what rounding is allowed?' '\bmoney\b|\bcurrency\b|\bcost\b|\bprice\b|\btokens?\b|\bquota\b|\bcents\b|\bdecimal\b|\bbilling\b'
  emit_derived "$q" "$intent_lc" auth             'auth and access'     'Who may perform each operation, and how are authentication and authorization enforced?' '\bauth[a-z]*\b|\bpermissions?\b|\broles?\b|\baccess\b|\bidentity\b|\bcredentials?\b'
  emit_derived "$q" "$intent_lc" ownership        'data ownership'      'Who owns each input and output, and which system is the source of truth?' '\bpersist[a-z]*\b|\bstores?\b|\bstored\b|\bdatabase\b|\brecords?\b|\bretention\b|\bowners?\b|\bownership\b'
  emit_derived "$q" "$intent_lc" failure          'failure behaviour'   'What must happen on failure, timeout, malformed upstream output, and partial completion?' '\bretry\b|\btimeouts?\b|\bunavailable\b|\bdegraded\b|\brollback\b|\berrors?\b|\bfail[a-z]*\b'
  emit_derived "$q" "$intent_lc" unhappy          'unhappy paths'       'What should happen for empty, malformed, huge, duplicate, or concurrent input?' '\binputs?\b|\bparse[a-z]*\b|\buploads?\b|\bimports?\b|\bfiles?\b|\bform\b|\bpayload\b'
  emit_derived "$q" "$intent_lc" rename_refactor  'rename/refactor'     'What call sites, imports, and references must be updated, and must external behaviour stay identical?' '\brenam[a-z]*\b|\brefactor[a-z]*\b|\brestructur[a-z]*\b|reorganis[a-z]*|reorganiz[a-z]*'

  mv "$q" "$STATE/questions.tsv"
}
open_count() {
  local id dim text trigger n=0; [ -s "$STATE/questions.tsv" ] || { printf '0'; return; }
  while IFS='	' read -r id dim text trigger; do [ -n "$id" ] && ! question_answered "$id" && n=$((n+1)); done < "$STATE/questions.tsv"
  printf '%s' "$n"
}
emit_questions() {
  local intent="$1" n="$2" id dim text trigger status qtmp="$STATE/.open.tmp.$$"; : > "$qtmp"
  if [ -s "$STATE/questions.tsv" ]; then
    while IFS='	' read -r id dim text trigger; do
      [ -n "$id" ] || continue; question_answered "$id" && status=answered || status=open
      [ "$status" = open ] && printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$dim" "$text" "$status" "$trigger" >> "$qtmp"
    done < "$STATE/questions.tsv"
  fi
  if [ "$JSON" -eq 1 ]; then
    jq -Rn --arg intent "$intent" --argjson count "$n" '[inputs|split("\t")|{id:.[0],dimension:.[1],question:.[2],status:.[3],trigger:.[4]}] as $questions | {ok:($count==0),intent:$intent,open_questions:$questions}' < "$qtmp"
  else
    toon_preamble intake 'ambiguity gate: inspectable open questions' "$(now_iso)"
    if [ "$n" -eq 0 ]; then toon_empty questions; else toon_open questions "$n" 'id,dimension,question,status,trigger'; while IFS='	' read -r id dim text status trigger; do toon_row "$id" "$dim" "$text" "$status" "$trigger"; done < "$qtmp"; fi
    [ "$n" -gt 0 ] && toon_next 'intake.sh answer QUESTION_ID "your answer"' 'intake.sh ask "the same intent"' || toon_next 'intake.sh sow SOW.md' 'intake.sh status'
  fi
  rm -f "$qtmp"
}
rubric_gate() {
  local n; [ -e "$STATE/intent.txt" ] || { printf '%s\n' 'no intent has been recorded' >&2; return 1; }
  n="$(open_count)"; [ "$n" -eq 0 ] || { emit_questions "$(read_intent)" "$n"; return 1; }; return 0
}
stage_blocked() { die "$ERR_AMBIGUOUS" "$1" "$3" "complete the '$1' stage and retry"; }

ask() {
  local intent="${1-}" old; acquire_lock; old="$(read_intent)"
  if [ ! -e "$STATE/intent.txt" ] || [ "$old" != "$intent" ]; then
    mkdir -p "$STATE/answers"
    rm -f "$STATE/questions.tsv" "$STATE/blueprint.md" "$STATE/summary.md" "$STATE/acceptance-checks.draft" \
      "$STATE/sow.md" "$STATE/atomic.tsv" "$STATE/challenges.tsv" "$STATE/clarifications.tsv" \
      "$STATE/clarifications.business.tsv" "$STATE/clarifications.technical.tsv" "$STATE"/*.ok 2>/dev/null || true
    rm -f "$STATE/answers"/*.txt 2>/dev/null || true
    atomic_write "$STATE/intent.txt" "$STATE/.intent.tmp.$$" "$intent"; write_questions "$intent"
  elif [ ! -e "$STATE/questions.tsv" ]; then write_questions "$intent"; fi
  local n; n="$(open_count)"; emit_questions "$intent" "$n"; [ "$n" -eq 0 ] || return "$ERR_AMBIGUOUS"
}
answer() {
  local id="${1-}" text="${2-}" p
  [ -n "$id" ] && [ -n "$text" ] || die "$ERR_USAGE" answer_args 'answer requires QUESTION_ID and TEXT' 'use intake.sh answer ID "answer text"'
  valid_id "$id" || die "$ERR_USAGE" unknown_question 'unknown question id' 'run intake.sh to list open question ids'
  [ -e "$STATE/intent.txt" ] || die "$ERR_USAGE" no_intent 'no intent has been recorded' 'run intake.sh ask INTENT first'
  question_line "$id" >/dev/null || die "$ERR_USAGE" unknown_question 'question is not part of the current intent' 'run intake.sh to list the current questions'
  acquire_lock; p="$(answer_path "$id")"
  if [ -s "$p" ]; then
    [ "$JSON" -eq 1 ] && jq -cn --arg id "$id" '{ok:true,changed:false,id:$id,status:"already_answered"}' || { toon_preamble intake 'ambiguity answer' "$(now_iso)"; toon_open answer 1 'id,status'; toon_row "$id" already_answered; }; return 0
  fi
  atomic_write "$p" "$STATE/.answer.$id.tmp.$$" "$text"
  [ "$JSON" -eq 1 ] && jq -cn --arg id "$id" '{ok:true,changed:true,id:$id,status:"answered"}' || { toon_preamble intake 'ambiguity answer' "$(now_iso)"; toon_open answer 1 'id,status'; toon_row "$id" answered; }
}

validate_sow() {
  local file="$1" ih expected heading; [ -s "$file" ] || { echo 'SOW file is missing or empty' >&2; return 1; }
  ih="$(hash_file "$STATE/intent.txt")"; expected="$(grep -i '^source_intent_hash:' "$file" | head -1 | sed 's/^[^:]*:[[:space:]]*//')"
  [ "$expected" = "$ih" ] || { echo "source_intent_hash does not match intent.txt (expected $ih)" >&2; return 1; }
  for heading in 'Request restatement' 'Built for' 'Must do' 'Explicitly will not do' 'Done when' 'Acceptance threshold'; do
    grep -qi "^## ${heading}[[:space:]]*$" "$file" || { echo "missing SOW section: $heading" >&2; return 1; }
    awk -v wanted="## $heading" 'BEGIN{found=0; body=0} $0==wanted{found=1; next} /^## /&&found{exit} found && $0 !~ /^[[:space:]]*$/{body=1} END{if(!found || body==0) exit 1}' "$file" || { echo "SOW section is empty: $heading" >&2; return 1; }
  done
  grep -qi '^request:[[:space:]]*[^[:space:]].*' "$file" || { echo 'SOW needs a non-empty request: line' >&2; return 1; }
  grep -qiE '[0-9]+%|p95|exit[[:space:]]+0|100%' "$file" || { echo 'SOW needs a measurable acceptance threshold' >&2; return 1; }
  grep -qiE 'not|will not|out of scope|excluded|forbidden' "$file" || { echo 'SOW needs explicit non-goals' >&2; return 1; }
  grep -qiE 'TBD|TODO|<[^>]+>|\[fill|placeholder|to be decided|lorem ipsum' "$file" && { echo 'SOW contains an unresolved placeholder' >&2; return 1; }
  return 0
}
sow() {
  rubric_gate || stage_blocked sow_missing "$ERR_AMBIGUOUS" 'SOW is gated by the existing rubric; answer it before drafting the SOW'
  local file="${1-}" tmp="$STATE/.sow.tmp.$$"; [ -n "$file" ] || die "$ERR_USAGE" sow_args 'sow requires a FILE' 'write the derived SOW, then run intake.sh sow FILE'; [ -f "$file" ] || die "$ERR_USAGE" sow_missing_file 'SOW FILE does not exist' 'pass a readable SOW file'
  validate_sow "$file" || stage_blocked sow_invalid "$ERR_AMBIGUOUS" 'SOW is not complete or is not linked to the recorded request'
  cp "$file" "$tmp" && mv "$tmp" "$STATE/sow.md" || die "$ERR_GENERIC" state_write_failed 'could not store SOW' 'check the intake state directory'; record_stage sow; printf 'sow: accepted (%s)\n' "$STATE/sow.md"
}

atomic_ids_file="$STATE/.atomic.ids.$$"
validate_atomic() {
  local file="$1" id tier parents description inputs outputs acceptance decision parent ptier rows=0
  [ -s "$file" ] || { echo 'atomic decomposition is missing or empty' >&2; return 1; }
  head -1 "$file" | grep -qx 'id\ttier\tparents\tdescription\tinputs\toutputs\tacceptance\tdesign_decision' || { echo 'atomic.tsv has the wrong header' >&2; return 1; }
  awk -F '\t' 'NR>1 && NF!=8 {bad=1} END{exit bad?1:0}' "$file" || { echo 'every atomic row must have exactly 8 tab-separated fields' >&2; return 1; }
  : > "$atomic_ids_file"
  while IFS='	' read -r id tier parents description inputs outputs acceptance decision; do
    [ "$id" = id ] && continue; [ -n "$id" ] || continue; rows=$((rows+1))
    cut -f1 "$atomic_ids_file" 2>/dev/null | grep -Fqx "$id" && { echo "duplicate atomic id: $id" >&2; return 1; }; printf '%s\t%s\n' "$id" "$tier" >> "$atomic_ids_file"
    case "$tier" in feature|service|module) ;; *) echo "invalid tier for $id: $tier" >&2; return 1;; esac
    [ -n "$description" ] && [ -n "$inputs" ] && [ -n "$outputs" ] && [ -n "$acceptance" ] || { echo "atomic row $id lacks an independent build contract" >&2; return 1; }
    case "$decision" in none|no) ;; *) echo "atomic leaf $id still contains a design decision: $decision" >&2; return 1;; esac
    printf '%s\n' "$description $inputs $outputs $acceptance" | grep -qiE 'TBD|TODO|to be decided|decide|design choice|either|unknown|figure out|determine' && { echo "atomic leaf $id still contains a design decision" >&2; return 1; }
    if [ "$tier" = feature ]; then [ "$parents" = - ] || { echo "feature $id must have parents=-" >&2; return 1; }; else [ "$parents" != - ] || { echo "$tier $id must compose a lower tier" >&2; return 1; }; fi
  done < "$file"
  [ "$rows" -gt 0 ] || { echo 'atomic decomposition has no rows' >&2; return 1; }
  while IFS='	' read -r id tier parents description inputs outputs acceptance decision; do
    [ "$id" = id ] && continue; [ -n "$id" ] || continue
    if [ "$tier" = service ]; then
      for parent in $(printf '%s' "$parents" | tr ',' ' '); do ptier="$(awk -F '\t' -v p="$parent" '$1==p{print $2}' "$atomic_ids_file")"; [ "$ptier" = feature ] || { echo "service $id must compose feature $parent" >&2; return 1; }; done
    elif [ "$tier" = module ]; then
      for parent in $(printf '%s' "$parents" | tr ',' ' '); do ptier="$(awk -F '\t' -v p="$parent" '$1==p{print $2}' "$atomic_ids_file")"; [ "$ptier" = service ] || { echo "module $id must compose service $parent" >&2; return 1; }; done
    fi
  done < "$file"; return 0
}
atomic_stage() {
  rubric_gate || stage_blocked atomic_rubric "$ERR_AMBIGUOUS" 'atomic decomposition is gated by the rubric'
  [ -s "$STATE/sow.md" ] && validate_sow "$STATE/sow.md" >/dev/null 2>&1 || stage_blocked atomic_sow "$ERR_AMBIGUOUS" 'atomic decomposition is gated by a valid SOW'
  local file="${1-}" tmp="$STATE/.atomic.tmp.$$"; [ -n "$file" ] || die "$ERR_USAGE" atomic_args 'atomic requires a FILE' 'write atomic.tsv and pass its path'; [ -f "$file" ] || die "$ERR_USAGE" atomic_missing_file 'atomic FILE does not exist' 'pass a readable atomic.tsv file'
  validate_atomic "$file" || stage_blocked atomic_invalid "$ERR_AMBIGUOUS" 'a leaf still has a design decision or violates the feature/service/module contract'
  cp "$file" "$tmp" && mv "$tmp" "$STATE/atomic.tsv" || die "$ERR_GENERIC" state_write_failed 'could not store atomic decomposition' 'check the intake state directory'; record_stage atomic; printf 'atomic: accepted (%s)\n' "$STATE/atomic.tsv"
}

challenge_source_exists() {
  case "$1" in
    FAILURE-CORPUS:*) grep -q "| \*\*${1#*:}\*\* |" "$D/docs/design/FAILURE-CORPUS.md" 2>/dev/null;;
    THREAD-LESSONS:*) grep -q "| ${1#*:} |" "$D/state/memory/learning/THREAD-LESSONS.md" 2>/dev/null;;
    *) return 1;;
  esac
}
validate_challenges() {
  local file="$1" id source leaf risk trigger mitigation rows=0
  [ -s "$file" ] || { echo 'challenge register is missing or empty' >&2; return 1; }
  head -1 "$file" | grep -qx 'id\tsource\taffected_leaf\trisk\ttrigger\tmitigation' || { echo 'challenges.tsv has the wrong header' >&2; return 1; }
  awk -F '\t' 'NR>1 && NF!=6 {bad=1} END{exit bad?1:0}' "$file" || { echo 'every challenge row must have exactly 6 tab-separated fields' >&2; return 1; }
  while IFS='	' read -r id source leaf risk trigger mitigation; do
    [ "$id" = id ] && continue; [ -n "$id" ] || continue; rows=$((rows + 1))
    challenge_source_exists "$source" || { echo "challenge $id cites no known corpus row: $source" >&2; return 1; }
    grep -Fqx "$leaf" <(tail -n +2 "$STATE/atomic.tsv" | cut -f1) || { echo "challenge $id points at unknown atomic leaf: $leaf" >&2; return 1; }
    [ -n "$risk" ] && [ -n "$trigger" ] && [ -n "$mitigation" ] || { echo "challenge $id needs risk, trigger, and mitigation" >&2; return 1; }
  done < "$file"; [ "$rows" -gt 0 ] || { echo 'challenge register has no rows' >&2; return 1; }; return 0
}
challenges() {
  rubric_gate || stage_blocked challenges_rubric "$ERR_AMBIGUOUS" 'challenge mining is gated by the rubric'
  [ -s "$STATE/sow.md" ] && validate_sow "$STATE/sow.md" >/dev/null 2>&1 || stage_blocked challenges_sow "$ERR_AMBIGUOUS" 'challenge mining is gated by a valid SOW'
  [ -s "$STATE/atomic.tsv" ] && validate_atomic "$STATE/atomic.tsv" >/dev/null 2>&1 || stage_blocked challenges_atomic "$ERR_AMBIGUOUS" 'challenge mining is gated by an atomic decomposition'
  local file="${1-}" tmp="$STATE/.challenges.tmp.$$"; [ -n "$file" ] || die "$ERR_USAGE" challenges_args 'challenges requires a FILE' 'write challenges.tsv with FAILURE-CORPUS or THREAD-LESSONS sources'; [ -f "$file" ] || die "$ERR_USAGE" challenges_missing_file 'challenges FILE does not exist' 'pass a readable challenges.tsv file'
  validate_challenges "$file" || stage_blocked challenges_invalid "$ERR_AMBIGUOUS" 'every challenge must cite a real FAILURE-CORPUS or THREAD-LESSONS row'
  cp "$file" "$tmp" && mv "$tmp" "$STATE/challenges.tsv" || die "$ERR_GENERIC" state_write_failed 'could not store challenge register' 'check the intake state directory'; record_stage challenges; printf 'challenges: accepted (%s)\n' "$STATE/challenges.tsv"
}

clarification_ref_exists() {
  case "$1" in
    sow:*) grep -qi "${1#*:}" "$STATE/sow.md" 2>/dev/null;;
    atomic:*) grep -Fqx "${1#*:}" <(tail -n +2 "$STATE/atomic.tsv" | cut -f1) 2>/dev/null;;
    challenge:*) grep -Fqx "${1#*:}" <(tail -n +2 "$STATE/challenges.tsv" | cut -f1) 2>/dev/null;;
    *) return 1;;
  esac
}
validate_clarifications() {
  local file="$1" id kind ref question blocking answer rows=0
  [ -s "$file" ] || { echo 'clarification register is missing or empty' >&2; return 1; }
  head -1 "$file" | grep -qx 'id\tkind\tgap_ref\tquestion\tblocking\tanswer' || { echo 'clarifications.tsv has the wrong header' >&2; return 1; }
  awk -F '\t' 'NR>1 && NF!=6 {bad=1} END{exit bad?1:0}' "$file" || { echo 'every clarification row must have exactly 6 tab-separated fields' >&2; return 1; }
  while IFS='	' read -r id kind ref question blocking answer; do
    [ "$id" = id ] && continue; [ -n "$id" ] || continue; rows=$((rows+1))
    case "$kind" in business|technical) ;; *) echo "clarification $id must be business or technical" >&2; return 1;; esac
    clarification_ref_exists "$ref" || { echo "clarification $id has an untraceable gap: $ref" >&2; return 1; }
    printf '%s' "$question" | grep -qiE '^(what do you want|tell me more|provide more detail|any other requirements)' && { echo "clarification $id is generic noise; tie it to $ref" >&2; return 1; }
    case "$blocking" in yes|no) ;; *) echo "clarification $id blocking must be yes or no" >&2; return 1;; esac
    [ "$blocking" = yes ] || [ -n "$answer" ] || { echo "non-blocking clarification $id still needs an answer" >&2; return 1; }
  done < "$file"; [ "$rows" -gt 0 ] || { echo 'clarification register has no rows' >&2; return 1; }; return 0
}
clarifications() {
  rubric_gate || stage_blocked clarifications_rubric "$ERR_AMBIGUOUS" 'clarifications are gated by the rubric'
  [ -s "$STATE/sow.md" ] && validate_sow "$STATE/sow.md" >/dev/null 2>&1 || stage_blocked clarifications_sow "$ERR_AMBIGUOUS" 'clarifications are gated by a valid SOW'
  [ -s "$STATE/atomic.tsv" ] && validate_atomic "$STATE/atomic.tsv" >/dev/null 2>&1 || stage_blocked clarifications_atomic "$ERR_AMBIGUOUS" 'clarifications are gated by an atomic decomposition'
  [ -s "$STATE/challenges.tsv" ] && validate_challenges "$STATE/challenges.tsv" >/dev/null 2>&1 || stage_blocked clarifications_challenges "$ERR_AMBIGUOUS" 'clarifications are gated by a challenge register'
  local file="${1-}" tmp="$STATE/.clarifications.tmp.$$"; [ -n "$file" ] || die "$ERR_USAGE" clarifications_args 'clarifications requires a FILE' 'write linked business/technical clarifications.tsv'; [ -f "$file" ] || die "$ERR_USAGE" clarifications_missing_file 'clarifications FILE does not exist' 'pass a readable clarifications.tsv file'
  validate_clarifications "$file" || stage_blocked clarifications_invalid "$ERR_AMBIGUOUS" 'clarifications must be specific, linked, split business/technical, and answered when non-blocking'
  cp "$file" "$tmp" && mv "$tmp" "$STATE/clarifications.tsv" || die "$ERR_GENERIC" state_write_failed 'could not store clarifications' 'check the intake state directory'
  tail -n +2 "$STATE/clarifications.tsv" | awk -F '\t' '$2=="business"' > "$STATE/clarifications.business.tsv"
  tail -n +2 "$STATE/clarifications.tsv" | awk -F '\t' '$2=="technical"' > "$STATE/clarifications.technical.tsv"
  record_stage clarifications; printf 'clarifications: accepted (business=%s technical=%s)\n' "$STATE/clarifications.business.tsv" "$STATE/clarifications.technical.tsv"
}
clarify() {
  local id="${1-}" text="${2-}" qfile="$STATE/.clarifications.answer.$$" found=0; [ -n "$id" ] && [ -n "$text" ] || die "$ERR_USAGE" clarify_args 'clarify requires ID and TEXT' 'use intake.sh clarify ID "answer"'; [ -s "$STATE/clarifications.tsv" ] || stage_blocked clarifications_missing "$ERR_AMBIGUOUS" 'install clarifications before answering one'
  : > "$qfile"
  while IFS='	' read -r cid kind ref question blocking answer; do
    if [ "$cid" = "$id" ]; then found=1; printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$cid" "$kind" "$ref" "$question" "$blocking" "$text" >> "$qfile"; else printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$cid" "$kind" "$ref" "$question" "$blocking" "$answer" >> "$qfile"; fi
  done < <(tail -n +2 "$STATE/clarifications.tsv")
  [ "$found" -eq 1 ] || { rm -f "$qfile"; die "$ERR_USAGE" unknown_clarification 'clarification ID is not in the current register' 'run intake.sh status to inspect it'; }
  { printf '%s\n' 'id	kind	gap_ref	question	blocking	answer'; cat "$qfile"; } > "$STATE/.clarifications.final.$$" && mv "$STATE/.clarifications.final.$$" "$STATE/clarifications.tsv"
  rm -f "$qfile"; clarifications "$STATE/clarifications.tsv" >/dev/null; printf 'clarification: answered (%s)\n' "$id"
}

open_blockers() { awk -F '\t' 'NR>1 && $5=="yes" && $6=="" {n++} END{print n+0}' "$STATE/clarifications.tsv" 2>/dev/null; }
stage_ready() { [ -s "$STATE/sow.md" ] && validate_sow "$STATE/sow.md" >/dev/null 2>&1 && [ -s "$STATE/atomic.tsv" ] && validate_atomic "$STATE/atomic.tsv" >/dev/null 2>&1 && [ -s "$STATE/challenges.tsv" ] && validate_challenges "$STATE/challenges.tsv" >/dev/null 2>&1 && [ -s "$STATE/clarifications.tsv" ] && validate_clarifications "$STATE/clarifications.tsv" >/dev/null 2>&1; }
gate() {
  local reason='ready' blockers=0; rubric_gate >/dev/null 2>&1 || reason='rubric'
  if [ "$reason" = ready ]; then
    if ! stage_ready; then reason='stages'; fi
  fi
  if [ "$reason" = ready ]; then
    blockers="$(open_blockers)"
    if [ "$blockers" -ne 0 ]; then reason='blocking_clarifications'; fi
  fi
  if [ "$JSON" -eq 1 ]; then jq -cn --arg reason "$reason" --argjson blockers "${blockers:-0}" '{ok:($reason=="ready"),reason:$reason,open_blockers:$blockers}'; else toon_preamble intake 'code-start gate' "$(now_iso)"; toon_open gate 1 'decision,reason,open_blockers'; [ "$reason" = ready ] && toon_row allow ready "${blockers:-0}" || toon_row refuse "$reason" "${blockers:-0}"; fi
  [ "$reason" = ready ] || return "$ERR_AMBIGUOUS"
}
blueprint() {
  gate >/dev/null || stage_blocked blueprint_not_ready "$ERR_AMBIGUOUS" 'code is blocked until SOW, atomic decomposition, corpus challenges, and answered blockers exist'
  local bp="$STATE/blueprint.md" checks="$STATE/acceptance-checks.draft" model="$GENERATOR_MODEL"
  { printf '%s\n' '# fleet build blueprint' '' '## Statement of Work' ''; cat "$STATE/sow.md"; printf '%s\n' '' '## Atomic decomposition'; cat "$STATE/atomic.tsv"; printf '%s\n' '' '## Challenge register'; cat "$STATE/challenges.tsv"; printf '%s\n' '' '## Clarifications (business)'; cat "$STATE/clarifications.business.tsv"; printf '%s\n' '' '## Clarifications (technical)'; cat "$STATE/clarifications.technical.tsv"; printf '%s\n' '' '## Deterministic gate' 'The blueprint is buildable only while intake.sh gate exits 0. Any artifact drift or new blocking clarification returns exit 7.'; } > "$bp"
  { printf '%s\n' '# fleet acceptance checks — DRAFT' "drafting_model: $model" 'blueprint_state: DRAFT'; printf '%s\n' 'check_1: every SOW acceptance threshold is measurable and linked to the recorded request hash.' 'check_2: every feature leaf has explicit inputs, outputs, acceptance, and design_decision=none.' 'check_3: every challenge cites docs/design/FAILURE-CORPUS.md or learn/THREAD-LESSONS.md and names a trigger and mitigation.' 'check_4: every clarification is business or technical, linked to a gap, and every blocking row is answered before gate exit 0.'; } > "$checks"
  record_stage blueprint; [ "$JSON" -eq 1 ] && jq -cn --arg blueprint "$bp" --arg checks "$checks" '{ok:true,status:"READY",blueprint:$blueprint,acceptance_checks:$checks}' || { toon_preamble intake 'ready build blueprint' "$(now_iso)"; toon_open blueprint 1 'status,file,checks'; toon_row READY "$bp" "$checks"; }
}
summary() {
  gate >/dev/null || stage_blocked summary_not_ready "$ERR_AMBIGUOUS" 'summary is withheld until the full intake sequence is ready'
  local out="$STATE/summary.md" intent; intent="$(read_intent)"
  { printf '%s\n' '# Sign-off summary' '' 'Asked:' "$intent" '' 'Artifacts:' "- SOW: $STATE/sow.md" "- Atomic: $STATE/atomic.tsv" "- Challenges: $STATE/challenges.tsv" "- Business clarifications: $STATE/clarifications.business.tsv" "- Technical clarifications: $STATE/clarifications.technical.tsv" '' 'Code-start gate: READY (deterministic exit 0)' '' 'Exact four checks awaiting signature:'; sed -n 's/^check_[0-9]*: //p' "$STATE/acceptance-checks.draft"; } > "$out"
  [ "$JSON" -eq 1 ] && jq -Rs --arg file "$out" '{ok:true,file:$file,summary:.' < "$out" || { toon_preamble intake 'sign-off summary' "$(now_iso)"; toon_open summary 1 'status,file'; toon_row READY "$out"; [ "$FULL" -eq 1 ] && sed -n '1,160p' "$out"; }
}
status() {
  local intent n sow_state atomic_state challenge_state clarification_state blockers; intent="$(read_intent)"; n="$(open_count)"
  [ -s "$STATE/sow.md" ] && validate_sow "$STATE/sow.md" >/dev/null 2>&1 && sow_state=ready || sow_state=blocked
  [ -s "$STATE/atomic.tsv" ] && validate_atomic "$STATE/atomic.tsv" >/dev/null 2>&1 && atomic_state=ready || atomic_state=blocked
  [ -s "$STATE/challenges.tsv" ] && validate_challenges "$STATE/challenges.tsv" >/dev/null 2>&1 && challenge_state=ready || challenge_state=blocked
  [ -s "$STATE/clarifications.tsv" ] && validate_clarifications "$STATE/clarifications.tsv" >/dev/null 2>&1 && clarification_state=ready || clarification_state=blocked
  blockers="$(open_blockers)"
  if [ "$JSON" -eq 1 ]; then jq -cn --arg intent "$intent" --argjson open "$n" --arg sow "$sow_state" --arg atomic "$atomic_state" --arg challenges "$challenge_state" --arg clarifications "$clarification_state" --argjson blockers "$blockers" '{ok:true,intent:$intent,open_questions:$open,stages:{sow:$sow,atomic:$atomic,challenges:$challenges,clarifications:$clarifications},open_blocking_clarifications:$blockers}'; else toon_preamble intake 'staged intake state' "$(now_iso)"; toon_open stages 1 'rubric_open,sow,atomic,challenges,clarifications,blocking_clarifications'; toon_row "$n" "$sow_state" "$atomic_state" "$challenge_state" "$clarification_state" "$blockers"; fi
}
main() {
  local cmd=""; require_paths
  while [ "$#" -gt 0 ]; do case "$1" in --help|-h) usage; return 0;; --json) JSON=1; shift;; --full) FULL=1; shift;; --now) need_val --now "${2-}"; NOW="$2"; shift 2;; --*) reject_unknown_flag "$1";; *) cmd="$1"; shift; break;; esac; done
  case "$cmd" in
    '') status;; ask) [ "$#" -eq 1 ] || die "$ERR_USAGE" ask_args 'ask requires one intent argument' 'use intake.sh ask "intent"'; ask "$1";;
    answer) [ "$#" -eq 2 ] || die "$ERR_USAGE" answer_args 'answer requires QUESTION_ID and TEXT' 'use intake.sh answer ID "answer text"'; answer "$1" "$2";;
    sow) [ "$#" -le 1 ] || reject_unknown_flag "$1"; sow "${1-}";; atomic) [ "$#" -le 1 ] || reject_unknown_flag "$1"; atomic_stage "${1-}";;
    challenges) [ "$#" -le 1 ] || reject_unknown_flag "$1"; challenges "${1-}";; clarifications) [ "$#" -le 1 ] || reject_unknown_flag "$1"; clarifications "${1-}";;
    clarify) [ "$#" -eq 2 ] || die "$ERR_USAGE" clarify_args 'clarify requires ID and TEXT' 'use intake.sh clarify ID "answer"'; clarify "$1" "$2";;
    gate) [ "$#" -eq 0 ] || reject_unknown_flag "$1"; gate;; blueprint) [ "$#" -eq 0 ] || reject_unknown_flag "$1"; blueprint;; summary) [ "$#" -eq 0 ] || reject_unknown_flag "$1"; summary;; status) [ "$#" -eq 0 ] || reject_unknown_flag "$1"; status;;
    *) die "$ERR_USAGE" unknown_subcommand "unknown subcommand '$cmd'" 'run intake.sh --help';;
  esac
}
main "$@"
