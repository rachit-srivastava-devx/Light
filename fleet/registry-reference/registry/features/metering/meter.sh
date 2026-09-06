#!/usr/bin/env bash
# meter.sh — keyless token adapter.
#
# ccusage numbers are reported by the local Claude/Codex transcript reader. tiktoken numbers are
# counted for the exact text supplied to this command. The two labels are intentionally distinct;
# this adapter never estimates, infers, or substitutes one measurement for the other.
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
. "$D/registry/lib/err.sh"

PY="${FLEET_PYTHON:-$D/var/venv/bin/python}"
[ -x "$PY" ] || PY=python3

usage() {
  printf '%s\n' \
    'meter.sh reported [--json]     ccusage daily totals, labeled reported' \
    'meter.sh count [--model NAME] [--text TEXT|--file PATH]' \
    '                                 tiktoken count, labeled counted'
}

reported() {
  local raw ec fixture="${FLEET_CCUSAGE_JSON:-}"
  if [ -n "$fixture" ]; then
    [ -f "$fixture" ] || die "$ERR_USAGE" fixture_missing "ccusage fixture not found: $fixture" 'set FLEET_CCUSAGE_JSON to a readable JSON file'
    raw="$(cat "$fixture")"; ec=$?
  else
    require_bin ccusage 'install ccusage and retry'
    raw="$(ccusage daily --json --offline </dev/null 2>&1)"; ec=$?
  fi
  [ "$ec" -eq 0 ] || die "$ERR_GENERIC" ccusage_failed "$raw" 'run ccusage daily --json --offline'
  if printf '%s' "$raw" | jq -e 'type == "object" and (.daily | type == "array")' >/dev/null 2>&1; then
    ec=0
  else
    ec=$?
  fi
  [ "$ec" -eq 0 ] || die "$ERR_PARSE" ccusage_unparseable 'ccusage daily did not return the expected JSON shape' 'run ccusage daily --json --offline'
  printf '%s' "$raw" | jq -c '
    def reported: {value:(. // 0), measurement:"reported"};
    {source:"ccusage", measurements:[
      .daily[] as $day |
      (if (($day.modelBreakdowns // []) | length) > 0 then $day.modelBreakdowns else
        [{modelName:(($day.modelsUsed // ["unknown"])[0]), inputTokens:$day.inputTokens,
          outputTokens:$day.outputTokens, cacheCreationTokens:$day.cacheCreationTokens,
          cacheReadTokens:$day.cacheReadTokens, cost:$day.totalCost}]
       end)[] |
      {agent:($day.metadata.agents // [$day.agent // "unknown"]), period:$day.period,
       model:.modelName, input_tokens:(.inputTokens|reported), output_tokens:(.outputTokens|reported),
       cache_creation_tokens:(.cacheCreationTokens|reported), cache_read_tokens:(.cacheReadTokens|reported),
       total_tokens:((.inputTokens // 0)+(.outputTokens // 0)+(.cacheCreationTokens // 0)+(.cacheReadTokens // 0)|reported),
       cost_usd:(.cost // $day.totalCost // 0|reported)}
    ]}'
}

counted() {
  local model=gpt-4o text="" file="" out ec
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --model) need_val --model "${2-}"; model="$2"; shift 2 ;;
      --text) need_val --text "${2-}"; text="$2"; shift 2 ;;
      --file) need_val --file "${2-}"; file="$2"; shift 2 ;;
      -h|--help) usage; return 0 ;;
      *) reject_unknown_flag "$1" ;;
    esac
  done
  [ -z "$file" ] || { [ -f "$file" ] || die "$ERR_USAGE" input_missing "text file not found: $file" 'pass --text or a readable --file'; text="$(<"$file")"; }
  [ -n "$text" ] || [ ! -t 0 ] || die "$ERR_USAGE" input_missing 'count requires --text, --file, or stdin' 'pass the prompt text to count'
  [ -n "$text" ] || text="$(cat)"
  [ -x "$PY" ] || die "$ERR_NOTOOL" tiktoken_python 'tiktoken Python environment is missing' 'run ./fleet setup --only tiktoken'
  out="$(printf '%s' "$text" | FLEET_TOKEN_MODEL="$model" "$PY" -c '
import json, os, sys
import tiktoken

model = os.environ["FLEET_TOKEN_MODEL"]
try:
    encoding = tiktoken.encoding_for_model(model)
    encoding_name = encoding.name
    model_supported = True
except KeyError:
    encoding = tiktoken.get_encoding("o200k_base")
    encoding_name = encoding.name
    model_supported = False
tokens = len(encoding.encode(sys.stdin.read(), disallowed_special=()))
print(json.dumps({"source": "tiktoken", "measurement": "counted", "model": model,
                  "encoding": encoding_name, "model_supported": model_supported,
                  "tokens": {"value": tokens, "measurement": "counted"}}, separators=(",", ":")))
')"; ec=$?
  [ "$ec" -eq 0 ] || die "$ERR_PARSE" tiktoken_failed 'tiktoken could not count the supplied text' 'check the .venv tiktoken installation'
  printf '%s\n' "$out"
}

case "${1-}" in
  reported) shift; [ "$#" -eq 0 ] || reject_unknown_flag "$1"; reported ;;
  count) shift; counted "$@" ;;
  -h|--help|'') usage ;;
  *) reject_unknown_flag "${1-}" ;;
esac
