#!/usr/bin/env bash
# Ordered, keyless inference lanes. Exit: 0 ok · 3 every configured lane unavailable.
set -u

usage='usage: freelane.sh [--model M] <prompt>'
requested_override=''
if [ "${1:-}" = "--model" ]; then
  [ "$#" -ge 2 ] || { echo "$usage" >&2; exit 7; }
  requested_override=$2
  shift 2
fi
[ "$#" -ge 1 ] || { echo "$usage" >&2; exit 7; }
PROMPT=$1

BUDGET_S=${FREELANE_BUDGET_S:-90}
case "$BUDGET_S" in
  ''|*[!0-9]*|0) echo "freelane: FREELANE_BUDGET_S must be a positive integer" >&2; exit 3 ;;
esac

# FREELANE_URL is retained as a test/development override for the built-in first endpoint.
# Extra lanes are newline-separated url|model[|dialect] records in FREELANE_LANES and/or
# bin/lanes.conf. `dialect` selects the request/response shape and defaults to `openai`.
#
# Every lane used to be sent an OpenAI-shaped {"model","messages"} body regardless of what it
# actually accepts. The DevToolBox lane in bin/lanes.conf wants {"prompt":...} and therefore
# answered HTTP 400 "Missing \"prompt\" field" on every call it ever received -- a lane listed as
# adopted that had never once worked. An unknown dialect is REFUSED, never defaulted: silently
# falling back to openai is exactly what hid that bug.
#   openai  request {"model","messages":[{role,content}]}  reply choices[0].message.content
#   prompt  request {"model","prompt"}                     reply .response
default_url=${FREELANE_URL:-https://api.llm7.io/v1/chat/completions}
default_model=${FREELANE_MODEL:-codestral-latest}
default_dialect=${FREELANE_DIALECT:-openai}
FREELANE_DIALECTS='openai prompt'
[ -z "$requested_override" ] || default_model=$requested_override
LANE_URLS=("$default_url")
LANE_MODELS=("$default_model")
LANE_DIALECTS=("$default_dialect")

add_lane() {
  lane_record=$1
  lane_record=${lane_record%%#*}
  [ -n "${lane_record//[[:space:]]/}" ] || return 0
  case "$lane_record" in
    *'|'*) ;;
    *) echo "freelane: ignoring malformed lane (expected url|model[|dialect]): $lane_record" >&2; return 0 ;;
  esac
  lane_url=${lane_record%%|*}
  lane_rest=${lane_record#*|}
  case "$lane_rest" in
    *'|'*) lane_model=${lane_rest%%|*}; lane_dialect=${lane_rest#*|} ;;
    *)     lane_model=$lane_rest;       lane_dialect=openai ;;
  esac
  lane_url=$(printf '%s' "$lane_url" | awk '{$1=$1;print}')
  lane_model=$(printf '%s' "$lane_model" | awk '{$1=$1;print}')
  lane_dialect=$(printf '%s' "$lane_dialect" | awk '{$1=$1;print}')
  [ -n "$lane_url" ] && [ -n "$lane_model" ] || {
    echo "freelane: ignoring empty lane field: $lane_record" >&2
    return 0
  }
  # Refuse, do not default. A lane whose dialect we do not know would be sent the wrong body and
  # fail with an HTTP error that reads like the service being down.
  case " $FREELANE_DIALECTS " in
    *" $lane_dialect "*) ;;
    *) echo "freelane: ignoring lane with unknown dialect '$lane_dialect' (known: $FREELANE_DIALECTS): $lane_url" >&2
       return 0 ;;
  esac
  LANE_URLS+=("$lane_url")
  LANE_MODELS+=("$lane_model")
  LANE_DIALECTS+=("$lane_dialect")
}

if [ -n "${FREELANE_LANES:-}" ]; then
  while IFS= read -r lane_line || [ -n "$lane_line" ]; do add_lane "$lane_line"; done <<< "$FREELANE_LANES"
fi
lane_config=${FREELANE_CONFIG:-"$(cd "$(dirname "$0")" && pwd)/lanes.conf"}
if [ -f "$lane_config" ]; then
  while IFS= read -r lane_line || [ -n "$lane_line" ]; do add_lane "$lane_line"; done < "$lane_config"
fi

command -v curl >/dev/null 2>&1 || { echo "freelane: curl is required" >&2; exit 3; }
command -v python3 >/dev/null 2>&1 || { echo "freelane: python3 is required" >&2; exit 3; }

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT
payload_file="$work_dir/request.json"
started_at=$(date +%s)
tried=()
lane_count=${#LANE_URLS[@]}
lane_index=0

lane_name() {
  python3 - "$1" <<'PY'
from urllib.parse import urlparse
import sys
u = urlparse(sys.argv[1])
print(u.hostname or sys.argv[1])
PY
}

retry_after_seconds() {
  python3 - "$1" <<'PY'
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import sys
value = sys.argv[1].strip()
try:
    seconds = int(value)
except ValueError:
    try:
        when = parsedate_to_datetime(value)
        if when.tzinfo is None:
            when = when.replace(tzinfo=timezone.utc)
        seconds = max(0, int((when - datetime.now(timezone.utc)).total_seconds()))
    except Exception:
        seconds = 0
print(max(0, seconds))
PY
}

for lane_url in "${LANE_URLS[@]}"; do
  lane_model=${LANE_MODELS[$lane_index]}
  lane_dialect=${LANE_DIALECTS[$lane_index]}
  lane_number=$((lane_index + 1))
  now=$(date +%s)
  remaining=$((BUDGET_S - (now - started_at)))
  if [ "$remaining" -le 0 ]; then
    tried+=("$(lane_name "$lane_url"):budget-exhausted")
    lane_index=$((lane_index + 1))
    continue
  fi

  python3 - "$lane_model" "$PROMPT" "$lane_dialect" > "$payload_file" <<'PY'
import json, sys
model, prompt, dialect = sys.argv[1], sys.argv[2], sys.argv[3]
if dialect == "prompt":
    body = {"model": model, "prompt": prompt}
else:
    body = {"model": model, "messages": [{"role": "user", "content": prompt}]}
print(json.dumps(body))
PY
  body_file="$work_dir/body.$lane_number"
  headers_file="$work_dir/headers.$lane_number"
  lanes_left=$((lane_count - lane_index))
  request_timeout=$((remaining / lanes_left))
  [ "$request_timeout" -ge 1 ] || request_timeout=1
  curl_rc=0
  if http_status=$(curl -sS -m "$request_timeout" -D "$headers_file" -o "$body_file" -w '%{http_code}' \
      "$lane_url" -H 'Content-Type: application/json' --data-binary "@$payload_file" 2>"$work_dir/curl.$lane_number"); then
    :
  else
    curl_rc=$?
    http_status=000
  fi

  reason=''
  if [ "$curl_rc" -eq 28 ]; then
    reason=timeout
  elif [ "$curl_rc" -ne 0 ]; then
    reason="transport-$curl_rc"
  elif [ "$http_status" = 429 ]; then
    reason=rate-limit
  elif [ "$http_status" -ge 500 ] 2>/dev/null; then
    reason="http-$http_status"
  elif [ "$http_status" -lt 200 ] 2>/dev/null || [ "$http_status" -ge 300 ] 2>/dev/null; then
    reason="http-$http_status"
  else
    content_file="$work_dir/content.$lane_number"
    metadata_file="$work_dir/meta.$lane_number"
    parse_rc=0
    if python3 - "$body_file" "$content_file" "$metadata_file" "$lane_dialect" <<'PY'
import json, sys
try:
    with open(sys.argv[1], encoding="utf-8") as f:
        data = json.load(f)
except Exception:
    sys.exit(11)
dialect = sys.argv[4] if len(sys.argv) > 4 else "openai"
if "error" in data:
    # DevToolBox returns {"error": "Missing \"prompt\" field"} -- a bare string. .get() on a str
    # raised AttributeError, so a clear gateway message was reported as "unparseable-response".
    err = data["error"]
    message = str(err.get("message", "unknown")) if isinstance(err, dict) else str(err)
    with open(sys.argv[3], "w", encoding="utf-8") as f:
        f.write(message)
    sys.exit(10)
try:
    if dialect == "prompt":
        content = data["response"]
    else:
        content = data["choices"][0]["message"]["content"]
except (KeyError, IndexError, TypeError):
    sys.exit(11)
with open(sys.argv[2], "w", encoding="utf-8") as f:
    f.write(str(content))
with open(sys.argv[3], "w", encoding="utf-8") as f:
    f.write(str(data.get("model", "UNRESOLVED")))
# The endpoint returns real token counts and freelane was discarding them, so the meter recorded
# null for a lane that HAD been measured. Emit them for the caller; absent stays absent (null),
# never 0 -- an unmeasured lane and a zero-cost lane are different facts.
usage = data.get("usage") or {}
with open(sys.argv[3] + ".usage", "w", encoding="utf-8") as f:
    f.write(json.dumps({
        "prompt_tokens": usage.get("prompt_tokens"),
        "completion_tokens": usage.get("completion_tokens"),
        "total_tokens": usage.get("total_tokens"),
    }))
PY
    then
      resolved_model=$(<"$metadata_file")
      lane_labels=''
      if [ "$lane_number" -gt 1 ]; then
        # nosemgrep: bash.lang.security.ifs-tampering.ifs-tampering -- IFS is set only inside this
        # command substitution's own subshell (the `;` starts a new command in that subshell, not
        # the caller's shell), scoped to exactly the `${tried[*]}` join it exists to control. B11.
        lane_labels=$(IFS=,; echo "${tried[*]}")
        lane_labels="$lane_labels,"
      fi
      lane_labels="$lane_labels$(lane_name "$lane_url"):answered"
      cat "$content_file"
      printf '\n'
      lane_usage=''
      [ -r "$metadata_file.usage" ] && lane_usage=" usage=$(tr -d '\n' < "$metadata_file.usage")"
      echo "[resolved_model=$resolved_model requested=$lane_model lane=$lane_number/$lane_count tried=$lane_labels$lane_usage]" >&2
      exit 0
    else
      parse_rc=$?
    fi
    if [ "$parse_rc" -eq 10 ]; then
      gateway_message=$(<"$metadata_file")
      case "$(printf '%s' "$gateway_message" | tr '[:upper:]' '[:lower:]')" in
        *busy*|*overload*) reason=busy ;;
        *rate*limit*|*too*many*) reason=rate-limit ;;
        *) reason=gateway-error ;;
      esac
    else
      reason=unparseable-response
    fi
  fi

  tried+=("$(lane_name "$lane_url"):$reason")
  retry_after=$(awk 'BEGIN{IGNORECASE=1} /^Retry-After:/ {sub(/^[^:]*:[[:space:]]*/, ""); sub(/\r$/, ""); print; exit}' "$headers_file" 2>/dev/null || true)
  if [ -n "$retry_after" ]; then
    delay=$(retry_after_seconds "$retry_after")
    now=$(date +%s)
    remaining=$((BUDGET_S - (now - started_at)))
    later=$((lane_count - lane_number))
    sleep_cap=$((remaining - later))
    [ "$sleep_cap" -lt 0 ] && sleep_cap=0
    [ "$delay" -le "$sleep_cap" ] || delay=$sleep_cap
    [ "$delay" -le 0 ] || sleep "$delay"
  fi
  lane_index=$((lane_index + 1))
done

# nosemgrep: bash.lang.security.ifs-tampering.ifs-tampering -- same subshell-scoped join as above.
tried_text=$(IFS=,; echo "${tried[*]}")
echo "freelane: all $lane_count lanes unavailable; tried=$tried_text" >&2
exit 3
