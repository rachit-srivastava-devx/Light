#!/usr/bin/env bash
# Probe public LLM endpoints without sending credentials. A response counts as a
# usable Fleet lane only when it contains both answer text and a non-empty model
# reported by the endpoint. Requested model names are never used as evidence.
set -u

for tool in curl python3; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "lane-probe: missing required tool: $tool" >&2
    exit 3
  fi
done

ATTEMPTS="${LANE_PROBE_ATTEMPTS:-2}"
case "$ATTEMPTS" in
  ''|*[!0-9]*|0)
    echo "lane-probe: LANE_PROBE_ATTEMPTS must be a positive integer" >&2
    exit 6
    ;;
esac

TOTAL=0
CHECKED=0
USABLE=0

printf '%s\n' 'endpoint | keyless? | answered? | resolved model | latency | rate-limit behaviour'
printf '%s\n' '--- | --- | --- | --- | --- | ---'

probe() {
  name="$1"
  keyless="$2"
  method="$3"
  url="$4"
  dialect="$5"
  payload="$6"

  TOTAL=$((TOTAL + 1))
  answered_count=0
  rate_count=0
  auth_count=0
  challenge_count=0
  http_count=0
  latency_total_ms=0
  resolved=''
  codes=''
  attempt=1

  while [ "$attempt" -le "$ATTEMPTS" ]; do
    if [ "$method" = POST ]; then
      raw="$(curl -sS --max-time 35 -w '\n__LANE_META__%{http_code} %{time_total}' \
        -H 'Content-Type: application/json' -d "$payload" "$url" 2>&1)"
    else
      raw="$(curl -sS --max-time 35 -w '\n__LANE_META__%{http_code} %{time_total}' \
        "$url" 2>&1)"
    fi
    curl_rc=$?

    parsed="$(python3 -c '
import json, sys

raw, curl_rc, dialect = sys.argv[1:4]
body, marker, meta = raw.rpartition("\n__LANE_META__")
http = "000"
latency_ms = 0
if marker:
    fields = meta.strip().split()
    if fields:
        http = fields[0]
    if len(fields) > 1:
        try:
            latency_ms = int(float(fields[1]) * 1000 + 0.5)
        except ValueError:
            pass

answered = False
model = ""
detail = ""
data = None
try:
    data = json.loads(body)
except Exception:
    data = None

if isinstance(data, dict):
    candidate = data.get("model")
    if isinstance(candidate, str):
        model = candidate.strip()
    if dialect == "openai":
        choices = data.get("choices")
        if isinstance(choices, list) and choices:
            message = choices[0].get("message", {}) if isinstance(choices[0], dict) else {}
            content = message.get("content") if isinstance(message, dict) else None
            answered = isinstance(content, str) and bool(content.strip())
    elif dialect == "ollama":
        message = data.get("message", {})
        content = message.get("content") if isinstance(message, dict) else None
        answered = isinstance(content, str) and bool(content.strip())
    elif dialect == "devtoolbox":
        content = data.get("response")
        answered = isinstance(content, str) and bool(content.strip())

    error = data.get("error", "")
    if isinstance(error, dict):
        detail = str(error.get("message", error.get("type", "")))
    elif error:
        detail = str(error)
elif dialect == "plain" and http.startswith("2"):
    answered = bool(body.strip())

if not detail and not answered:
    detail = body.strip().replace("\n", " ")[:100]

def clean(value):
    return str(value).replace("|", "/").replace("\n", " ").replace("\r", " ")

limited = http in {"402", "429"} or "rate limit" in detail.lower() or "capacity exhausted" in detail.lower()
auth = http in {"401", "403"} or "api key" in detail.lower() or "authentication required" in detail.lower()
challenge = http == "418" or "challenge" in detail.lower()
print("|".join(clean(v) for v in (
    int(answered), model, http, latency_ms, int(limited), int(auth), int(challenge), curl_rc, detail
)))
' "$raw" "$curl_rc" "$dialect")"

    old_ifs="$IFS"
    IFS='|'
    read -r answered model http latency_ms limited auth challenge parsed_curl_rc detail <<EOF
$parsed
EOF
    IFS="$old_ifs"
    if [ -z "$parsed_curl_rc" ]; then
      echo "lane-probe: parser produced no curl status for $name" >&2
      exit 6
    fi
    if [ "${LANE_PROBE_VERBOSE:-0}" = 1 ] && [ -n "$detail" ]; then
      printf '%s: attempt %s: %s\n' "$name" "$attempt" "$detail" >&2
    fi

    if [ "$answered" = 1 ]; then
      answered_count=$((answered_count + 1))
    fi
    if [ -n "$model" ]; then
      if [ -z "$resolved" ]; then
        resolved="$model"
      elif [ "$resolved" != "$model" ]; then
        resolved="varies: $resolved, $model"
      fi
    fi
    if [ "$limited" = 1 ]; then rate_count=$((rate_count + 1)); fi
    if [ "$auth" = 1 ]; then auth_count=$((auth_count + 1)); fi
    if [ "$challenge" = 1 ]; then challenge_count=$((challenge_count + 1)); fi
    if [ "$http" != 000 ]; then
      http_count=$((http_count + 1))
      latency_total_ms=$((latency_total_ms + latency_ms))
    fi
    if [ -z "$codes" ]; then codes="$http"; else codes="$codes,$http"; fi
    attempt=$((attempt + 1))
  done

  CHECKED=$((CHECKED + 1))
  if [ "$http_count" -gt 0 ]; then
    latency="$((latency_total_ms / http_count))ms avg"
  else
    latency='unreachable'
  fi

  if [ "$rate_count" -gt 0 ]; then
    rate="$rate_count/$ATTEMPTS limited"
  elif [ "$challenge_count" -gt 0 ]; then
    rate='browser challenge'
  elif [ "$auth_count" -gt 0 ]; then
    rate='auth gate; not reached'
  elif [ "$answered_count" -eq "$ATTEMPTS" ]; then
    rate="none in $ATTEMPTS"
  elif [ "$answered_count" -gt 0 ]; then
    rate="$answered_count/$ATTEMPTS answered"
  else
    rate="not reached (HTTP $codes)"
  fi

  if [ -z "$resolved" ]; then resolved='UNRESOLVED'; fi
  if [ "$answered_count" -gt 0 ]; then answered_text="$answered_count/$ATTEMPTS yes"; else answered_text="0/$ATTEMPTS no"; fi

  # Model attribution is part of usability, not an informational extra.
  if [ "$answered_count" -gt 0 ] && [ "$resolved" != UNRESOLVED ]; then
    USABLE=$((USABLE + 1))
  fi

  printf '%s | %s | %s | %s | %s | %s\n' \
    "$name" "$keyless" "$answered_text" "$resolved" "$latency" "$rate"
}

openai_prompt='{"messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64,"stream":false}'

probe 'https://api.llm7.io/v1/chat/completions' yes POST \
  'https://api.llm7.io/v1/chat/completions' openai \
  '{"model":"codestral-latest","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64,"stream":false}'
probe 'https://blockrun.ai/api/v1/chat/completions' yes POST \
  'https://blockrun.ai/api/v1/chat/completions' openai \
  '{"model":"nvidia/step-3.7-flash","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64,"stream":false}'
probe 'https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate' yes POST \
  'https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate' devtoolbox \
  '{"prompt":"Reply exactly KEYLESS_OK"}'
probe 'https://text.pollinations.ai/openai' 'yes, capped' POST \
  'https://text.pollinations.ai/openai' openai \
  '{"model":"openai","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64,"stream":false}'
probe 'https://duck.ai/duckchat/v1/chat' 'yes, challenge' POST \
  'https://duck.ai/duckchat/v1/chat' openai \
  '{"model":"gpt-4o-mini","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}]}'
probe 'https://router.huggingface.co/v1/chat/completions' no POST \
  'https://router.huggingface.co/v1/chat/completions' openai \
  '{"model":"meta-llama/Llama-3.1-8B-Instruct","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://api.cloudflare.com/client/v4/accounts/anonymous/ai/run/@cf/meta/llama-3.1-8b-instruct' no POST \
  'https://api.cloudflare.com/client/v4/accounts/anonymous/ai/run/@cf/meta/llama-3.1-8b-instruct' openai \
  '{"messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}]}'
probe 'https://g4f.space/api/pollinations/chat/completions' 'yes, PoW credits' POST \
  'https://g4f.space/api/pollinations/chat/completions' openai \
  '{"model":"openai","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://api.groq.com/openai/v1/chat/completions' no POST \
  'https://api.groq.com/openai/v1/chat/completions' openai \
  '{"model":"llama-3.1-8b-instant","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://api.together.xyz/v1/chat/completions' no POST \
  'https://api.together.xyz/v1/chat/completions' openai \
  '{"model":"meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://openrouter.ai/api/v1/chat/completions' no POST \
  'https://openrouter.ai/api/v1/chat/completions' openai \
  '{"model":"openrouter/free","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent' no POST \
  'https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent' openai \
  '{"contents":[{"parts":[{"text":"Reply exactly KEYLESS_OK"}]}]}'
probe 'https://ollama.com/api/chat' no POST \
  'https://ollama.com/api/chat' ollama \
  '{"model":"gpt-oss:20b","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"stream":false}'
probe 'https://api.deepinfra.com/v1/openai/chat/completions' no POST \
  'https://api.deepinfra.com/v1/openai/chat/completions' openai \
  '{"model":"meta-llama/Llama-3.1-8B-Instruct","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://ai.hackclub.com/proxy/v1/chat/completions' no POST \
  'https://ai.hackclub.com/proxy/v1/chat/completions' openai \
  '{"model":"qwen/qwen3-32b","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://api.mangaai.mangoi.in/v1/chat/completions' unknown POST \
  'https://api.mangaai.mangoi.in/v1/chat/completions' openai \
  '{"model":"deepseek-r1","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'
probe 'https://818233.xyz/{prompt}' unknown GET \
  'https://818233.xyz/Reply+exactly+KEYLESS_OK' plain ''
# Deliberately omit model: ch.at copies req.model into its response while its
# production backend is independently fixed. An empty field exposes the lack
# of resolved-model readback instead of manufacturing attribution.
probe 'https://ch.at/v1/chat/completions' yes POST \
  'https://ch.at/v1/chat/completions' openai "$openai_prompt"
probe 'https://neurorouters.com/api/v1/chat/completions' no POST \
  'https://neurorouters.com/api/v1/chat/completions' openai \
  '{"model":"cognitivecomputations/dolphin-mistral-24b-venice-edition:free","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}],"max_tokens":64}'

printf 'checked=%s total=%s usable=%s\n' "$CHECKED" "$TOTAL" "$USABLE"
if [ "$CHECKED" -eq 0 ] || [ "$TOTAL" -eq 0 ] || [ "$USABLE" -eq 0 ]; then
  exit 6
fi
exit 0
