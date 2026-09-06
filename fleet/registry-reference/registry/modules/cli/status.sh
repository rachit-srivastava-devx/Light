#!/usr/bin/env bash
# status.sh — a read-only status surface over on-disk suites and recorded exits.
#
# Assumptions: suite filenames are newline-free and use the documented *.test.sh suffix, and
# var/test-results.tsv holds one result per suite.
#
# var/test-results.tsv HAS NO WRITER. The checks that actually run today are `./fleet test`,
# registry/features/gate/gate.sh and registry/features/scan/scan.sh, and none of them writes this legacy cache. The retired verifier,
# only thing that ever produced it, was retired with gates/ when no-mistakes was adopted
# (ADOPT.md S2 "gate board + merge authority"); no adopted tool writes this format, and
# registry/features/gate/gate.sh records receipts to the ledger, not here. The file on disk is therefore a frozen
# snapshot: a suite added or re-run since then reads back as pending until its row is recorded
# by hand, or until FLEET_TEST_RESULTS points at a fresher file. Read every `done` row as a
# past run, not a current one. Closing this for real needs an ADOPT.md row for the runner, not a hand-rolled
# replacement here (ADOPT.md S5).
#
# This command never executes a suite. Missing results are pending; malformed or duplicate
# results are needs-iteration so bad evidence can never look like a pass.
# Does not handle: a suite filename containing a newline (the POSIX line-oriented result
# contract cannot represent that path); such a tree must be renamed before status is trusted.
set -u

D="${FLEET_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}"
. "$D/registry/lib/toon.sh"
. "$D/registry/lib/receipt.sh"
. "$D/registry/lib/err.sh"

OUT="${FLEET_STATUS_OUTPUT:-$D/docs/reports/STATUS.md}"
NOW="${FLEET_NOW:-}"
JSON=0
FULL=0

usage() {
  printf '%s\n' \
    'status.sh [--json] [--full] [--now ISO]' \
    '  no arguments       regenerate STATUS.md and show a compact live summary' \
    '  --full             print the generated STATUS.md after the summary'
}

now_iso() { [ -n "$NOW" ] && printf '%s' "$NOW" || date -u +%Y-%m-%dT%H:%M:%SZ; }

# The list is derived from the filesystem every time. The known metadata below describes
# legacy components; it is not a component registry and cannot hide a newly added suite.
component_files() {
  {
    local f
    for f in "$D"/tests/*.test.sh "$D"/tests/*/*.test.sh; do
      [ -f "$f" ] && printf '%s\n' "$f"
    done
  } | LC_ALL=C sort
}

# component_data -> name|blurb|test-relative-path|results-key|requirements
component_data() {
  local file rel suite key
  while IFS= read -r file; do
    [ -n "$file" ] || continue
    rel="${file#$D/}"
    suite="${rel#tests/}"
    suite="${suite%.test.sh}"
    key="${suite##*/}"
    case "$key" in
      meter) printf '%s\n' "meter|Shows token use and refuses work that would exceed the budget.|$rel|meter|req2,req10,req15";;
      route) printf '%s\n' "route|Chooses a harness and model from task shape and measured capability.|$rel|route|req3,req13,req15";;
      ledger) printf '%s\n' "ledger|Records feature state, learning evidence, and the receipt chain.|$rel|ledger|req9,req12,req15";;
      intake) printf '%s\n' "intake|Blocks unclear work and locks the four acceptance checks.|$rel|intake|req5,req6,req7";;
      gates) printf '%s\n' "gates|Runs the ordered checks and stops at the first blocker.|$rel|gates|req4,req9,req11";;
      entry) printf '%s\n' "entry|Composes the flow and regenerates this status surface.|$rel|entry|req12,req13,req14";;
      *) printf '%s|Recorded test evidence for suite %s.|%s|%s|\n' "$suite" "$key" "$rel" "$key";;
    esac
  done <<EOF
$(component_files)
EOF
}

path_list_present() {
  local paths="$1" p
  for p in $paths; do [ -x "$D/$p" ] || return 1; done
  return 0
}

first_missing_path() {
  local paths="$1" p
  for p in $paths; do [ -x "$D/$p" ] || { printf '%s' "$D/$p"; return 0; }; done
  printf '%s' "$D/$paths"
}

# --- STATUS IS A READER, NEVER A RUNNER ---------------------------------------
STATUS_RESULTS="${FLEET_TEST_RESULTS:-$D/var/test-results.tsv}"
RESULT_CACHE=""

cleanup() {
  [ -n "$RESULT_CACHE" ] && rm -f "$RESULT_CACHE"
}
trap cleanup EXIT HUP INT TERM

build_result_cache() {
  RESULT_CACHE="$(mktemp "${TMPDIR:-/tmp}/fleet-status-results.XXXXXX")"
  [ -f "$STATUS_RESULTS" ] || return 0
  awk -F'\t' '
    NR == 1 { next }
    NF != 5 || $2 !~ /^[0-9]+$/ || $3 !~ /^[0-9]+$/ || $4 !~ /^[0-8]$/ {
      print $1 "|__INVALID__"; next
    }
    { print $1 "|" $4 "|" $2 "|" $3 }
  ' "$STATUS_RESULTS" > "$RESULT_CACHE"
}

# recorded_result <suite> -> "exit|passed|failed". A missing row returns 1.
# A malformed or duplicate row returns 1 with a typed marker on stdout.
recorded_result() {
  local suite="$1" row_suite row_ec row_pass row_fail rows=0 value=""
  [ -f "$RESULT_CACHE" ] || return 1
  while IFS='|' read -r row_suite row_ec row_pass row_fail; do
    [ "$row_suite" = "$suite" ] || continue
    rows=$((rows+1)); value="$row_ec|${row_pass-}|${row_fail-}"
  done < "$RESULT_CACHE"
  [ "$rows" -gt 0 ] || return 1
  [ "$rows" -eq 1 ] || { printf '%s' '__DUPLICATE__'; return 1; }
  case "$value" in
    __INVALID__|__INVALID__\|*) printf '%s' '__INVALID__'; return 1;;
    *) printf '%s' "$value";;
  esac
}

status_of() {
  local name="$1" test_file="$2" suite="$3" rec ec detail scripts=""
  STATUS_VALUE=pending; STATUS_DETAIL='no recorded test result'; STATUS_EXIT=0
  case "$suite" in
    meter) scripts='registry/services/measure/meter.sh';;
    # route.sh/detect.sh and the fleet.sh front door were retired into the firstmate adapter,
    # and the whole gates/ pipeline into the no-mistakes adapter (ADOPT.md S2). Guard the
    # adapter that exists, never the paths it replaced -- a guard on a deleted path reports
    # "missing implementation" for a component that is present and green.
    route) scripts='registry/services/dispatch/dispatch.sh';;
    ledger) scripts='registry/services/verify/verify.sh';;
    # bin/lockcheck.sh went with gates/02-acceptance and has NO replacement (ADOPT.md S3a):
    # dropped rather than repointed, because there is nothing to repoint it at.
    intake) scripts='registry/services/verify/intake.sh';;
    gates) scripts='registry/services/verify/verify.sh';;
    entry) scripts='registry/services/dispatch/dispatch.sh';;
  esac
  if [ -n "$scripts" ] && ! path_list_present "$scripts"; then
    STATUS_DETAIL="missing implementation: $(first_missing_path "$scripts")"
    return 0
  fi
  if [ ! -f "$D/$test_file" ]; then
    STATUS_VALUE=needs-iteration; STATUS_DETAIL='missing test suite'
    return 0
  fi
  if ! rec="$(recorded_result "$suite")"; then
    case "$rec" in
      __DUPLICATE__) STATUS_VALUE=needs-iteration; STATUS_DETAIL='duplicate recorded results — dedupe this suite in var/test-results.tsv' ;;
      __INVALID__) STATUS_VALUE=needs-iteration; STATUS_DETAIL='malformed recorded result — repair this suite row in var/test-results.tsv' ;;
      *) STATUS_VALUE=pending; STATUS_DETAIL='no recorded test result — run the suite, then record the row by hand (nothing writes this cache)' ;;
    esac
    return 0
  fi
  ec="${rec%%|*}"
  detail="${rec#*|}"
  local passed="${detail%%|*}" failed="${detail#*|}"
  if [ "$ec" -eq 0 ]; then
    STATUS_VALUE='done'
  else
    STATUS_VALUE=failed
  fi
  STATUS_DETAIL="$passed passed, $failed failed"; STATUS_EXIT="$ec"
}

require_inputs() {
  require_bin python3 'install python3 and retry'
  require_bin jq 'install jq and retry'
  require_bin shasum 'install shasum and retry'
}

write_status() {
  local generated="$1" total="$2" usable="$3" rows="$4" req_rows="$5" blockers="$6" not_built="$7" chain="$8" receipt_count="$9" tmp
  tmp="$OUT.tmp.$$"
  {
    printf '# fleet v2 — STATUS\n\n'
    printf '_Generated %s by `registry/modules/cli/status.sh` from current files, recorded test exits, and the receipt ledger._\n\n' "$generated"
    printf '## %s of %s suites are usable right now.\n\n' "$usable" "$total"
    printf '| component | what it does | status | next action |\n|---|---|---|---|\n%s\n' "$rows"
    printf '## Requirement coverage\n\n'
    printf '| requirement | status | evidence |\n|---|---|---|\n%s\n' "$req_rows"
    printf '## Blockers\n\n'
    if [ -n "$blockers" ]; then printf '%s\n' "$blockers"; else printf '%s\n' '- none observed by the recorded checks.'; fi
    printf '\n## What is not built\n\n'
    if [ -n "$not_built" ]; then printf '%s\n' "$not_built"; else printf '%s\n' '- No unbuilt requirement was found in the current evidence.'; fi
    printf '\n## Receipt evidence\n\n- %s\n- receipt lines observed: %s\n' "$chain" "$receipt_count"
  } > "$tmp" || die "$ERR_GENERIC" status_write_failed 'could not write STATUS.md' 'check the workspace permissions'
  mv "$tmp" "$OUT" || die "$ERR_GENERIC" status_write_failed 'could not publish STATUS.md' 'check the workspace permissions'
}

main() {
  local cmd="" generated total=0 usable=0 name blurb test_file key _reqs st detail ec
  local req_rows="" blockers="" not_built="" chain receipt_count=0 req req_status req_evidence
  local rows="" action component_data_text status_rows=""
  local status_of_meter=pending status_of_route=pending status_of_ledger=pending
  local status_of_intake=pending status_of_gates=pending status_of_entry=pending
  local meter_detail='no meter suite evidence' route_detail='no route suite evidence'
  local intake_detail='no intake suite evidence'
  local gates_detail='no gates suite evidence' entry_detail='no entry suite evidence'

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --json) JSON=1; shift;;
      --full) FULL=1; shift;;
      --now) need_val --now "${2-}"; NOW="$2"; shift 2;;
      --help|-h) usage; return 0;;
      --*) reject_unknown_flag "$1";;
      *) [ -z "$cmd" ] || reject_unknown_flag "$1"; cmd="$1"; shift;;
    esac
  done
  [ -z "$cmd" ] || die "$ERR_USAGE" unknown_subcommand "unknown status subcommand '$cmd'" 'run status.sh --help'
  require_inputs
  build_result_cache
  generated="$(now_iso)"
  component_data_text="$(component_data)"

  while IFS='|' read -r name blurb test_file key _reqs; do
    [ -n "$name" ] || continue
    total=$((total+1))
    status_of "$name" "$test_file" "$key"
    st="$STATUS_VALUE"; detail="$STATUS_DETAIL"; ec="$STATUS_EXIT"
    [ "$st" = "done" ] && usable=$((usable+1))
    status_rows="${status_rows}${name}|${st}|${detail}|${ec}"$'\n'
    case "$st" in
      done) action='none — independently verify the component';;
      failed) action='re-run the suite, then record the result in var/test-results.tsv'; blockers="${blockers}- [$name] recorded exit $ec; proof: bash $test_file, then read $STATUS_RESULTS"$'\n';;
      needs-iteration) action='repair this suite row in var/test-results.tsv'; blockers="${blockers}- [$name] suite evidence is ambiguous or malformed; proof: inspect $STATUS_RESULTS"$'\n';;
      pending) action='run the suite, then record its result in var/test-results.tsv'; blockers="${blockers}- [$name] has no recorded result; proof: bash $test_file, then inspect the $key row in $STATUS_RESULTS"$'\n';;
    esac
    rows="${rows}| \`$name\` | $blurb | **$st** | $action |"$'\n'
    case "$key" in
      meter) status_of_meter="$st"; meter_detail="$detail";;
      route) status_of_route="$st"; route_detail="$detail";;
      ledger) status_of_ledger="$st";;
      intake) status_of_intake="$st"; intake_detail="$detail";;
      gates) status_of_gates="$st"; gates_detail="$detail";;
      entry) status_of_entry="$st"; entry_detail="$detail";;
    esac
  done <<EOF
$component_data_text
EOF

  # Requirement state remains deliberately narrower than suite state: a requirement is done only
  # when its implementing component is green. These are the contract's documented 1-15 labels.
  for req in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
    req_status=not-started; req_evidence='no implementing component is present in this tree'
    case "$req" in
      1|8) req_status=not-started; req_evidence='no owned implementation or green suite found';;
      2) req_status="$status_of_meter"; req_evidence="tests/small/meter.test.sh: $meter_detail";;
      3) req_status="$status_of_route"; req_evidence="route component: $route_detail";;
      4) req_status="$status_of_gates"; req_evidence="gates component: $gates_detail";;
      5|6|7) req_status="$status_of_intake"; req_evidence="registry/services/verify/intake.sh: $intake_detail";;
      9) if [ "$status_of_ledger" = "done" ] && [ "$status_of_gates" = "done" ]; then req_status='done'; else req_status=partial; fi; req_evidence="tests/small/ledger.test.sh + gates component: ledger=$status_of_ledger, gates=$status_of_gates";;
      10) req_status="$status_of_meter"; req_evidence="tests/small/meter.test.sh: $meter_detail";;
      11) req_status="$status_of_gates"; req_evidence="gates component: $gates_detail";;
      12) req_status="$status_of_entry"; req_evidence="entry component: $entry_detail";;
      13) if [ "$status_of_route" = "done" ] && [ "$status_of_entry" = "done" ]; then req_status='done'; else req_status=partial; fi; req_evidence="route + entry components: route=$status_of_route, entry=$status_of_entry";;
      14) req_status="$status_of_entry"; req_evidence="entry component: $entry_detail";;
      15) if [ "$status_of_meter" = "done" ] && [ "$status_of_route" = "done" ] && [ "$status_of_ledger" = "done" ]; then req_status='done'; else req_status=partial; fi; req_evidence="tests/small/meter.test.sh + route component + tests/small/ledger.test.sh: meter=$status_of_meter, route=$status_of_route, ledger=$status_of_ledger";;
    esac
    case "$req_status" in done) ;; failed|needs-iteration) req_status=partial;; pending) req_status=pending;; not-started) ;; esac
    req_rows="${req_rows}| req$req | $req_status | $req_evidence |"$'\n'
    [ "$req_status" = not-started ] && not_built="${not_built}- req$req: no implementation is owned by the current components."$'\n'
  done

  set +e
  chain="$(FLEET_LEDGER="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}" receipt_verify_chain 2>&1)"; ec=$?
  [ -f "${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}" ] && receipt_count="$(wc -l < "${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}" | tr -d ' ')"
  [ "$ec" -eq 0 ] || blockers="- receipt chain check exited $ec; proof: FLEET_LEDGER='${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}' receipt_verify_chain"$'\n'"$blockers"
  write_status "$generated" "$total" "$usable" "$rows" "$req_rows" "$blockers" "$not_built" "$chain" "$receipt_count"

  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg file "$OUT" --arg generatedAt "$generated" --argjson usable "$usable" --argjson total "$total" --arg chain "$chain" '{file:$file,generatedAt:$generatedAt,usable:$usable,total:$total,receipt_chain:$chain}'
  else
    toon_preamble status 'live suite and requirement evidence' "$generated"
    toon_open summary 1 'usable,total,file'; toon_row "$usable" "$total" "$OUT"
    toon_open components "$total" 'component,status,evidence'
    while IFS='|' read -r name st detail ec; do
      [ -n "$name" ] || continue
      toon_row "$name" "$st" "$detail"
    done <<EOF
$status_rows
EOF
    toon_next 'cat docs/reports/STATUS.md' 'run ./fleet test, the relevant feature adapters, and registry/services/verify/verify.sh — pending rows are unproven'
    if [ "$FULL" -eq 1 ]; then cat "$OUT"; fi
  fi
}
main "$@"
