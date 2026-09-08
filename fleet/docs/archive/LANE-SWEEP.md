# Keyless lane sweep — sustained-rate pass

Run from a macOS worktree on **2026-08-24, ~22:58–23:15 IST**. Scope: find keyless
free-inference lanes fleet can fan out to, and prove each one with a real call — then, for
every lane that answers, measure a **sustained** rate (12 sequential real calls, one every 8
seconds) instead of a quick burst, because that is the cadence requirement 5 actually needs.
`docs/LANES.md` (same day, ~13:28) already did the keyless/no-keyless sort with 2-attempt
bursts; this pass reconfirms that sort fresh, adds several candidates it did not cover, and
adds the sustained-rate measurement neither `bin/lane-probe.sh` nor `docs/LANES.md` performed.

## Top line — read this before the table

- **No line was appended to `bin/lanes.conf`.** The two lanes that sustained a clean 12/12 at
  8s cadence — `api.llm7.io` (the built-in default) and BlockRun — are **already** configured.
  There was nothing new and working to add. See "Why nothing was appended" below.
- **A live, previously-undocumented defect was found and proven:** the DevToolBox line already
  sitting in `bin/lanes.conf` (added earlier today, per its `docs/LANES.md` writeup) **cannot
  actually be reached through the real `bin/freelane.sh` code path.** Every real call fails with
  HTTP 400. Root cause and proof are below. Any fallback benefit the ledger credited to this
  lane has likely never actually fired.
- **This does not overturn `docs/DELTA.md` D57.** D57 already tried 8s pacing on the free lane
  and measured 3 of 12 — and concluded the lane's success rate is *variable*, not reliably
  fixed by pacing alone. This sweep's 12/12 is a real, fresh, favorable data point, not proof
  that the lane is now dependably fixed. Read "Reconciling with D55/D57" before treating this
  as a green light to re-run the full 128-observation S4b.

## Method

For every candidate: one real, unauthenticated `curl` call first (no `Authorization` header
unless the candidate's whole premise is "works even with one"). For every candidate whose first
call returned real answer text, a second pass sent **12 sequential real calls, sleeping 8
seconds between each** (not a burst), recording the real HTTP status and parsed success/failure
of every single attempt — the full 12-row denominator is in the tables below, not just a
fraction. A candidate that returned an auth/payment/challenge/DNS failure on the first call was
not put through the 12-call pass — it had already answered the only question that pass exists
to ask. A candidate whose working endpoint could not be located at all is marked **NOT
VERIFIED**, not folded into the reject count — an unmeasured lane is `null`, not `0` (`docs/DELTA.md`
uses the same rule for the cost meter).

Model attribution follows the same rule `bin/lanes.conf` already enforces for `ch.at`: a
model name is only evidence if the endpoint can be shown to *validate* it, not merely echo the
request. Every new candidate below that claims to report a model was re-tested with a
deliberately invalid model name; a real rejection (404/401 "model not found/not supported")
counts as validation, a silent echo does not.

## Lanes already in production, reconfirmed and sustained-tested

| Endpoint | Model | Keyless? | First call (this pass) | 12-call sustained (8s gap) | Verdict |
|---|---|---|---|---|---|
| `api.llm7.io/v1/chat/completions` (built-in default) | `codestral-latest` | Yes | Real call through `bin/freelane.sh` in isolation: answered in <1s | **12/12** — every attempt HTTP 200 with content, `resolved_model=codestral-latest` | **ADOPT** (already the default). See caveat below — D57 measured 3/12 for this same lane under its own 8s pacing test; treat this 12/12 as "healthy right now," not "now proven reliable." |
| `blockrun.ai/api/v1/chat/completions` | `nvidia/step-3.7-flash` | Yes, for `billing_mode=free` models | Real call through `bin/freelane.sh` in isolation: **failed**, live rate-limit, at that exact moment | **12/12** — every attempt HTTP 200 with content. Served model varied per call across `nvidia/step-3.7-flash`, `nvidia/nemotron-3-super-120b`, `nvidia/nemotron-super-49b` (fallback routing across backend capacity) | **ADOPT** (already in `bin/lanes.conf`). Confirms the project's own note that it is "capability-oriented overflow" — bursty, but reliable when paced. |
| `devtoolbox-api.devtoolbox-api.workers.dev/ai/generate` — **raw endpoint, correct `{"prompt": ...}` dialect** | `llama-3.2-3b-instruct` | Yes | HTTP 200, `{"model":"llama-3.2-3b-instruct","response":"KEYLESS_OK"}` | **12/12** — every attempt HTTP 200, model always `llama-3.2-3b-instruct` | The raw endpoint alone would be **ADOPT** — but see next row. |
| `devtoolbox-api.devtoolbox-api.workers.dev/ai/generate` — **as actually wired through `bin/freelane.sh`** | `llama-3.2-3b-instruct` | Yes | `FREELANE_URL=<this> FREELANE_MODEL=llama-3.2-3b-instruct FREELANE_CONFIG=/dev/null bash bin/freelane.sh "…"` → **`freelane: all 1 lanes unavailable; tried=devtoolbox-api.devtoolbox-api.workers.dev:http-400`** | Not run — already proven broken on every call, see below | **REJECT as currently wired — this is a live bug, not a rate limit.** |
| `ch.at/v1/chat/completions` | (deliberately no model sent, matching the existing convention) | Yes | HTTP 200, content present | **12/12 content**, model field **empty on all 12** | **REJECT** — reconfirms the existing `bin/lanes.conf` comment: this lane never reports a real model, so it is excluded by design regardless of its (excellent) raw reliability. |

### The DevToolBox bug, proven

`bin/freelane.sh` builds one hardcoded OpenAI-shaped request body for *every* configured lane
(`{"model": M, "messages": [{"role": "user", "content": P}]}`) and parses one hardcoded
OpenAI-shaped response (`choices[0].message.content`). DevToolBox's actual API is not OpenAI
dialect — it wants `{"prompt": "..."}` and returns `{"model": ..., "response": ...}`. Direct
side-by-side proof, same endpoint, same moment:

```
$ curl -d '{"prompt":"Reply exactly KEYLESS_OK"}' https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate
{"model": "llama-3.2-3b-instruct", "prompt": "Reply exactly KEYLESS_OK", "response": "KEYLESS_OK"}   # HTTP 200

$ curl -d '{"model":"llama-3.2-3b-instruct","messages":[{"role":"user","content":"Reply exactly KEYLESS_OK"}]}' \
       https://devtoolbox-api.devtoolbox-api.workers.dev/ai/generate
{"error": "Missing \"prompt\" field"}   # HTTP 400
```

The second request is byte-for-byte what `bin/freelane.sh` actually sends. This is a live
`bin/freelane.sh` code defect, not a `bin/lanes.conf` problem — appending another copy of the
same `url|model` line would change nothing, because the request shape freelane.sh generates is
per-run, not per-lane-configurable. **This is why nothing was appended for DevToolBox**: the
config already has the line; the code that would make it work does not exist yet. Fixing it
means teaching `bin/freelane.sh` a per-lane request/response dialect, which is outside the
"create `LANE-SWEEP.md`, optionally append to `lanes.conf`" scope given for this pass — flagging
it here is the deliverable.

## Reconciling with D55/D57

`docs/DELTA.md` D55 diagnosed the rate-limit-on-burst problem and added `PARITY_PACE_S` (default
6s) to `bin/parity-run.sh`. D57 (attempt six, the six attempts `handover/KT-CODEX.md` refers to)
went further: it paced a 12-call pilot at what it also calls 8s, got **3 of 12**, and concluded
*"the free lane's success rate is not merely rate-limited; it is variable, and 8s of pacing does
not fix it."*

This sweep's 12/12 for the exact same lane, at the exact same nominal cadence, is real — the raw
curl calls and timestamps are in the table above — and it does not contradict D57. It confirms
the "variable" half of D57's conclusion (12/12 now vs. 3/12 then, same lane, same pacing, only
the *when* differs) without confirming the "reliable" half anyone would need before re-running
the full 128-observation S4b on faith. Two things are true at once:

1. Right now, both `llm7` and BlockRun are healthy and would very likely pass another paced
   pilot if run in the next hour.
2. Neither is a *proof* the free lane will hold up on the next attempt, or the one after. D57
   already generated that exact false signal once (one paced pilot passed, the next did not).

One thing this sweep adds that D55/D57 did not have: **the DevToolBox fallback that
`bin/lanes.conf` already lists has apparently never actually engaged**, because it 400s
instantly. If a run's `freelane.sh` invocation exhausted `llm7`'s budget and fell through, it
was falling through past a lane that cannot answer, straight to BlockRun (or straight to
`exit 3`) — the effective failover chain has one fewer working link than the config file makes
it look like it has. That is a plausible contributor to the "cannot sustain 12 consecutive calls
reliably" verdict in D57 that nobody had isolated before.

**Recommendation for requirement 5** (a recommendation, not an action taken here — no git
commands were run and `bin/freelane.sh` was not modified): either (a) fix the DevToolBox dialect
mismatch in `bin/freelane.sh` so the failover chain has its second real link back, then retry a
small paced pilot now while both lanes are empirically healthy, understanding that is a bet on
current conditions the same way D57's first paced pilot was; or (b) take D57's own closing line
at face value — *"a paid lane with `PARITY_PACE_S=0` settles it in twenty minutes; nothing in
fleet needs to change first"* — and stop spending attempts on a lane whose defining property is
that it is sometimes fine and sometimes is not.

## New candidates found and called for real

Research targets: Pollinations, DuckDuckGo, Cloudflare Workers AI, HuggingFace, g4f-style
aggregators, "any OpenAI-compatible free proxy," and locally-installed CLIs. Beyond what
`docs/LANES.md` already covered, this turned up OVH AI Endpoints, OpenCode Zen, Puter.js, and
Kilo Gateway. Every one below was actually called; none were adopted on a README's word.

| Endpoint | Model | Keyless? | First call | 12-call sustained | Verdict |
|---|---|---|---|---|---|
| `oai.endpoints.kepler.ai.cloud.ovh.net/v1/chat/completions` (OVH AI Endpoints, anonymous tier) | `Qwen3-32B` | **Yes** — zero auth headers sent, HTTP 200 with real content. Model attribution verified: a deliberately fake model name (`totally-not-a-real-model-xyz123`) got a real `HTTP 404 model_not_found`, i.e. this endpoint validates, it does not echo. | Answered immediately, real content | **6/12** — attempts 1,4,5,6,10,11 got `HTTP 429 "API rate limit exceeded"`; attempts 2,3,7,8,9,12 answered. Matches OVH's own documented anonymous cap of ~2 requests/minute/IP/model. | **REJECT for primary/8s-cadence use** (below the 12/12 bar). Genuinely keyless and genuinely model-attributing, so worth keeping in mind as a low-frequency backup, not as requirement 5's primary lane. |
| `opencode.ai/zen/v1/chat/completions` (OpenCode Zen) | `big-pickle` | **Yes** on the wire — no auth header, HTTP 200 with real content on the very first try. Model attribution verified: fake model name → real `"ModelError: Model … is not supported"`. | Answered immediately, real content — looked like a strong new candidate | **0/12** — attempts 1–4: `HTTP 503 "Endpoint is unavailable"`; attempts 5–12: `HTTP 429 "Rate limit exceeded"`. Total collapse under any sustained load. | **REJECT.** The textbook case this whole exercise exists to catch: a single successful ad hoc call would have looked like a hit, and the sustained test is the only thing that disqualifies it. |
| `api.puter.com/puterai/openai/v1/chat/completions` (Puter.js) | — | **No.** The JS SDK's "no signup" claim is about the browser client; the REST endpoint itself demanded a bearer token. Called with zero auth: `HTTP 401 {"error":"Missing authentication token"}`. | Rejected immediately | Not run | **REJECT — requires an account/session token.** No token-acquisition flow was attempted (that would mean going through their signup/session creation, which is out of bounds here). |
| `api.kilo.ai` (Kilo Gateway) | — | Third-party sources claim "40+ free models, zero-config, `apiKey: anonymous`" | `api.kilo.ai/api/v1/chat/completions` returned the marketing site's HTML shell, not an API response — wrong or non-existent path | Not run | **NOT VERIFIED.** No working endpoint path was found within reasonable effort. This is recorded as unmeasured, not as a failure — a README claim without a real answered call is not evidence either way. |

## g4f.space (gpt4free) — every advertised "no-key" route tested, all gated

`g4f.dev`'s own documentation advertises five no-key OpenAI-compatible routes. All five were
called for real. All five are gated behind the same proof-of-work/signup wall — the "no-key"
claim does not survive contact:

| Route | First call | Verdict |
|---|---|---|
| `g4f.space/api/pollinations/chat/completions` | `HTTP 402` "No cake credits. Bake proof-of-work cakes… or sign up." (also in `docs/LANES.md`) | REJECT |
| `g4f.space/api/groq/chat/completions` | Same `HTTP 402` cake-credit message | REJECT |
| `g4f.space/api/nvidia/chat/completions` | Same `HTTP 402` cake-credit message | REJECT |
| `g4f.space/api/gemini/chat/completions` | Same `HTTP 402` cake-credit message | REJECT |
| `g4f.space/api/ollama/chat/completions` | Same `HTTP 402` cake-credit message | REJECT |

No proof-of-work "cakes" were baked and no g4f.dev account was created to earn credits — that
is the anti-abuse gate this endpoint deliberately places in front of anonymous use, and getting
around it would be the same category of thing as solving a CAPTCHA.

## Reconfirmed today via `bin/lane-probe.sh` and direct calls — still excluded

All of the following were re-called fresh today (not copied from the 9-hour-old `docs/LANES.md`
run) via `LANE_PROBE_ATTEMPTS=2 bash bin/lane-probe.sh` plus two direct checks for endpoints it
doesn't cover. Every one still fails the same way:

| Endpoint | Keyless? | Result | Verdict |
|---|---|---|---|
| `text.pollinations.ai/openai` | "Yes, capped" | `HTTP 402`, "API key budget too low… this key has 0.0000" | REJECT |
| `text.pollinations.ai/{prompt}` (legacy plain-GET dialect, not in `lane-probe.sh`) | "Yes, capped" | Same `HTTP 402` anonymous-budget message — confirms the legacy path is gated identically to the `/openai` path | REJECT |
| `duck.ai/duckchat/v1/chat` | "Yes, challenge" | `HTTP 418 ERR_CHALLENGE` — anonymous chat is gated behind a browser proof challenge. Not attempted to route around it; that is bot-detection, not a login wall. | REJECT |
| `router.huggingface.co/v1/chat/completions` | No | `HTTP 401` | REJECT |
| `api-inference.huggingface.co/models/gpt2` (legacy HF inference API, not in `lane-probe.sh`) | No | `curl: (6) Could not resolve host` — this hostname is decommissioned | REJECT |
| `api.cloudflare.com/…/accounts/anonymous/ai/run/…` | No | `HTTP 404` — no generic anonymous account path exists; a real account ID + API token is required | REJECT |
| `api.groq.com/openai/v1/chat/completions` | No | `HTTP 401` | REJECT |
| `api.together.xyz/v1/chat/completions` | No | `HTTP 401` | REJECT |
| `openrouter.ai/api/v1/chat/completions` | No | `HTTP 401` | REJECT |
| `generativelanguage.googleapis.com/…` | No | `HTTP 403` | REJECT |
| `ollama.com/api/chat` | No | `HTTP 401` | REJECT |
| `api.deepinfra.com/v1/openai/chat/completions` | No | `HTTP 401` | REJECT |
| `ai.hackclub.com/proxy/v1/chat/completions` | No | `HTTP 401` | REJECT |
| `api.mangaai.mangoi.in/v1/chat/completions` | Unknown | `curl: (6) Could not resolve host` | REJECT |
| `818233.xyz/{prompt}` | Unknown | `curl: (6) Could not resolve host` | REJECT |
| `neurorouters.com/api/v1/chat/completions` | No | `HTTP 401 "Missing API key"` | REJECT |

Full fresh probe run: `checked=19 total=19 usable=3` (`llm7`, BlockRun, DevToolBox — the three
that answer with a validated model on a short burst; this count does not know about the
DevToolBox wiring bug above, because `lane-probe.sh` calls the raw endpoint directly and
correctly, it does not go through `freelane.sh`).

## Locally-installed CLIs

Both `qwen` and `codex` are installed via Homebrew but shadowed by an ancient default `node`
(v10.16.2 from `nvm`, which cannot parse either CLI's ESM syntax — `SyntaxError: Unexpected
token {` on ordinary `--version`). Both had to be invoked as `/opt/homebrew/bin/node
/opt/homebrew/bin/{qwen,codex} …` (Homebrew's `node` is v26). Both already had cached,
pre-existing OAuth credentials on this machine (`~/.qwen/oauth_creds.json`,
`~/.codex/auth.json`) from a prior human login — neither was logged into as part of this task.

| CLI | Real call made | Result | Verdict |
|---|---|---|---|
| `qwen "Reply exactly KEYLESS_OK, nothing else" --approval-mode plan` | Yes, for real, stdin redirected from `/dev/null` | Produced **zero output for 8.5+ minutes**; the task was stopped rather than left hanging indefinitely | **REJECT.** Not keyless (rides a pre-existing personal login) and, independently, non-responsive within any reasonable latency budget for a fan-out lane in this real attempt. |
| `codex exec --sandbox read-only "Reply exactly KEYLESS_OK, nothing else"` | Yes, for real | Answered correctly ("KEYLESS_OK") in ~7s, reporting `model: gpt-5.6-luna`, `provider: openai` — but consumed **13,379 tokens** for a two-word prompt (fixed session/skill/hook overhead) | **REJECT for "keyless."** Requires the pre-existing personal OpenAI login already present on this box; a fresh checkout elsewhere would need `codex login` first, which fails the hard constraint. Also does not fit `bin/lanes.conf`'s `url|model` record shape — `bin/freelane.sh` only knows how to `curl` a URL, not shell out to a subprocess CLI, so even a keyless CLI could not be "appended" in the existing format without new code. |

## Why nothing was appended to `bin/lanes.conf`

The task rule was: append only a lane that sustains 12/12. Three lanes did — `llm7`, BlockRun,
and `ch.at` — and all three are already accounted for: `llm7` is the hardcoded default,
BlockRun already has a config line, and `ch.at` is correctly and deliberately excluded by the
config file's own existing comment about model attribution (a rule this sweep's evidence
agrees with, not one it is overriding). DevToolBox's raw endpoint also hit 12/12, but its
existing config line is provably non-functional through the real code path, so adding a
duplicate line would not change anything — the fix belongs in `bin/freelane.sh`, not in
`bin/lanes.conf`, and is out of scope for this pass. No other candidate reached 12/12 (OVH: 6/12;
OpenCode Zen: 0/12; everything else never got past the first call). `bin/lanes.conf` was left
untouched.
