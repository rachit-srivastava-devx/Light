#!/usr/bin/env bash
# memory.sh — offline hybrid SQLite FTS5 + sqlite-vec correction store (ADOPT.md §2).
#
# SQLite is the right local library for this workload: transactional correction writes plus
# FTS5/BM25 ranked lookup. DuckDB remains the analytic store; AgentRecall-X is retired because
# its keyword/stem/trigram ranking was the measured T29 recall failure and its JSON corpus was
# a linear-scan boundary. The first database open imports every row from insights-index.json.
#
# Rung 3 is anchored by registry/features/memory/memory-check.sh. This adapter owns rungs 1–2: a correction
# becomes a durable database record, and a probe is an explicit hit/miss check.
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/toon.sh"
. "$D/registry/lib/receipt.sh"
. "$D/registry/lib/err.sh"

: "${AGENT_RECALL_ROOT:=$D/state/memory/agentrecall}"
export AGENT_RECALL_ROOT
PY="${FLEET_PYTHON:-$D/var/venv/bin/python}"
[ -x "$PY" ] || PY=python3
STORE="$D/registry/features/memory/memory_store.py"
PROJECT="${FLEET_MEMORY_PROJECT:-fleet}"
JSON=0

usage() {
  printf '%s\n' \
    'memory.sh [--json] remember|recall|probe|migrate ...' \
    '  remember CORRECTION [--id ID] [--severity critical|important|minor]' \
    '  recall CONTEXT [--limit N]     hybrid BM25 + vector ranked retrieval' \
    '  probe LESSON_ID CONTEXT        exit 0 = hit, exit 6 = miss' \
    '  migrate                        import all legacy insights-index.json rows'
}

require_store() {
  require_bin "$PY" 'create .venv or set FLEET_PYTHON to Python 3'
  [ -f "$STORE" ] || die "$ERR_NOTOOL" store_missing 'SQLite memory adapter is missing' 'restore registry/features/memory/memory_store.py'
}

record_receipt() {
  local event="$1" task_id="$2" ec="$3" f
  f="${FLEET_LEDGER:-$D/ledger/RECEIPTS.jsonl}"
  FLEET_LEDGER="$f" receipt_append "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$event" memory "$task_id" \
    not-applicable not-applicable "$ec" 0 0 0 >/dev/null \
    || die "$ERR_GENERIC" receipt_failed 'could not append memory receipt' 'repair the receipt ledger and retry'
}

store() { "$PY" "$STORE" "$@"; }

cmd_remember() {
  local text="${1-}" id="" severity=important out ec
  [ -n "$text" ] || die "$ERR_USAGE" missing_correction 'remember requires a correction string' \
    'memory.sh remember "CORRECTION" [--id ID] [--severity critical|important|minor]'
  shift
  while [ "$#" -gt 0 ]; do
  case "$1" in
      --id) need_val --id "${2-}"; id="$2"; shift 2 ;;
      --severity) need_val --severity "${2-}"; severity="$2"; shift 2 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  case "$severity" in
    critical|important|minor) ;;
    *) die "$ERR_USAGE" invalid_severity 'severity must be critical, important, or minor' 'pass --severity critical|important|minor' ;;
  esac
  [ -n "$id" ] || id="$(printf '%s' "$text" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | cut -c1-24)-$(printf '%s' "$text" | shasum -a 256 | cut -c1-6)"
  require_store
  out="$(store remember --id "$id" --text "$text" --severity "$severity")"; ec=$?
  record_receipt memory_remember "$id" "$ec"
  [ "$ec" -eq 0 ] || die "$ERR_GENERIC" remember_failed 'SQLite memory write failed' 'inspect the database path and retry'
  if [ "$JSON" -eq 1 ]; then jq -cn --argjson result "$out" '{remembered:$result}'; else
    toon_open remembered 1 'id,database,keywords'
    toon_row "$id" "$(printf '%s' "$out" | jq -r '.database')" "$(printf '%s' "$out" | jq -r '.keywords')"
    toon_next "memory.sh probe $id \"CONTEXT\""
  fi
}

cmd_recall() {
  local ctx="${1-}" limit=10 out ec n id source title relevance
  [ -n "$ctx" ] || die "$ERR_USAGE" missing_context 'recall requires a context string' 'memory.sh recall "CONTEXT"'
  shift
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --limit) need_val --limit "${2-}"; limit="$2"; shift 2 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  require_store
  out="$(store search --query "$ctx" --limit "$limit")"; ec=$?
  record_receipt memory_recall "${PROJECT}-recall" "$ec"
  [ "$ec" -eq 0 ] || die "$ERR_GENERIC" recall_failed 'SQLite FTS5 search failed' 'inspect the database path and retry'
  if [ "$JSON" -eq 1 ]; then printf '%s\n' "$out"; return 0; fi
  n="$(printf '%s' "$out" | jq '.results|length')"
  if [ "$n" -eq 0 ]; then toon_empty matches; else
    toon_open matches "$n" 'id,source,title,relevance'
    while IFS=$'\t' read -r id source title relevance; do
      [ -n "$id" ] || continue
      toon_row "$id" "$source" "$title" "$relevance"
    done < <(printf '%s' "$out" | jq -r '.results[]|[.id,.source,.title,.relevance]|@tsv')
  fi
}

cmd_probe() {
  local id="${1-}" ctx="${2-}" out ec rank total relevance hit_ec result_ec verdict hit=false
  [ -n "$id" ] && [ -n "$ctx" ] || die "$ERR_USAGE" probe_args 'probe requires LESSON_ID and CONTEXT' \
    'memory.sh probe LESSON_ID "CONTEXT"'
  require_store
  out="$(store search --query "$ctx" --limit 20)"; ec=$?
  [ "$ec" -eq 0 ] || die "$ERR_GENERIC" recall_failed 'SQLite FTS5 search failed' 'inspect the database path and retry'
  rank="$(printf '%s' "$out" | jq -r --arg id "$id" '[.results[]?.id] | index($id) // empty')"
  [ -n "$rank" ] && rank=$((rank + 1)) || rank=""
  total="$(printf '%s' "$out" | jq -r '(.results // []) | length')"
  relevance="$(printf '%s' "$out" | jq -r --arg id "$id" 'first(.results[]? | select(.id == $id) | .relevance) // empty')"
  printf '%s' "$out" | jq -e --arg id "$id" '.results[]? | select(.id == $id)' >/dev/null 2>&1
  hit_ec=$?
  if [ "$hit_ec" -eq 0 ]; then hit=true; verdict=hit; result_ec=0; else verdict=miss; result_ec="$ERR_INVARIANT"; fi
  record_receipt memory_probe "$id" "$result_ec"
  if [ "$JSON" -eq 1 ]; then
    jq -cn --arg id "$id" --arg ctx "$ctx" --arg rank "$rank" --arg total "$total" --arg relevance "$relevance" \
      --argjson hit "$hit" '{probe:{id:$id,context:$ctx,hit:$hit,rank:($rank|if .=="" then null else tonumber end),returned:($total|tonumber),relevance:($relevance|if .=="" then null else tonumber end)}}'
  else
    toon_open probe 1 'id,hit,rank,returned,relevance'; toon_row "$id" "$verdict" "${rank:-none}" "$total" "${relevance:-none}"
  fi
  exit "$result_ec"
}

cmd_migrate() {
  local out ec
  require_store
  out="$(store migrate)"; ec=$?
  [ "$ec" -eq 0 ] || die "$ERR_GENERIC" migration_failed 'SQLite memory migration failed' 'inspect the legacy corpus and database path'
  printf '%s\n' "$out"
}

main() {
  local cmd="${1-}"
  case "$cmd" in
    --help|-h|'') usage ;;
    remember) shift; cmd_remember "$@" ;;
    recall) shift; cmd_recall "$@" ;;
    probe) shift; cmd_probe "$@" ;;
    migrate) shift; cmd_migrate "$@" ;;
    *) reject_unknown_flag "$cmd" ;;
  esac
}

if [ "${1-}" = '--json' ]; then JSON=1; shift; fi
main "$@"
