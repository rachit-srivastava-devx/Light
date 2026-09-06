#!/usr/bin/env bash
# memory-check.sh — rung-3 anchor: enforce promoted AgentRecall corrections on a diff.
#
# It reads the latest machine-checkable signature for each promoted category, confirms that the
# correction is present in the memory store, and checks only added source lines. It owns no
# supervision or gate orchestration; registry/features/gate/gate.sh supplies the diff and remains the gate adapter.
#
# The presence check (rung 3's gate on promotion) accepts a hit from EITHER durable store: the
# SQLite memories table that registry/features/memory/memory.sh writes (state/memory/agentrecall/
# fleet-memory.sqlite3), or the legacy state/memory/agentrecall/insights-index.json. This is what
# lets a lesson stored today via `memory.sh remember` become an enforceable promotion without a
# second hand-written JSON record. JSON remains required and validated up front so the 54 legacy
# insights keep working unchanged; SQLite is consulted first and, when it is missing or unreadable,
# this falls back to JSON alone and says so on stderr -- it never crashes and never silently passes.
set -u

D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/err.sh"

ROOT="$D"
DIFF=""
PROMOTIONS=""
INSIGHTS=""
DB=""
TMP_ROOT=""
VIOLATIONS=0
SIGNATURES=0

usage() {
  printf '%s\n' \
    'memory-check.sh --diff PATH|- [--root PATH] [--promotions PATH] [--insights PATH] [--db PATH]' \
    '  exits 6 when an added source line matches a promoted correction signature' \
    '  a promoted correction must be present in the SQLite memory store or insights-index.json'
}

cleanup() { [ -n "$TMP_ROOT" ] && rm -rf "$TMP_ROOT" 2>/dev/null || true; }
trap cleanup EXIT HUP INT TERM

while [ "$#" -gt 0 ]; do
  case "$1" in
    --diff) need_val --diff "${2-}"; DIFF="$2"; shift 2;;
    --root) need_val --root "${2-}"; ROOT="$2"; shift 2;;
    --promotions) need_val --promotions "${2-}"; PROMOTIONS="$2"; shift 2;;
    --insights) need_val --insights "${2-}"; INSIGHTS="$2"; shift 2;;
    --db) need_val --db "${2-}"; DB="$2"; shift 2;;
    --help|-h) usage; exit "$ERR_OK";;
    --*) reject_unknown_flag "$1";;
    *) reject_unknown_flag "$1";;
  esac
done

[ -n "$DIFF" ] || die "$ERR_USAGE" diff_missing 'memory-check requires --diff PATH|-' 'pass --diff <git diff file> or --diff -'
PROMOTIONS="${PROMOTIONS:-$ROOT/ledger/PROMOTIONS.jsonl}"
INSIGHTS="${INSIGHTS:-$ROOT/state/memory/agentrecall/insights-index.json}"
DB="${DB:-$ROOT/state/memory/agentrecall/fleet-memory.sqlite3}"
require_bin jq 'install jq and retry'
require_bin awk 'install awk and retry'
require_bin grep 'install grep and retry'
require_bin sed 'install sed and retry'

[ -f "$PROMOTIONS" ] || die "$ERR_NOTOOL" promotions_missing 'promotions ledger is missing' 'restore ledger/PROMOTIONS.jsonl'
[ -f "$INSIGHTS" ] || die "$ERR_NOTOOL" insights_missing 'AgentRecall insights index is missing' 'restore state/memory/agentrecall/insights-index.json'
jq -e . "$INSIGHTS" >/dev/null 2>&1
ec=$?
[ "$ec" -eq 0 ] || die "$ERR_PARSE" insights_unparseable 'AgentRecall insights index is not valid JSON' 'repair state/memory/agentrecall/insights-index.json'

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fleet-memory-check.XXXXXX")" \
  || die "$ERR_GENERIC" temp_failed 'could not create memory-check temporary directory' 'check the temporary directory'
DIFF_FILE="$TMP_ROOT/diff"
if [ "$DIFF" = '-' ]; then
  cat > "$DIFF_FILE"
  ec=$?
else
  cat "$DIFF" > "$DIFF_FILE"
  ec=$?
fi
[ "$ec" -eq 0 ] || die "$ERR_PARSE" diff_unreadable 'the supplied diff could not be read' 'pass a readable diff file or --diff -'

# SQLite is the primary presence store; state/memory/agentrecall/insights-index.json (checked
# above) is the required fallback. A missing or unreadable database degrades to JSON-only and
# says so -- it never dies for this reason, and the per-signature check below still refuses a
# correction that is truly absent from both stores.
MEMORIES_JSON="$TMP_ROOT/memories.json"
SQLITE_OK=0
if command -v sqlite3 >/dev/null 2>&1 && [ -f "$DB" ] && [ -r "$DB" ]; then
  sqlite3 "$DB" "SELECT json_group_array(json_object('title', title, 'body', body)) FROM memories;" \
    > "$MEMORIES_JSON" 2>/dev/null
  ec=$?
  if [ "$ec" -eq 0 ]; then
    jq -e . "$MEMORIES_JSON" >/dev/null 2>&1
    ec=$?
    [ "$ec" -eq 0 ] && SQLITE_OK=1
  fi
fi
[ "$SQLITE_OK" -eq 1 ] || printf 'memory-check: sqlite memory store unavailable (%s); falling back to insights-index.json only\n' "$DB" >&2

ADDED="$TMP_ROOT/added.tsv"
awk '
  /^\+\+\+ / { path=substr($0,5); sub(/^b\//, "", path); next }
  /^\+[^+]/ && path != "/dev/null" { print path "\t" substr($0,2) }
' "$DIFF_FILE" > "$ADDED"
ec=$?
[ "$ec" -eq 0 ] || die "$ERR_PARSE" diff_unparseable 'the supplied diff could not be parsed' 'pass a unified git diff'

ROWS="$TMP_ROOT/promotions.tsv"
jq -sr '
  map(select((.artifact? // "") != ""))
  | sort_by(.category, (.receipt_line // 0))
  | group_by(.category)
  | map(.[-1])
  | .[]
  | select((.pattern? // "") != "")
  | [.category, .pattern, (.scope // "source"), (.what_failed // ""), (.artifact // "")]
  | @tsv
' "$PROMOTIONS" > "$ROWS" 2>/dev/null
ec=$?
[ "$ec" -eq 0 ] || die "$ERR_PARSE" promotions_unparseable 'promotions ledger is not valid JSONL' 'repair ledger/PROMOTIONS.jsonl'

source_path() {
  # bin/* remains a historical diff namespace in the learning fixtures; live code is now under registry/.
  case "$1" in bin/*|registry/features/*|registry/services/*|registry/modules/*|registry/lib/*.sh) return 0;; *) return 1;; esac
}

exempt_path() {
  local path="$1" artifact="$2" exempt
  case "$path" in
    registry/features/memory/memory-check.sh|ledger/promotions/*|ledger/fixtures/*|state/memory/learning/fixtures/*|tests/*|state/memory/agentrecall/*) return 0;;
  esac
  [ "$path" = "$artifact" ] && return 0
  while IFS= read -r exempt; do
    [ -n "$exempt" ] || continue
    # Exemptions are deliberately shell globs from the promotion contract.
    # shellcheck disable=SC2254
    case "$path" in $exempt) return 0;; esac
  done <<EOF
$(jq -r --arg artifact "$artifact" '.[] | select(.artifact == $artifact) | (.exemptions? // [])[]?' "$PROMOTIONS" 2>/dev/null)
EOF
  return 1
}

normalize_pattern() {
  # PROMOTIONS.jsonl stores JSON-escaped EREs. Older promotion rows contain one extra escape
  # layer; the executable promotion artifacts and fixtures establish the intended ERE.
  printf '%s' "$1" | sed 's/\\\\/\\/g'
}

while IFS="$(printf '\t')" read -r category pattern scope what_failed artifact; do
  [ -n "$category" ] || continue
  pattern="$(normalize_pattern "$pattern")"
  SIGNATURES=$((SIGNATURES+1))
  hit=1
  if [ "$SQLITE_OK" -eq 1 ]; then
    jq -e --arg category "$category" --arg what "$what_failed" \
      'any(.[]; ((.title // "") | contains($category)) or ((.body // "") | contains($category)) or ((.title // "") | contains($what)) or ((.body // "") | contains($what)))' \
      "$MEMORIES_JSON" >/dev/null 2>&1
    hit=$?
  fi
  if [ "$hit" -ne 0 ]; then
    jq -e --arg category "$category" --arg what "$what_failed" \
      'any(.insights[]; ((.title // "") | contains($category)) or ((.title // "") | contains($what)))' \
      "$INSIGHTS" >/dev/null 2>&1
    hit=$?
  fi
  [ "$hit" -eq 0 ] || die "$ERR_INVARIANT" correction_missing \
    "promoted correction '$category' is absent from AgentRecall-X" \
    'restore the correction in state/memory/agentrecall/insights-index.json or store it via memory.sh remember before promoting it'

  empty="$TMP_ROOT/empty"
  : > "$empty"
  grep -E -q -- "$pattern" "$empty" 2>/dev/null
  ec=$?
  [ "$ec" -ne 2 ] || die "$ERR_PARSE" signature_unparseable \
    "promoted signature '$category' is not a valid ERE" 'repair the latest promotion signature'

  [ "$scope" = source ] || continue
  while IFS="$(printf '\t')" read -r path line; do
    [ -n "$path" ] || continue
    source_path "$path" || continue
    exempt_path "$path" "$artifact" && continue
    if printf '%s\n' "$line" | grep -E -q -- "$pattern"; then
      printf 'memory-check: rejected %s (%s) in %s\n' "$category" "$pattern" "$path" >&2
      VIOLATIONS=$((VIOLATIONS+1))
    fi
  done < "$ADDED"
done < "$ROWS"

[ "$VIOLATIONS" -eq 0 ] || die "$ERR_INVARIANT" memory_violation \
  "$VIOLATIONS added line(s) match promoted correction signatures" \
  'remove the recurrence or update the correction through the promotion process'
printf 'memory-check: clean (%s promoted signature(s) checked)\n' "$SIGNATURES"
exit "$ERR_OK"
