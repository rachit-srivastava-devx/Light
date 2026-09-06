# 04 — The Model Router & the Crew

> **Purpose:** how work is driven to different LLMs — the two-tier split (a fast conversational voice vs an
> async crew of cheap workers), the model portfolio with Aug-2026 prices, and the **deterministic router**
> that decides everything with a lookup table, never an LLM. The thesis of this file: **the LLMs supply
> words; deterministic code decides what happens.** Most turns touch no model at all.

---

## 1. The two tiers (and why most turns skip the model)

| Tier | Who | On the hot path? | Job |
|---|---|---|---|
| **Fast voice** | the cheap conversational model | only when genuinely conversational | a short, warm spoken reply (question / chit-chat / re-anchor) |
| **The crew** (async) | the same cheap model, called as workers | no — off the hot path, filler covers | **atomize** a task into steps; **re-atomize** a too-big step; batch/background work |
| **Deterministic** | pure code + cache | yes | the *majority* of turns: `done`/`next`/`pause`, templated check-ins, next-step-from-cache |

The design goal is to make the **deterministic** row carry the most traffic. A focus session is ~15 turns
([README §7](README.md)) but most are "done"/"next"/"stuck" — none of which needs a model at the moment the
user speaks, because the steps were already produced at intake and the next one is in cache
([03 §4](03-VOICE-LATENCY-PIPELINE.md)). The model is on the *critical path* only for a genuine question,
and even then a filler hides its tail ([03 §5](03-VOICE-LATENCY-PIPELINE.md)).

## 2. The deterministic router (a table, not a model)

The router is a **pure function** `route(state, intent, signals) → action` — a lookup table, fully
unit-testable, with **zero LLM calls on the routing decision itself** ([05 §1](05-DETERMINISM-AND-LLM-CONSISTENCY.md)):

```
route(state, intent, stuck_count):
  intent ∈ {done, next, pause}                → DETERMINISTIC: advance/pause; speak next step from cache
  intent == stuck  AND stuck_count < 2         → DETERMINISTIC: templated non-judgmental re-anchor (cached audio)
  intent == stuck  AND stuck_count ≥ 2         → CREW.reatomize(step)          [async · filler covers]
  intent == question OR chitchat               → FAST_VOICE(short, capped)     [hot path · rare]
  state  == INTAKE                             → CREW.atomize(task)            [async · filler covers]
  else                                         → FAST_VOICE(short, capped)
```

Design points an interviewer probes:
- **Routing is deterministic; only the *classifier feeding it* is probabilistic.** The intent label
  (`done/next/stuck/question/…`) is produced on the hot path by a **rule first** (keyword match on
  "done"/"next"/"stuck" — the high-frequency labels), falling to the cheap model **only when the rule is
  ambiguous**. So the common path is rule → table → cache, entirely LLM-free. The classifier is the one
  stochastic input and it is eval'd on its own gold set ([06 §1](06-EVALS-AND-TESTING.md)).
- **Escalation is bounded.** "Stuck" re-anchors deterministically twice; only a *third* stuck triggers a
  crew re-atomize (break the step smaller). Unbounded model escalation is how a stuck moment costs 4× and
  9 s. One hop, then a different mechanism.
- **The router never lets a model choose a side-effect.** Advancing a step, ending the session, and
  spending money are transitions the *table* owns; the model's output is words that get spoken, never a
  command that gets executed. This is the structural version of "not heavily dependent on LLMs."

## 3. The crew — the workers, precisely

Four workers; only one is heavy:

| Worker | Trigger | Sync? | I/O | Model |
|---|---|---|---|---|
| **Atomizer** | INTAKE (once) / reatomize | async | task → schema-locked `[{step_text, est_min, done_signal}]` | cheap (Flash-Lite) |
| **Intent classifier** | every user utterance | hot (tiny) | transcript → `{done,next,stuck,pause,question,chitchat}` | **rule first**, model only if ambiguous |
| **Conversational responder** | question / chit-chat | hot (rare) | utterance + context → ≤ 2-sentence reply | cheap (Flash-Lite) |
| **Check-in phraser** | check-in timer | mostly deterministic | → non-judgmental line | **templated** (cached premium audio); model only on repeated stuck |

The **atomizer is the product-defining worker** — turning "file my taxes" into "open the tax portal" is
where the ADHD value is won or lost. It is schema-locked, validated, and repaired-once before any step can
be spoken ([05 §3](05-DETERMINISM-AND-LLM-CONSISTENCY.md)), and its output quality has its own gold set +
"is-this-atomic" metric ([06 §2](06-EVALS-AND-TESTING.md)). Everything else is deliberately small or
deterministic so the cost and latency live in one place.

## 4. The model portfolio (Aug-2026 prices, $1 = ₹95 — [README §6](README.md))

| Model | $/M in / out | Role | Share of turns |
|---|---|---|---|
| **Rules + on-device + cache** | ₹0 | done/next/pause classify, templated check-ins, next-step audio | **~65 %** |
| **Gemini 2.5 Flash-Lite** | **$0.10 / $0.40** | atomize, ambiguous-classify, converse | **~30 %** |
| **Claude Haiku 4.5** | **$1 / $5** | re-atomize hard tasks, cross-provider failover | **~5 %** |

Blended LLM cost per session (worked): ~1 atomize (~900 in / 180 out) + ~1 re-atomize + ~3 converse
(~400 in / 60 out) + a few ambiguous classifies ≈ **~5 real model calls**, ~6k in / 1k out on Flash-Lite →
`6,000×$0.10/M + 1,000×$0.40/M = $0.0006 + $0.0004 = $0.001` ≈ **₹0.10/session**, budget **~₹0.15** with a
Haiku escalation now and then. This is the **minor** cost line — voice dominates ([08](08-COST-MODEL.md)).
The portfolio principle: **buy the cheapest model that passes the atomizer eval bar; escalate a hard task
one tier, once; keep everything else off the model entirely.**

## 5. Why the crew is text-LLM + TTS, not speech-to-speech

The tempting "one model does it all" is a speech-to-speech model (OpenAI Realtime / Gemini Live) driving
the whole session. It is rejected on **three** grounds, not one:
1. **Cost:** ₹47–260/session vs ~₹0.15 LLM + cached-premium voice ([08 §3](08-COST-MODEL.md)) — 300–1700×.
2. **Determinism:** a text LLM's output can be **schema-locked, validated, and pinned**
   ([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md)); a speech-to-speech black box cannot be gated the same way,
   so the session's control path could no longer be deterministic — violating target 5.
3. **Control of the voice:** the pipeline lets us pin *one* warm voice identity ([03 §4](03-VOICE-LATENCY-PIPELINE.md))
   and cache fixed phrases; S2S re-synthesizes the personality every call.

So the split — **cheap text LLM for words, premium TTS for warmth, deterministic code for control** — is
what buys quality *and* cost *and* determinism at once. S2S is a deferred premium-max experiment.

## 5.1 Token economy — every technique, with its measured saving

The LLM is already the *minor* cost line (~₹0.15–0.2/session, §4), but token discipline also buys **TTFT**
— prompt tokens are prefill, and prefill is latency ([03 §4](03-VOICE-LATENCY-PIPELINE.md)). Techniques, in
order of payoff:

| Technique | Mechanism | Saving |
|---|---|---|
| **Retrieve, don't dump** | send *only* the matched prior task from the context pack, never the 60 KB pack ([12 §3](12-LLD-AND-SESSION-CONTRACT.md)) | the single biggest win: ~15k tokens → ~900 |
| **Memory reuse** | a lexical hit on a repeat task skips the call **entirely** ([12 §3](12-LLD-AND-SESSION-CONTRACT.md)) | **100 %** on repeat tasks |
| **Context/prompt caching** | stable prefix `[system + rubric + pinned few-shots]` cached; Gemini context-cache bills cached input at **−90 %**, Anthropic prompt-cache at **10 %** | ~−60 % of input cost on a ~900-token call with a ~600-token prefix |
| **Strict prefix ordering** | stable → volatile, volatile **last**; one byte earlier invalidates everything after it | protects the above (scar §8.2) |
| **Transcript compaction** | drop disfluencies/backchannels before they ever reach a prompt; send the semantic transcript, not the verbatim one | ~15–25 % of a rambling intake |
| **Output caps** | steps ≤ 120 chars, replies ≤ 2 sentences, hard `max_tokens` | bounds the expensive side (output is 4× input) *and* the tail latency |
| **Batch the non-urgent** | end-of-session embedding/summarization via Batch API at **−50 %** | off the hot path entirely |

**Note the compounding:** caching applies to the prefix, compaction shrinks the volatile tail, and memory
reuse removes the call altogether — so the *distribution* of calls matters more than any single call's size.

### 5.2 MCP — the honest verdict (right tool, wrong layer for Phase 1)

**Recommendation: do not put MCP on the hot path; adopt it as the Phase-2 integration boundary.**

- **Why not now:** MCP's value is letting a model *pull* context on demand instead of receiving a dump.
  Our hot-path context is already tiny (~900 tokens) and **precisely selected by deterministic retrieval**
  before the call ([12 §3](12-LLD-AND-SESSION-CONTRACT.md)) — so there is nothing left to pull. Adding
  tool-call round trips would **add** tokens (tool schemas in every prompt) and **add latency**
  (an extra model round trip inside a 200 ms budget) to buy a saving we already captured a cheaper way.
  Deterministic retrieval beats model-driven retrieval whenever you can *compute* what's relevant — and here
  we can.
- **Where it becomes right:** the moment the orb needs **external systems** — calendar, notes, a real task
  manager, email — the context becomes large, unpredictable, and owned by someone else. That is exactly
  MCP's shape: one protocol, many providers, model-driven selection. It also keeps those integrations
  **outside** our latency budget, since they'd run in the async crew, never the voice turn.
- **Trigger to adopt:** the first external data source ([README §9.2](README.md), Phase 2). Design the crew's
  tool interface MCP-shaped now (a named tool with a typed schema) so adoption is an implementation swap,
  not a refactor — *design for it, don't pay for it yet.*

## 6. Prompt/context caching + provider failover

- **Caching:** the atomizer's system prompt + pinned few-shots are a **stable cached prefix** (Gemini
  context caching bills cached input at −90 %; Anthropic prompt-cache at 10 %). The prompt is assembled in
  strict stability order `[system + rubric (cached)] → [task + light context (volatile, last)]` — one byte
  changed in the prefix = a full-price re-read of everything after it (scar §8.2).
- **Failover:** Gemini Flash-Lite primary; on elevated error/latency (TTFT p99 > 2× baseline over 60 s) the
  router waterfalls to **Haiku** (a *different provider* — real resilience, not same-provider retry). All
  prompts are provider-portable (no proprietary extensions in the template), and the atomizer eval
  ([06](06-EVALS-AND-TESTING.md)) runs against the fallback monthly — an untested fallback is a fiction.
- **Free-tier reach:** Gemini's free tier + Sarvam's ₹1,000 credits + Fish's free tier let a solo dev run
  to ~1–2 K users at ₹0 before any paid spend ([08 §5](08-COST-MODEL.md)) — part of "reachable to many."

## 7. How to test (exactly)

1. **Router correctness (deterministic unit test):** a table of `(state, intent, stuck_count) → expected
   action` covering every branch of §2. Because the router is a pure function, the gate is **100 % — it's
   code, not an eval.** A mismatch is a bug, not a regression.
2. **Intent-classifier eval** (the stochastic input): a **500-utterance** labeled set
   (done/next/stuck/pause/question/chit-chat, ADHD phrasings + Hinglish). **Gate: accuracy ≥ 97 %; the
   costly confusions — `question`→`next` (skips a real question) — weighted and driven to ~0**
   ([06 §1](06-EVALS-AND-TESTING.md)).
3. **Mis-route cost audit:** label each possible mis-route as *down* (UX cost — treat a question as "next")
   or *up* (latency/₹ cost — a model call where a rule would do); assert both are bounded and monitored.
4. **Mix-drift alert:** track the share of turns hitting Flash-Lite vs Haiku vs deterministic; **alert if
   the model (paid) share exceeds budget** (a prompt change that pushes traffic onto the model is a silent
   cost regression, [08 §6](08-COST-MODEL.md)).
5. **Failover drill:** kill the primary provider in staging; assert the router waterfalls to Haiku within
   the breach window and the atomizer still passes its eval on the fallback.

## 8. Operator's scars

1. **The LLM that "decided" to end the session.** An early version let the model emit a `session_complete`
   signal; it occasionally fired one mid-session on an ambiguous "okay I think that's it" that meant one
   *step*, not the session. Ending is now a table transition on an explicit intent + confirmation; the model
   never emits control. (This is target 5, learned.)
2. **The cache-busting rubric tweak.** Editing one word of the atomizer's system prompt at peak invalidated
   the cached prefix for every session at once — a self-inflicted cost + TTFT spike that looked like a
   provider incident. Prompt edits are now off-peak, staggered, and the prompt is versioned
   ([05 §5](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
3. **Same-provider "failover" that wasn't.** v0's fallback was another Gemini endpoint — useless when
   Gemini itself had the incident. The fallback is now a *different provider* (Haiku), tested monthly.
4. **Unbounded stuck-escalation.** "Stuck" used to escalate to the model every time; a genuinely stuck user
   generated a model call per check-in, costing money and adding latency to exactly the person least able to
   tolerate it. Bounded to two deterministic re-anchors, then one re-atomize (§2).

## 9. Interview questions this file answers

- "How do you drive work to different LLMs — walk me through the routing." (§2 — the deterministic table)
- "Does an LLM ever decide what your app does?" (§2, §8.1 — no; the table owns transitions)
- "Which models, at what cost, and what fraction of turns even hit a model?" (§4 — ~65 % touch none)
- "Why not just use one speech-to-speech model for everything?" (§5 — cost + determinism + voice control)
- "How do you make a provider outage a non-event?" (§6 — cross-provider failover, tested)
- "How do you test a router that has a stochastic classifier in it?" (§7 — deterministic router unit-tested 100 %, classifier eval'd separately)
