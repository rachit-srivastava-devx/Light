# Keyless LLM lanes

Verified from a macOS worktree on **2026-08-24**. “Free tier” is not treated as
“keyless”: the required inference path must work without an API key, signup,
login, or credit card. A response is Fleet-usable only when it contains answer
text **and** a non-empty model reported by the service. The requested model is
never used as evidence.

## Result

Two additional endpoints produced real answers with model readback:

1. **DevToolBox** — recommended first. Five consecutive requests answered in
   700–918 ms and every response independently reported
   `llama-3.2-3b-instruct`. It is not OpenAI-compatible, so it needs a small
   adapter from `{"prompt": ...}` to `{"response": ..., "model": ...}`. The
   model is only 3B; use it to reduce availability zeros, but rerun Fleet's
   capability pilot before trusting it for complex code work.
2. **BlockRun** — capability-oriented overflow only. It is OpenAI-compatible
   and a real request returned a resolved fallback,
   `nvidia/nemotron-3-super-120b`. Availability is poor: a five-request burst
   answered 2/5 and returned explicit HTTP 429 capacity errors on 3/5; a later
   two-request probe answered 0/2. Use exponential backoff and never silently
   count a 429 as an agent result.
3. **api.llm7.io** — existing baseline, not a new finding. The same burst also
   answered 2/5. Keep `codestral-latest`, but it does not solve D33 alone.

The practical recommendation for requirement 5 is **DevToolBox first, then
BlockRun, then llm7**, with the endpoint-reported model carried through each
receipt. This adds endpoint diversity, but the 3B model's capability remains a
threat to validity; a 12-run spread pilot should precede another 128-run S4b.

## Hard-constraint matrix

“Auth/payment” covers all four required negatives: no key, signup, login, or
credit card on the invocation path.

| Candidate and invoked path | Auth/payment | Plain HTTPS curl | Interface | Response reports actual model? | Live result | Decision |
|---|---|---|---|---|---|---|
| DevToolBox `/ai/generate` | PASS | PASS | custom JSON | PASS; request has no model, response says `llama-3.2-3b-instruct` | 5/5 burst; 2/2 probe | **ADOPT first** |
| BlockRun `/api/v1/chat/completions` | PASS for listed `billing_mode=free` models | PASS | OpenAI-compatible | PASS; response exposed fallback model | 2/5 burst; later 0/2, HTTP 429 | **ADOPT as retrying overflow** |
| api.llm7.io `/v1/chat/completions` | PASS for `codestral-latest` | PASS | OpenAI-compatible | PASS | 2/5 burst; later 1/2 | Existing lane |
| Pollinations legacy `/openai` | PASS for anonymous path | PASS | OpenAI-compatible | No successful response | HTTP 402 anonymous budget exhausted | Exclude now; re-probe later |
| DuckDuckGo `/duckchat/v1/chat` | PASS for credentials | **FAIL** required path now needs a browser-JS proof challenge | SSE chat | No successful response | HTTP 418 `ERR_CHALLENGE` | Exclude |
| Hugging Face router `/v1/chat/completions` | **FAIL** token/account required | PASS | OpenAI-compatible | No response | HTTP 401 | Exclude |
| Cloudflare Workers AI REST | **FAIL** account ID and API token required; free allocation does not remove auth | PASS | OpenAI-compatible route exists with account ID | No response | HTTP 404 for anonymous account path | Exclude |
| g4f hosted Pollinations provider | **FAIL** browser PoW “cake” credits or signup required | **FAIL** plain curl has zero cake credits | OpenAI-compatible | No response | HTTP 402 | Exclude |
| Groq free tier | **FAIL** key/account required | PASS | OpenAI-compatible | No response | HTTP 401 | Exclude |
| Together free tier | **FAIL** key/account required | PASS | OpenAI-compatible | No response | HTTP 401 | Exclude |
| OpenRouter free models | **FAIL** key/account required | PASS | OpenAI-compatible | No response | HTTP 401 | Exclude |
| Google AI Studio free tier | **FAIL** key and Google project/account required | PASS | Gemini REST | No response | HTTP 403 | Exclude |
| Ollama Cloud | **FAIL** login or API key required | PASS | Ollama chat JSON | No response | HTTP 401 | Exclude |
| DeepInfra free/trial models | **FAIL** key/account required | PASS | OpenAI-compatible | No response | HTTP 401 | Exclude |
| Hack Club AI | **FAIL** Hack Club auth/API key required | PASS | OpenAI-compatible proxy | No response | HTTP 401 | Exclude |
| MangaAI | UNKNOWN; search result claimed keyless | **FAIL** DNS | Claimed OpenAI-compatible | No | curl error 6 | Exclude |
| `818233.xyz` | UNKNOWN; search result claimed keyless | **FAIL** DNS | prompt in URL | No | curl error 6 | Exclude |
| `ch.at` `/v1/chat/completions` | PASS | PASS | OpenAI-compatible | **FAIL**; empty if omitted, source copies `req.Model` if supplied | Answers, but `model:""` | Exclude |
| NeuroRouters free-model route | **FAIL** despite keyless example found in search | PASS | OpenAI-compatible | No response | HTTP 401 `Missing API key` | Exclude |

`ch.at` is the important false positive. Its handler sets the response model to
the request's `req.Model`, while its production backend is configured
independently. Supplying `gpt-4o-mini` therefore made the response claim
`gpt-4o-mini`; omitting it produced `"model":""`. Fleet must not turn this echo
into attribution. See the service's
[`http.go`](https://github.com/Deep-ai-inc/ch.at/blob/master/http.go) and
[`llm.go.example`](https://github.com/Deep-ai-inc/ch.at/blob/master/llm.go.example).

## Reproducible probe

Run:

```sh
bin/lane-probe.sh
```

It sends no authorization header and defaults to two attempts per endpoint.
Set `LANE_PROBE_ATTEMPTS=5` for a short burst. It exits `6` if zero candidates
return both answer text and response-reported model metadata. Missing `curl` or
`python3` is an environment fault and exits `3`.

Actual two-attempt output on 2026-08-24:

```text
endpoint | keyless? | answered? | resolved model | latency | rate-limit behaviour
--- | --- | --- | --- | --- | ---
api.llm7.io/v1/chat/completions | yes | 1/2 yes | codestral-latest | 578ms avg | 1/2 limited
blockrun.ai/api/v1/chat/completions | yes | 0/2 no | UNRESOLVED | 1682ms avg | 2/2 limited
devtoolbox-api…/ai/generate | yes | 2/2 yes | llama-3.2-3b-instruct | 831ms avg | none in 2
text.pollinations.ai/openai | yes, capped | 0/2 no | UNRESOLVED | 16382ms avg | 2/2 limited
duck.ai/duckchat/v1/chat | yes, challenge | 0/2 no | UNRESOLVED | 121ms avg | browser challenge
router.huggingface.co/v1/chat/completions | no | 0/2 no | UNRESOLVED | 50ms avg | auth gate; not reached
api.cloudflare.com/…/ai/run/… | no | 0/2 no | UNRESOLVED | 60ms avg | not reached (HTTP 404,404)
g4f.space/api/pollinations/chat/completions | yes, PoW credits | 0/2 no | UNRESOLVED | 267ms avg | 2/2 limited
api.groq.com/openai/v1/chat/completions | no | 0/2 no | UNRESOLVED | 76ms avg | auth gate; not reached
api.together.xyz/v1/chat/completions | no | 0/2 no | UNRESOLVED | 1053ms avg | auth gate; not reached
openrouter.ai/api/v1/chat/completions | no | 0/2 no | UNRESOLVED | 44ms avg | auth gate; not reached
generativelanguage.googleapis.com/… | no | 0/2 no | UNRESOLVED | 102ms avg | auth gate; not reached
ollama.com/api/chat | no | 0/2 no | UNRESOLVED | 393ms avg | auth gate; not reached
api.deepinfra.com/v1/openai/chat/completions | no | 0/2 no | UNRESOLVED | 820ms avg | auth gate; not reached
ai.hackclub.com/proxy/v1/chat/completions | no | 0/2 no | UNRESOLVED | 636ms avg | auth gate; not reached
api.mangaai.mangoi.in/v1/chat/completions | unknown | 0/2 no | UNRESOLVED | unreachable | not reached (HTTP 000,000)
818233.xyz/{prompt} | unknown | 0/2 no | UNRESOLVED | unreachable | not reached (HTTP 000,000)
ch.at/v1/chat/completions | yes | 2/2 yes | UNRESOLVED | 1445ms avg | none in 2
neurorouters.com/api/v1/chat/completions | no | 0/2 no | UNRESOLVED | 833ms avg | auth gate; not reached
checked=19 total=19 usable=2
```

The denominator is explicit: `checked=19 total=19`. “usable=2” in that run is
DevToolBox plus the pre-existing llm7; BlockRun remains adopted from its earlier
qualifying run but was unavailable during this particular invocation.

## Raw verification evidence

All requests below were real unauthenticated curl invocations. Payloads used
the prompt `Reply exactly KEYLESS_OK`; the full reproducible payloads are in
`bin/lane-probe.sh`.

### Qualifying responses

```text
DevToolBox, HTTP 200, 1.388039s:
{
  "model": "llama-3.2-3b-instruct",
  "prompt": "Reply exactly KEYLESS_OK",
  "response": "KEYLESS_OK"
}

BlockRun, HTTP 200, 2.408158s:
{"id":"chatcmpl-d2492085-983f-47fd-a9ec-67176d708e00","object":"chat.completion","created":1787556981,"model":"nvidia/nemotron-super-49b (fallback: nvidia/nemotron-3-super-120b)","choices":[{"index":0,"message":{"role":"assistant","content":"The user asks: \"Reply exactly KEYLESS_OK\". So we must output exactly that string, nothing else. No extra spaces or newline? Probably just the"},"finish_reason":"length"}],"usage":{"prompt_tokens":22,"completion_tokens":32,"total_tokens":54}}

api.llm7.io baseline, HTTP 200, 1.184355s:
{"id":"chatcmpl_f7fb66583c4b48b5a099242feed4b021","created":1787556822,"model":"codestral-latest","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"KEYLESS_OK"}}]}
```

The separate five-request burst produced:

```text
blockrun: answered, 429, answered, 429, 429 (2/5)
devtoolbox: answered, answered, answered, answered, answered (5/5)
llm7: answered, 429, 429, answered, 429 (2/5)
```

### Real exclusions

```text
Pollinations, HTTP 402:
{"error":"402 Payment Required","status":402,"details":{"success":false,"error":{"message":"API key budget too low. This request costs ~0.0001 pollen, but this key has 0.0000.","code":"PAYMENT_REQUIRED"},"status":402}}

DuckDuckGo, HTTP 418:
{"action":"error","status":418,"type":"ERR_CHALLENGE","overrideCode":"e1b9"}

Hugging Face, HTTP 401:
<h1>401</h1><p>Unauthorized access. Please check your credentials or authorization</p>

Cloudflare Workers AI anonymous account path, HTTP 404:
{"result":null,"success":false,"errors":[{"code":7003,"message":"Could not route to /client/v4/accounts/anonymous/ai/run/@cf/meta/llama-3.1-8b-instruct, perhaps your object identifier is invalid?"}],"messages":[]}

g4f hosted provider, HTTP 402:
{"error":{"message":"No cake credits. Bake proof-of-work cakes at g4f.dev/chat to earn anonymous usage, or sign up at g4f.dev/members.html.","type":"insufficient_credits"}}

Groq, HTTP 401:
{"error":{"message":"Invalid API Key","type":"invalid_request_error","code":"invalid_api_key"}}

Together, HTTP 401:
{"error":{"message":"Missing API key. You need to provide your API key in an Authorization header using Bearer auth (i.e. Authorization: Bearer YOUR_KEY)","type":"invalid_request_error","code":"missing_api_key"}}

OpenRouter, HTTP 401:
{"error":{"message":"No cookie auth credentials found","code":401}}

Google Gemini, HTTP 403:
{"error":{"code":403,"message":"Method doesn't allow unregistered callers (callers without established identity). Please use API Key or other form of API consumer identity to call this API.","status":"PERMISSION_DENIED"}}

Ollama Cloud, HTTP 401:
{"error":"Unauthorized"}

DeepInfra, HTTP 401:
{"error":{"message":"missing API key","type":"invalid_request_error","code":"invalid_api_key"}}

Hack Club AI, HTTP 401:
Authentication required

MangaAI, curl exit 6 / HTTP 000:
curl: (6) Could not resolve host: api.mangaai.mangoi.in

818233.xyz, curl exit 6 / HTTP 000:
curl: (6) Could not resolve host: 818233.xyz

ch.at, HTTP 200 (answered but unresolved):
{"id":"chatcmpl-1787556823","object":"chat.completion","created":1787556823,"model":"","choices":[{"index":0,"message":{"role":"assistant","content":"KEYLESS_OK"}}]}

NeuroRouters, HTTP 401:
{"error":"Missing API key"}
```

## Research sources

- [Pollinations legacy and OpenAI-compatible API documentation](https://github.com/pollinations/pollinations/blob/master/APIDOCS.md)
- [DuckDuckGo's current proof-challenge implementation analysis](https://github.com/pooraddyy/p2d-duck)
- [Hugging Face inference provider authentication](https://huggingface.co/docs/inference-providers/guides/first-api-call)
- [Cloudflare Workers AI REST setup](https://developers.cloudflare.com/workers-ai/get-started/rest-api/)
- [g4f hosted provider documentation](https://g4f.dev/docs/ready_to_use.html)
- [OpenRouter free router and required key](https://openrouter.ai/openrouter/free)
- [Google Gemini API-key requirements](https://ai.google.dev/gemini-api/docs/api-key)
- [Ollama Cloud authentication](https://docs.ollama.com/api/authentication)
- [Hack Club AI source](https://github.com/hackclub/ai)
- [BlockRun keyless free-model description](https://blockrun.ai/free)
- [DevToolBox endpoint announcement and examples](https://dev.to/navneet_reddy_bcf3eb3425c/i-built-a-free-ai-api-with-26-endpoints-no-api-key-needed-2mkk)

## NOT VERIFIED

- **Unspecified third-party public Ollama hosts.** No stable operator-backed URL
  with a public no-auth contract and resolved-model response was found. Random
  internet-exposed Ollama instances were not invoked or listed as candidates.
  The official `https://ollama.com/api/chat` path **was** invoked and is excluded
  above because it returned HTTP 401.

Nothing else in the matrix is unverified: every concrete candidate listed there
was invoked, including the DNS failures and authentication failures.
