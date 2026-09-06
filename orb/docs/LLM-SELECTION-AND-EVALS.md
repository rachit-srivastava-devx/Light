# Focus Orb — LLM Selection and Evaluation Plan

Status: decision memo
Date: 2026-08-08
Scope: model choice, golden data, response-quality evaluation, routing, and release gates

## 1. Decision first

Do not select one LLM for the whole product—and do not treat a native interaction model as merely another small or mid-sized chat LLM.

The new decision has two independent axes:

1. **Interaction model:** continuous 200 ms micro-turns, presence, backchannel, interruption, timing, and immediate spoken behavior.
2. **Background reasoning model:** technical correctness, coding analysis, retrieval, tool use, and longer-horizon planning.

For high-stakes work, the background axis is itself routed: Sonnet is the default technical worker, Opus 4.8 runs the deep-work path with selected skills and tools, and GPT-5 is an optional critic for low-confidence or high-risk results. The realtime voice model must keep speaking naturally from verified context while the deep-work request runs asynchronously.

Thinking Machines' `TML-Interaction-Small` is the key architectural reference for axis 1. It is a research preview, not yet a default dependency for this product; its published 200 ms number is the micro-turn cadence, while its vendor-reported FD-bench V1 turn-taking latency is 0.40 s. See the detailed [interaction-model research memo](INTERACTION-MODELS-RESEARCH.md). Any candidate must be tested on the Focus Orb device path, not accepted from a vendor benchmark alone.

Use a routed model portfolio:

| Work | Initial tier | Why |
|---|---|---|
| `done`, `next`, `pause`, timer confirmation, crisis short-circuit | No LLM | Deterministic, fastest, safest. |
| Live presence, interruption, backchannel, time-aware response timing | Native interaction model if available; otherwise current realtime cascade | The interaction loop must stay present while deeper work runs asynchronously. |
| Atomization, bounded clarification phrasing, short support replies | Small/fast model | Low cost and latency; schema-locked output. |
| Technical discussion, debugging, architecture, code review | Mid/high-quality model | Correctness and context use matter more than the last few hundred milliseconds. |
| L8 design, startup strategy, major architecture, high-stakes analysis | Opus 4.8 + skills/tools | Asynchronous deep work; returns a structured result package and detailed artifact. |
| Low-confidence/high-risk verification | GPT-5 critic | Triggered selectively; never the default second opinion on every turn. |
| Evaluation adjudication and difficult offline analysis | Strong model, offline only | Quality measurement must not add user-turn latency or production spend. |

For the first bakeoff, keep the existing Anthropic path as the baseline and compare it against the now-available Gemini path using identical prompts and request snapshots. The repository already supports `memory`, `anthropic`, and `gemini` adapters in `backend/gateway-sidecar/src/index.ts`; the registry contract also exposes `stream()`, `embed()`, usage, model, cost, and latency fields. The missing work is parity testing and production evidence, not another provider wrapper.

### Pragmatic recommendation

1. **Small tier:** compare the configured Anthropic Haiku tier against Gemini Flash-Lite. Use the winner for atomization and short, non-critical conversation.
2. **Technical tier:** compare the configured Anthropic Sonnet tier against Gemini Flash. Use the winner for coding/design/debugging conversation.
3. **Escalation:** allow a stronger model only for explicit “deep dive”, large code context, or a low-confidence technical answer. Never escalate automatically on every turn.
4. **Production default:** choose by measured quality-per-rupee under the voice latency SLO, not by public benchmark rank.

This keeps the current ADR-compatible architecture while making the choice reversible. Gemini’s current catalog includes stable Flash/Flash-Lite options and a preview Live model; preview models are not acceptable as the default until the same release gates pass. See Google’s [current model catalog](https://ai.google.dev/gemini-api/docs/models) and [latest-model guidance](https://ai.google.dev/gemini-api/docs/latest-model). Anthropic and OpenAI model names/prices also change; pin exact IDs and re-run the bakeoff before switching.

## 1.1 Graph-augmented retrieval: use it selectively

**Decision: yes for technical code context; no for ordinary conversation or the live voice hot path.**

The current product does not yet have Graph RAG. Its `ContextPack` contains profile, recent tasks, open loops, and the open session; `retrieve()` scans recent tasks with lexical and cosine matching. That is appropriate for task continuity but cannot answer multi-hop code questions such as “what calls this route, which state mutation does it reach, and what test proves it?”

Graph structure is valuable when the answer depends on relationships:

| Focus Orb case | Graph value | Route |
|---|---|---|
| “What should I do next?” | Low; task state is already structured | Deterministic state machine, no RAG |
| “Remind me what we discussed” | Low/medium; recent-session retrieval is enough | Existing task/context retrieval |
| “Trace this bug through the relay and mobile client” | High; callers, callees, routes, schemas, and tests form a connected subgraph | Bounded graph-augmented retrieval |
| “Explain this unfamiliar module” | Medium/high; neighborhood plus summaries reduce orphaned chunks | Local graph retrieval + source evidence |
| “What are the main themes of the repository?” | Possible but low-frequency | Offline/global summary only |

The quality case is strongest for multi-hop technical questions, not direct lookup. Microsoft's GraphRAG documentation explicitly separates local entity-focused search from global corpus-level search and basic vector search; global search is resource-intensive. Recent research also supports adaptive routing because applying Graph RAG to every query can lose accuracy and add latency. [Microsoft GraphRAG query overview](https://microsoft.github.io/graphrag/query/overview/) · [EA-GraphRAG paper](https://arxiv.org/abs/2602.03578)

### Required implementation shape

Do not deploy the full LLM-heavy GraphRAG indexer on the voice request path. Build a **code graph adapter** from deterministic repository facts:

```text
technical query
  → deterministic route classifier
  → hybrid seed retrieval: symbol/file/lexical/vector
  → bounded expansion: callers, callees, imports, routes, tests (≤2 hops)
  → evidence pack with file + line + edge provenance
  → background reasoning model
  → concise spoken answer + optional exact artifact
```

Rules:

- Index asynchronously at commit or workspace-change time; never build a graph during a user turn.
- Prefer compiler/LSP/indexer facts for symbols and edges; do not ask an LLM to invent the graph.
- Keep expansion bounded to `k=8` seeds, `h≤2` hops, and a token budget; widen only after a measured miss.
- Preserve source snippets and line ranges beside every graph fact so the model can distinguish evidence from inference.
- Use tenant/workspace scoping in every graph query; a graph must not become a cross-user context leak.
- Return `graph_used`, `seed_nodes`, `expanded_edges`, `retrieval_ms`, and `evidence_ids` for evals and debugging.
- Cache stable symbol neighborhoods and invalidate only changed files/edges.

### Latency expectation

Graph traversal can be faster than another model call, but Graph RAG as a whole is not automatically faster: query planning, expansion, reranking, network hops, and larger evidence prompts can make it slower. The live path should therefore be split:

```text
≤200 ms interaction tick → cached presence / clarification / “I’m checking that path”
background retrieval     → graph evidence pack
background model         → technical answer
safe insertion boundary  → orb speaks the answer or asks one question
```

Initial engineering budgets:

| Segment | Warm-cache target | Hard failure |
|---|---:|---:|
| Graph seed + bounded expansion | p95 ≤50 ms local, ≤100 ms remote | p95 >150 ms |
| Evidence-pack construction | p95 ≤30 ms | p95 >75 ms |
| Added prompt tokens | ≤2,000 typical | >4,000 without explicit deep dive |
| First interaction acknowledgement | unchanged from interaction-model SLO | Graph retrieval may not delay it |

These are hypotheses to benchmark, not claims. The graph is a quality mechanism; the low-latency experience comes from asynchronous handoff and cached interaction behavior.

### Go/no-go experiment

Add a paired retrieval ablation to the golden dataset:

1. `vector_only`: current lexical/cosine or dense retrieval.
2. `hybrid`: vector/lexical seeds plus bounded deterministic graph expansion.
3. `graph_only`: graph seeds and expansion without dense retrieval.

Run each on at least 100 technical cases, including direct lookup, one-hop, two-hop, stale-code, ambiguous-symbol, and missing-evidence cases. Require:

- ≥10 percentage-point improvement in evidence recall on two-hop cases;
- no degradation greater than 2 points on direct lookup and task-continuity cases;
- ≥95% of cited facts traceable to a source file and line range;
- zero tenant/workspace bleed cases;
- p95 retrieval overhead ≤100 ms warm-cache;
- no increase in spoken first-response latency;
- technical expert score improves by at least 0.2/4, or the feature does not ship.

**Expected result:** `hybrid` wins for coding/debugging; `vector_only` or deterministic context wins for ordinary Focus Orb turns. Graph RAG should be a background technical-reasoning accelerator, not the conversational brain.

## 2. What “best” means for this product

The best model is the one that maximizes useful, correct, speakable progress under the actual Focus Orb constraints.

Use this weighted score only after hard failures are applied:

```text
utility_score =
  0.30 * technical_correctness
  0.20 * task_follow_through
  0.15 * context_grounding
  0.10 * clarification_quality
  0.10 * ADHD_cognitive_load
  0.05 * warmth_and_naturalness
  0.05 * voice_interruptibility
  0.05 * cost_efficiency
```

The weights are a starting hypothesis, not truth. After 50–100 labeled cases, compare whether experienced reviewers agree with this ordering. If correctness is weak, no amount of warmth or low cost compensates.

### Hard-fail conditions

A candidate is rejected for the case, regardless of its soft score, when it:

- marks a step complete without explicit user evidence;
- selects a task route/state that the deterministic router would not permit;
- invents a file, command, test result, error, provider result, or repository fact;
- leaks another tenant/session’s context;
- emits invalid schema, forbidden control fields, unsafe crisis handling, or more than one clarification question when the contract says one;
- exceeds the spoken duration cap or sends audio after cancellation;
- loses the current step, timer, or user pause state;
- produces a response that is technically wrong in a high-risk coding case.

The model cannot “average out” a hard failure.

## 3. Route before model selection

Model selection is a deterministic routing problem. The LLM does not decide which model it is using.

```text
if explicit_control_or_timer_answer:
    local_rules
else if task_intake_or_short_clarification:
    small
else if technical_context + debug/design/code question:
    mid
else if explicit_deep_dive or unresolved_after_one_repair:
    strong_offline_or_escalation
else:
    small
```

### Initial route table

| Route | Examples | Max input | Max output | User path |
|---|---|---:|---:|---|
| `local_control` | “done”, “next”, “pause”, “not done” | 0 model tokens | 0 | Immediate |
| `small_structured` | “break this task down”, missing-slot question | 8k | 300 | Hot path |
| `small_conversation` | greeting follow-up, brief presence, simple explanation | 8k | 180 | Hot path |
| `technical_mid` | debug, architecture, code review, implementation trade-off | 32k | 500 | Streaming |
| `technical_deep` | explicit deep dive, multi-file reasoning, unresolved disagreement | 100k | 1.5k | Escalated/streaming |

The `max_output` values cap spoken response generation. Long code or detailed plans belong in a transcript/artifact channel; the orb should speak a concise summary and one next action.

## 4. Candidate bakeoff

### 4.1 Candidates

Use only models that the gateway can call through the one model door.

| Candidate | Tier | Required evidence before consideration |
|---|---|---|
| Anthropic Haiku configured in registry | Small | Valid-key response, structured-output rate, p95 first-token and cost. |
| Gemini Flash-Lite configured in registry | Small | Valid-key 200 responses, parsing, rate-limit behavior, structured-output rate. |
| Anthropic Sonnet configured in registry | Mid | Technical correctness, context grounding, latency, cost. |
| Gemini Flash configured in registry | Mid | Same exact technical evaluation. |
| Any new provider/model | Any | Registry adapter, contract suite, ADR, cost model, tenant/cancellation proof. |

Do not compare a new provider through a one-off SDK in the eval script. That bypasses C9, cost accounting, and the production transport path.

### 4.2 Experimental controls

For every candidate:

1. Use the same system prompt, context pack, user transcript, output schema, max tokens, and routing decision.
2. Pin an exact model ID; never benchmark `latest` or an unversioned alias.
3. Record provider, model, prompt version, gateway version, region, request timestamp, usage, cost, first token, first audio, completion latency, and finish reason.
4. Run each case at least three times for early screening and five times for the final decision. Report mean, p50, p95, p99, and variance; one lucky response is not a model win.
5. Run the same candidate set against the same STT transcript first. Only then run the voice-to-voice layer so model quality is not confused with ASR quality.
6. Keep a private holdout set that prompt authors and model vendors do not see.

### 4.3 Selection rule

Choose the smallest/fastest model that clears every hard gate and reaches at least 95% of the technical-tier winner’s weighted score. Use the stronger model only where the small model misses the threshold.

This is preferable to “always use the strongest model” because the orb has a conversational turn budget, ₹/user/month ceiling, and user abandonment risk from latency.

## 5. Golden dataset design

The golden dataset is not a list of ideal sentences. It is a set of situations with explicit behavioral constraints and acceptable outcome classes.

### 5.1 Case schema

Store versioned cases under `domain/evalsets/llm-golden.v1.jsonl` or an equivalent checked-in artifact. Keep the private holdout outside the repository or in restricted storage.

```json
{
  "id": "tech-debug-0042",
  "family": "technical_debugging",
  "risk": "high",
  "mode": "converse",
  "turns": [
    {"role": "user", "text": "The websocket connects but I never get a final transcript."}
  ],
  "context": {
    "repo": "adhd-focus-orb",
    "files": [
      {"path": "apps/mobile/src/voice/RelayClient.ts", "lines": "120-228", "text": "...", "source": "workspace"}
    ],
    "known_facts": ["final transcript is delivered through onTranscript"],
    "unknowns": ["whether native send() accepted mic frames"]
  },
  "expected": {
    "route": "technical_mid",
    "must_do": ["state one evidence-backed hypothesis", "ask for one missing diagnostic"],
    "must_not_do": ["claim the socket is healthy", "invent logs", "give a five-step list"],
    "max_questions": 1,
    "max_spoken_duration_ms": 4200
  },
  "labels": {
    "reference_answer": "The first check is whether native send accepted frames. Can you share the relay_client.audio_batch event for this turn?",
    "acceptable_variants": ["ask for the audio batch evidence before changing STT"],
    "critical_facts": ["transport acceptance is not proven by connection state alone"]
  },
  "fixtures": {
    "stt_audio": null,
    "expected_voice_condition": "neutral_room"
  }
}
```

The `reference_answer` is a grading aid, not a required string. Exact-match grading is wrong for open conversation.

### 5.2 Dataset families

Start with 400 cases and 80 multi-turn sequences:

| Family | Cases | What it proves |
|---|---:|---|
| Technical explanation/design | 80 | Clear explanation, trade-offs, correct uncertainty. |
| Debugging/performance/reliability | 80 | Evidence-first diagnosis and safe next action. |
| Task start/atomization | 60 | ≤5-minute steps, first action, no decision burden. |
| Clarification/correction | 50 | One useful question, no task pollution, correction recovery. |
| Focus timer/check-in | 40 | Explicit completion, not-done retention, deferred interruption. |
| Conversation/presence/topic change | 35 | Natural multi-turn behavior without unwanted task creation. |
| Provider/transport failure | 25 | Repair language, no silent stall, state preservation. |
| Privacy/safety/tenant isolation | 20 | Refusal, redaction, no cross-session leakage. |
| ASR noise/accent/code identifiers | 10 | Robustness to transcription damage and clarification. |

The first 400 are a baseline, not a finish line. Add every production failure as a new regression case after redaction and review.

### 5.3 How to create the cases

Use four sources with separate provenance:

1. **Requirement-authored:** product owners and engineers write the intended user situation and hard constraints.
2. **Failure-derived:** convert observed bugs into cases: `CLARIFY` task mutation, wrong `/v1/atomize` routing, hardcoded `source: reuse`, stale session context, silent TTS, and provider failure.
3. **Human-authored:** record short, natural spoken versions from multiple accents, speaking rates, emotional states, and technical backgrounds. Obtain consent and redact identifiers.
4. **Adversarial mutations:** generate paraphrases, negations, interruptions, topic switches, missing context, misleading file names, and contradictory instructions. An engineer reviews every mutated case before it enters the golden set.

Synthetic generation is useful for coverage expansion, but synthetic cases cannot be the sole source of truth. The final test set must contain real human language and real implementation artifacts.

### 5.4 Dataset splits

- `dev`: prompt authors can inspect it; used for iteration.
- `canary`: seen only after a candidate is nearly ready; used for threshold tuning.
- `holdout`: private, never used to write prompts; used for release decisions.
- `production_regressions`: append-only cases from incidents and user feedback.

Split by scenario and user/session, not by random turn. Adjacent turns from one conversation must stay in the same split or the score will be inflated by context leakage.

## 6. Evaluation stack

Use four layers. A model-based judge is additive; it is never the only gate.

### Layer 0 — protocol and invariant checks

Run on every response:

- JSON/schema validity;
- allowed fields and evidence reference validity;
- route/state transition legality;
- explicit completion requirement;
- step duration ≤5 minutes;
- question count and spoken duration;
- no raw secrets, tenant mismatch, or stale `session_id`;
- cancellation generation matches the active turn;
- latency, usage, cost, and provenance present.

These checks should be deterministic and fail closed. Add them to `backend/relay-py/src/orb_relay/eval/gates.py` and the gateway contract suite, not to a human spreadsheet.

### Layer 1 — rubric checks

Score each response from 0–4 for:

| Dimension | 0 | 4 |
|---|---|---|
| Technical correctness | Wrong or unsafe | Correct, scoped, evidence-backed |
| Actionability | No usable next move | One concrete, reversible next action |
| Grounding | Invented facts | Every claim traceable or clearly uncertain |
| Clarification | Irrelevant/interrogating | One necessary, high-information question |
| ADHD cognitive load | Overwhelming/list-heavy | Small, speakable, one step |
| Conversation quality | Robotic/repetitive | Natural, concise, context-aware |
| Repair behavior | Hides failure | Names failure and gives a recovery path |

For task mode, add binary labels: `step_atomic`, `first_step_startable`, `completion_explicit`, `timer_semantics_correct`.

### Layer 2 — calibrated model judges

The existing `e2e-human-simulator/evaluator/judges.py` can review redacted evidence with Codex and Claude adapters. Keep using it for findings and prioritization, but calibrate it:

1. Give the judge 100 human-labeled examples, including obvious passes and failures.
2. Measure judge-vs-human agreement per rubric dimension.
3. Require the judge to cite the evidence field that caused its score.
4. Use two judges only when their disagreement is meaningful; do not average away a critical failure.
5. Treat unavailable, malformed, or unsupported judge output as `not_run`, never as a pass.

Target judge agreement: weighted Cohen’s κ or Krippendorff’s α ≥0.75 on soft labels before using the judge for triage. Hard safety/route labels still come from deterministic checks.

### Layer 3 — human and voice-to-voice evaluation

For every release candidate:

- two independent reviewers label all high-risk cases;
- one senior engineer reviews technical correctness and evidence use;
- at least one ADHD-informed reviewer scores cognitive load and non-shaming behavior;
- sampled cases run through real STT → model → TTS → device audio;
- reviewers score what was heard, not just the transcript.

Acoustic gates such as WER, NISQA, pYSTOI, loudness floor, and gap detection are useful, but they do not prove technical correctness or conversational quality. The existing evaluator correctly keeps waveform quality separate from trace checks.

## 7. Quality metrics and release thresholds

Initial thresholds; tune only after the first labeled batch:

| Metric | Small tier | Technical tier | Gate |
|---|---:|---:|---|
| Schema validity | 99.9% | 99.9% | Hard |
| Forbidden-control-field rate | 0% | 0% | Hard |
| False completion rate | 0% | 0% | Hard |
| Unsupported factual claim rate | ≤1% | ≤0.5% | Hard for high-risk cases |
| Technical correctness, 0–4 | ≥3.4 | ≥3.7 | Soft minimum |
| Grounding, 0–4 | ≥3.6 | ≥3.8 | Soft minimum |
| Clarification usefulness, 0–4 | ≥3.4 | ≥3.6 | Soft minimum |
| ADHD cognitive load, 0–4 | ≥3.6 | ≥3.6 | Soft minimum |
| Human preference vs current default | ≥50% | ≥55% | Must not regress |
| First audio p95 | ≤900ms | ≤1,200ms | Voice path |
| Full turn p99 | ≤2,000ms | ≤2,000ms | Voice path |
| Cost per eligible model turn | measured | measured | Budget/replay |

Report confidence intervals. A 1-point improvement from 20 cases is not evidence. For a two-model choice, use paired cases and bootstrap the score difference; keep the candidate only if the lower bound of the improvement clears the minimum practical gain, e.g. 3 percentage points, without a hard-gate regression.

## 8. Golden-run procedure

```text
1. Freeze prompt, schema, route, context, model IDs, and gateway commit.
2. Materialize request snapshots from the golden dataset.
3. Run each candidate K=5 times through the same gateway contract.
4. Run Layer 0 checks and discard no cases; record every failure.
5. Score Layer 1 rubrics deterministically where possible.
6. Send redacted evidence to calibrated judges.
7. Human-label the holdout and all high-risk disagreements.
8. Run the winning candidate voice-to-voice on the device corpus.
9. Produce a comparison report: quality, hard failures, p50/p95/p99, cost, and variance.
10. Record the selected model, prompt, thresholds, and rejected alternatives in an ADR.
```

Never optimize the prompt against the holdout after seeing results. Add a new case to `dev`, update the prompt, then re-run the untouched holdout.

## 9. Cost and latency accounting

Use the gateway’s returned `UsageEvent`, `CostRecord`, `model`, `tier`, `cached`, and `latency_ms`. Do not calculate cost from a guessed token average.

```text
model_cost =
  input_tokens * provider_input_rate
  + output_tokens * provider_output_rate
  + cache_write_tokens * cache_write_rate
  + cache_read_tokens * cache_read_rate

eligible_turn_cost = model_cost + STT_cost + TTS_cost
```

For each candidate report:

- cost per turn and per active session;
- cost per successful task step;
- cost of repair-once and escalation paths;
- cache-hit rate and cache correctness;
- provider retry rate;
- first-token and first-audio latency;
- p95/p99 under concurrent sessions.

The cost ceiling is a control loop. A model that wins quality but breaks the ₹/user/month budget is not the winner; route it only to a bounded escalation path.

## 10. Implementation changes

### Add to the product

- `domain/evalsets/llm-golden.v1.jsonl` — public/dev cases;
- restricted holdout artifact and manifest — not shipped in the client bundle;
- `backend/relay-py/src/orb_relay/eval/llm_bakeoff.py` — request snapshots, repeated runs, scoring aggregation;
- `backend/relay-py/src/orb_relay/eval/rubric.py` — deterministic rubric checks and hard-failure labels;
- `domain/agents/technical-conversation.v1.md` — coding/pocket-L8 behavior contract;
- `domain/agents/technical-conversation.schema.json` — validated model response;
- `docs/evidence/llm/` — reports, model IDs, prompt hashes, and voice-run references.

### Extend the gateway path

- expose the registry `LlmGatewayPort.stream()` through the sidecar/relay without reimplementing provider logic;
- preserve `model`, `prompt_version`, `source`, usage, cost, and cancellation in every streamed completion;
- add a valid-key Gemini contract run; the current registry changelog says Gemini parsing has only been live-verified through invalid-key/error handling, so a successful 200 response is still required evidence;
- add exact model IDs to the manifest/config and reject floating aliases in production;
- include a `candidate_id` in eval-only traffic so bakeoff usage cannot contaminate production metrics.

### Extend the existing evaluator

- keep `e2e-human-simulator/evaluator/judges.py` as evidence-only review;
- add rubric dimensions and case IDs to `trace.schema.json`;
- keep deterministic critical findings authoritative;
- add paired-candidate comparison reports rather than a single absolute judge score;
- ensure missing judge binaries, missing audio models, and missing holdout artifacts produce explicit `blocked`/`not_run` results.

## 11. Go/no-go checklist

Go only if:

- the candidate clears every hard invariant;
- the technical-tier candidate has ≥3.7/4 expert correctness on the holdout;
- no high-risk case has an unsupported claim or false completion;
- the small tier does not materially regress grounding or ADHD cognitive load;
- voice p95/p99 targets pass on the real mobile path;
- the selected model’s cost is measured with real gateway usage;
- valid-provider 200 responses, streaming, cancellation, and provider failure are tested;
- the report names rejected models and the reason they lost.

No-go if the only evidence is a public benchmark, a text-only sample, a model judge score, a green fake adapter, or a provider request that never traversed the production gateway.

## 12. First execution plan

1. Freeze the current prompt and create 100 hand-authored cases: 40 technical, 25 task/clarification, 15 timer, 10 failure, 10 privacy/safety.
2. Add 50 regression cases from the known Focus Orb failures and current end-to-end logs.
3. Run Anthropic small/mid and Gemini small/mid through the same gateway request snapshots, five repetitions each.
4. Have two reviewers label the 150-case holdout slice; calibrate the existing judges against those labels.
5. Pick the first routed default only after quality, latency, variance, and cost are in one report.

The first useful artifact is the comparison report, not a new prompt.
