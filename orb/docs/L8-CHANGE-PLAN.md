# Focus Orb — L8 Implementation Change Plan

Status: implementation plan
Date: 2026-08-09
Scope: all changes learned from the interaction-model, model-selection, Graph RAG, and study-with-me discussions.

This file is the execution plan. The detailed design notes remain useful, but this document answers one question: **what must change in the product, contracts, runtime, evaluation system, and operating process to deliver the intended experience?**

## 1. Target product

Focus Orb is an orb-only, voice-first conversational AI that:

- greets the user when a connection becomes active;
- stays available for technical discussion, coding questions, design reviews, debugging, and planning;
- asks one useful clarification question when the request is underspecified;
- gives an ADHD user one actionable task at a time;
- confirms whether the task was completed instead of inferring completion;
- uses a visible/spoken timer for short work units;
- supports a separate long-form silent co-presence session based on the reference `42–6–42` study template;
- remains interruptible, low-latency, grounded, and honest about uncertainty.

The Orb is not a generic chatbot and not a timer with an LLM attached. It is a deterministic focus-control plane with a conversational interaction model around it.

## 2. Current repository reality

The codebase already has useful seams:

| Concern | Current seam | Current limitation |
|---|---|---|
| Session/task state | `apps/mobile/src/runtime/T0FocusSession.ts`, `apps/mobile/src/session/StateMachine.ts` | No dedicated long-form study-session state machine |
| Turn completion | `apps/mobile/src/runtime/VoiceLoopController.ts`, `SemanticEndpointer.ts` | Current pipeline still uses endpointing as the main turn boundary |
| Backchannel | `apps/mobile/src/voice/Backchannel.ts` | Deterministic and bounded; needs mode/wait/study awareness |
| Barge-in | `apps/mobile/src/voice/BargeIn.ts` | Yields control, but playback generation cancellation must be end-to-end |
| Transport | `apps/mobile/src/voice/RelayClient.ts` and relay implementations | Needs micro-turn events, reconnect checkpoints, and background-result events |
| Task context | `apps/mobile/src/lld/ContextPack.ts` | Stores profile, recent tasks, open loops, and session only |
| Task retrieval | `ContextPack.retrieve()` | Scans recent tasks with lexical/cosine matching; no code graph or evidence provenance |
| Durable task history | `backend/relay-py/src/orb_relay/store/context_store.py` | Recent-task SQLite store; good tenant/user boundary, not a technical knowledge store |
| LLM boundary | `backend/relay-py/src/orb_relay/proxy/gateway_client.py`, gateway sidecar | Gateway is the correct single model door; current completion seam is blocking |
| Cost control | gateway/session cost contracts | Must remain in-path for every new interaction/background call |

The implementation must extend these seams. Do not create a second task FSM, timer, playback path, or provider SDK path.

## 3. Architecture change: two model planes

The new interaction-model research changes the architecture from “voice pipeline calls an LLM” to two cooperating planes:

```text
audio / text / explicitly-authorized visual context
  → 200 ms input micro-turn transport
  → InteractionModelAdapter
      ├─ immediate presence, backchannel, interruption suggestion, silence
      └─ rich ContextPack → BackgroundReasoningAdapter
                              ├─ coding/debugging reasoning
                              ├─ Graph-augmented code retrieval
                              ├─ search and tool calls
                              └─ longer-horizon planning
  → deterministic TurnPolicy + StateMachine + StepGate + FocusTimeBox
  → orb audio, timer ring, and optional transcript/artifact projection
```

### 3.1 Important latency correction

The Thinking Machines reference uses **200 ms time-aligned micro-turns**. That is an interaction cadence, not proof that a full spoken response reaches the user in under 200 ms. Their published research preview reports 0.40 s turn-taking latency on its benchmark.

Focus Orb must report these separately:

1. micro-turn processing time;
2. microphone capture to first audible output;
3. turn-taking latency;
4. time to first useful spoken content;
5. full response completion;
6. cancellation time after user speech.

The product claim is not `<200 ms end-to-end` until real Android/iOS device traces prove it.

### 3.2 New contracts

Add provider-neutral contracts behind the registry/gateway boundaries:

```ts
interface InteractionModelAdapter {
  open(input: InteractionSessionInput): Promise<InteractionStream>;
}

interface InteractionStream {
  pushInput(chunk: InputMicroTurn): void; // target cadence: 200 ms
  onEvent(listener: (event: InteractionEvent) => void): Unsubscribe;
  interrupt(reason: 'user_speech' | 'user_cancel' | 'safety'): void;
  attachBackgroundResult(result: BackgroundResult): void;
  checkpoint(): Promise<InteractionCheckpoint>;
  close(): Promise<void>;
}

interface BackgroundReasoningAdapter {
  run(request: BackgroundReasoningRequest): AsyncIterable<BackgroundResult>;
}
```

Model events are suggestions and language. They cannot advance the task FSM, mark a step complete, start/stop an authoritative timer, or execute an unsafe tool call.

### 3.3 Two-speed conversation contract

High-stakes work must not make the user wait in silence while Opus thinks. Split the experience into two concurrent lanes:

```text
user speech / intent
        │
        ├── Gemini realtime lane
        │     ├─ immediate acknowledgement or useful context
        │     ├─ one clarification question when needed
        │     └─ verified unfinished-work suggestions
        │
        └── Opus deep-work lane
              ├─ selected skills
              ├─ Graph RAG and repository evidence
              ├─ tools, calculations, and structured planning
              └─ detailed response package
                              │
                              ▼
                    safe result integration boundary
```

#### High-stakes trigger

Route to the Opus lane when the intent classifier detects requests such as:

- L8 engineering/system design;
- architecture or migration decisions;
- startup strategy, product strategy, or business model work;
- security, reliability, cost, or compliance decisions;
- multi-file code investigation;
- explicit “deep dive”, “research this”, or “work on this in detail”.

The trigger is deterministic and auditable. The user can override it with “keep this quick” or “think deeply about this”.

#### Gemini realtime context policy

Gemini may keep the conversation moving using a **verified context candidate pack**, selected in this order:

1. the current user request;
2. the current active task and step;
3. an explicitly open work item in the current session;
4. an unfinished task not marked `done`;
5. a previous conversation topic with a durable checkpoint;
6. a system-detected likely current activity, clearly labelled as an inference.

Every item must carry:

```ts
type ContextConfidence = 'verified' | 'inferred' | 'needs_confirmation';

interface ContextCandidate {
  source: 'current_turn' | 'active_task' | 'open_loop' | 'previous_session' | 'system_signal';
  text: string;
  confidence: ContextConfidence;
  evidence_id: string;
  observed_at_ms: number;
}
```

Gemini can say: “We left the relay-latency task unfinished yesterday; do you want to continue it?” It must not say: “You are working on the relay latency task now” unless the user confirms it.

#### Opus result package

Opus returns a structured package rather than raw prose:

```ts
interface DeepWorkResult {
  request_id: string;
  status: 'working' | 'needs_user_input' | 'ready' | 'failed';
  summary_for_voice: string;
  detailed_artifact: string;
  decisions: readonly string[];
  open_questions: readonly string[];
  evidence_ids: readonly string[];
  skills_used: readonly string[];
  confidence: 'high' | 'medium' | 'low';
}
```

Gemini may acknowledge `working`, ask one bounded question from `open_questions`, or present `summary_for_voice` after the result is validated. The detailed artifact remains available through the technical transcript/artifact channel and is not read aloud in full.

#### Authority and failure rules

- Gemini never fabricates previous conversation, task state, or Opus progress.
- Opus never directly mutates the task FSM or marks a task complete.
- If context is inferred, Gemini must ask for confirmation before acting on it.
- If Opus fails, Gemini says the deep work is unavailable and offers a bounded fallback; it does not improvise a high-stakes answer as if the research completed.
- If the user changes topic, cancel or suspend the Opus request and preserve the partial checkpoint.
- GPT-5 is an optional critic for low-confidence or high-risk Opus results, not a default third call.

#### Example interaction

```text
User: Design the L8 architecture for the Focus Orb.

Gemini: “I’ll work through the detailed architecture. While I do that, we still have the
         unfinished retrieval-grounding issue from our last session. Do you want to keep
         that in scope?”

Opus:   runs the L8 skill, code-graph retrieval, cost/latency analysis, and evaluation plan

Gemini: “I have the architecture draft ready. The main decision is a two-speed interaction
         lane with deterministic task state. I’ll give you the detailed artifact now.”
```

## 4. Interaction behavior changes

### 4.1 Add `TurnPolicy`

Create a pure deterministic `TurnPolicy` between endpointing signals and response creation. It must account for:

- `present`, `converse`, `focus_5m`, `study_with_me_42_6_42`, `check_in`, `quiet_presence`, and `safety` modes;
- eager, normal, and patient turn behavior;
- explicit “wait”, “stay quiet”, and “I’m thinking out loud” instructions;
- user speech priority over Orb playback;
- technical thinking pauses;
- suppression of proactive speech during quiet focus;
- timer check-ins at safe boundaries;
- protected safety messages.

The policy owns whether to keep listening, create a response, play a cached backchannel, defer speech, cancel output, or enter a waiting state. Provider VAD/endpointer signals are evidence, not authority.

### 4.2 Make interaction events explicit

The mobile/relay protocol needs typed events for:

```text
activity_start
activity_end
micro_turn_received
response_started
audio_chunk
backchannel
stay_silent
background_request
background_result
response_cancelled
barge_in
turn_complete
session_checkpointed
connection_draining
connection_resumed
```

Use `generation_id` on every output. Barge-in must cancel the active generation and clear queued audio immediately. No stale audio may arrive after cancellation.

### 4.3 Preserve conversation identity across reconnects

Use separate `session_id` and `connection_id` values. Persist a product-owned checkpoint containing:

- current mode and wait mode;
- active task and current step;
- timer phase and monotonic elapsed time;
- last acknowledged turn;
- background requests/results in flight;
- provider resumption token, if any;
- context-compaction summary.

Reconnect must not duplicate the greeting, restart a timer, repeat a response, or lose the current step.

## 5. Add the study-with-me mode

Add an explicit opt-in mode: `study_with_me_42_6_42`.

```text
SETUP          30–60 s: greet, confirm task, choose timebox, explicit start
FOCUS_1        42 min: silent co-presence, focus bed, timer ring
BREAK          6 min: guided reset, lower-intensity bed, optional auto-resume
FOCUS_2        42 min: same task, silent co-presence
WRAP           30–90 s: ask done/partial/blocked, checkpoint, next choice
```

Behavioral rules:

- setup may ask one blocking clarification;
- focus blocks are silent by default;
- the user may say “pause”, “I’m stuck”, “stop”, or “check in every five minutes”;
- five-minute check-ins are opt-in for this mode;
- elapsed time never proves task completion;
- break and wrap transitions are deterministic;
- the Orb uses the existing audio bed, ducking, barge-in, timer ring, and checkpoint seams;
- do not introduce text/buttons/dashboard requirements for the core hands-free flow.

Keep this separate from `focus_5m`, `technical_pairing`, and `quiet_presence`.

## 6. Model-selection changes

Do not choose one LLM for the whole product. Route by responsibility:

| Responsibility | Model/control path |
|---|---|
| `done`, `next`, `pause`, timers, safety short-circuits | No LLM; deterministic local path |
| Live presence, silence, backchannel, interruption, time-aware interaction | Gemini/native interaction model; current realtime cascade as fallback |
| Short clarification, task atomization, brief support | Small/fast model, schema-locked |
| Coding, debugging, architecture, code review | Sonnet background lane by default |
| L8 design, startup strategy, major architecture, high-stakes analysis | Opus with selected skills, Graph RAG, and tools |
| Low-confidence/high-risk result review | GPT-5 critic, only when policy triggers it |
| Evaluation adjudication | Strong offline model, never user-turn hot path |

The first provider bakeoff should compare exact pinned Anthropic small/mid and Gemini small/mid candidates through the existing gateway, with identical request snapshots. A native interaction-model candidate is a separate axis and must not be judged only by text quality.

Required model-selection dimensions:

- technical correctness;
- task-follow-through correctness;
- context/evidence grounding;
- clarification quality;
- ADHD cognitive load;
- naturalness and warmth;
- interruptibility;
- first-audio and turn-taking latency;
- variance across repeated runs;
- cost per user/month.

The production router should optimize for **quality per user-minute**, not model prestige. A high-stakes request may spend more tokens because it prevents a wrong decision; routine presence and unfinished-task suggestions must remain cheap and fast.

Hard-fail any candidate that invents repository facts, marks completion without evidence, leaks context, violates schemas, speaks after cancellation, loses state, or gives technically wrong high-risk guidance.

## 7. Retrieval and Graph RAG changes

Graph RAG is a selective technical-reasoning capability, not the conversational brain.

### 7.1 Use cases

Graph retrieval is valuable for:

- tracing callers/callees;
- following mobile → relay → gateway routes;
- locating state mutations and their tests;
- explaining an unfamiliar module through its dependency neighborhood;
- grounding answers in symbols, files, line ranges, and evidence edges.

It is not needed for greetings, timers, one-step task planning, or ordinary conversation.

### 7.2 Implementation

Build a deterministic code graph from compiler/LSP/indexer facts. Do not use an LLM to invent symbol relationships.

```text
technical query
  → route classifier
  → lexical/vector symbol seeds (k ≤ 8)
  → bounded graph expansion (h ≤ 2)
  → source snippets + line/edge provenance
  → background reasoning model
  → concise spoken summary + exact optional artifact
```

Rules:

- index on commit/workspace changes, never during the voice turn;
- keep every query tenant/workspace scoped;
- cache stable symbol neighborhoods and invalidate changed files/edges;
- include `graph_used`, `seed_nodes`, `expanded_edges`, `retrieval_ms`, and `evidence_ids`;
- keep typical added context ≤2,000 tokens;
- target warm-cache retrieval p95 ≤100 ms;
- do not delay the first interaction acknowledgement for graph retrieval;
- fall back to vector/lexical retrieval when graph evidence is absent or stale.

### 7.3 Required ablation

Run `vector_only`, `hybrid`, and `graph_only` on at least 100 technical cases. Ship only if hybrid achieves:

- ≥10 percentage-point evidence-recall improvement on two-hop cases;
- no more than 2-point regression on direct lookup;
- ≥95% source-line traceability;
- zero tenant/workspace bleed;
- no first-audio regression;
- expert technical score improvement ≥0.2/4.

## 8. Golden dataset and evaluation changes

Create a versioned dataset with both single-turn cases and multi-turn audio sequences.

### 8.1 Dataset shape

Initial baseline:

- 400 single-turn golden cases;
- 80 multi-turn sequences;
- 5 repeated runs per probabilistic candidate;
- held-out regression set from production failures after redaction;
- voice-to-voice replay set with real pauses, interruptions, background noise, and ASR errors.

Each case must contain:

```json
{
  "case_id": "technical.trace.001",
  "mode": "technical_pairing",
  "input": "...",
  "context": { "task": "...", "step": "...", "evidence": ["..."] },
  "expected_route": "technical_mid",
  "expected_state_transition": "none",
  "gold_facts": ["..."],
  "forbidden_claims": ["..."],
  "acceptable_answers": ["..."],
  "timing_window_ms": { "earliest": 250, "latest": 1200 },
  "hard_gates": ["no_completion_without_evidence"]
}
```

### 8.2 Scenario families

Cover:

1. connection greeting and reconnect;
2. pure conversation and social repair;
3. ambiguous task intake and one-question clarification;
4. atomization into one startable step;
5. done/not-done/partial/blocked confirmation;
6. five-minute timer and safe check-in;
7. silent focus and study-with-me 42–6–42;
8. user thinking pauses and explicit wait mode;
9. barge-in during filler, first audio, long answer, and safety speech;
10. overlapping speech and stale audio cancellation;
11. technical correctness and code-grounded evidence;
12. Graph RAG one-hop/two-hop/missing-evidence cases;
13. background reasoning result arriving during user speech;
14. reconnect, context compression, and duplicate-response prevention;
15. cost exhaustion, provider failure, and local fallback.

### 8.3 Metrics and gates

Hard metrics:

- response while `wait_mode=true`: 0 except safety;
- unsolicited speech during quiet focus: 0;
- stale audio after barge-in: 0 chunks;
- duplicate response after reconnect: 0;
- timer interruption during user speech: 0;
- task completion without explicit evidence: 0;
- tenant/context bleed: 0;
- invalid structured output: 0 on control paths;
- technical high-risk correctness: 100% on the protected set.

Soft metrics:

- technical expert correctness ≥3.7/4;
- clarification usefulness;
- ADHD cognitive load;
- naturalness of turn handoff;
- perceived co-presence;
- silence appropriateness;
- background-result integration quality;
- first-audio and turn-taking latency;
- cost efficiency.

Use calibrated judges only as a supplement. Anchor model judges against expert labels, report confidence intervals, and use paired cases for model comparisons.

## 9. Known correctness and production gaps to fix first

The prior runtime audits identified these before adding more sophistication:

1. `CLARIFY` must not mutate the task or advance the plan before the user answers.
2. Atomizer output must be grounded in actual task/context evidence; unsupported provenance is forbidden.
3. Placeholder or constant embeddings must not influence reuse/seed decisions.
4. Retrieval must expose provenance and distinguish reuse, seed, and cold paths honestly.
5. TTS/audio latency must be measured capture-to-speaker; provider latency alone is insufficient.
6. Native fallback audio must exist when a provider or local voice engine is unavailable.
7. Streaming/cancellation must prevent duplicate or stale spoken responses.
8. The deterministic task FSM remains the source of truth even when the interaction/background model is highly capable.

Do these before claiming that Graph RAG or a newer model improved quality.

## 10. Delivery order

### P0 — correctness and observability

1. Fix clarify mutation and provenance/retrieval truthfulness.
2. Replace placeholder embedding behavior with a tested adapter or disable semantic reuse until valid.
3. Add end-to-end latency spans and generation IDs.
4. Add hard no-speech-during-quiet and no-stale-audio gates.

### P1 — interaction foundation

1. Add `TurnPolicy` and explicit interaction events.
2. Add `InteractionModelAdapter` with current cascade fallback.
3. Add session/connection checkpoints and reconnect reconciliation.
4. Add asynchronous background reasoning handoff.

### P2 — Focus Orb product modes

1. Add `study_with_me_42_6_42` state machine.
2. Reuse audio bed, timer ring, ducking, barge-in, and checkpoint infrastructure.
3. Add setup/break/wrap spoken contracts.
4. Add simulator and human comparison against `focus_5m`.

### P3 — technical intelligence

1. Add deterministic code-graph indexing and bounded hybrid retrieval.
2. Add evidence-pack provenance and graph retrieval telemetry.
3. Run vector/hybrid/graph ablation.
4. Route only proven technical cases through Graph RAG.

### P4 — model and quality optimization

1. Run the gateway-routed provider bakeoff.
2. Add 400-case golden set and 80 sequence corpus.
3. Run voice-to-voice device evaluation.
4. Canary model/prompt/routing changes behind eval and cost gates.

## 11. Non-functional requirements

- **Latency:** target 200 ms interaction ticks; measure first audible output and turn-taking separately; no graph/tool call may block presence.
- **Reliability:** reconnect without duplicate greeting, timer reset, state loss, or stale audio.
- **Correctness:** deterministic state transitions; evidence-backed technical answers; explicit uncertainty.
- **Privacy:** tenant/user/session isolation in context, graph, logs, and eval fixtures; redact audio/transcripts before dataset admission.
- **Cost:** every model/background/retrieval operation is metered through the gateway and cost-control plane.
- **Operability:** structured traces for route, model, retrieval, evidence, timing, audio generation, cancellation, and state transition.
- **Accessibility:** orb, voice, and audio must be sufficient; transcript/artifact channel is optional support, not required for core use.
- **Change safety:** no model, prompt, routing, graph, or interaction-policy change ships without the relevant eval suite and rollback path.
- **Scale:** T0 memory adapters remain runnable on ₹0 infrastructure; T2 adapters must support durable stores, indexed graph retrieval, provider failover, and tenant isolation.

## 12. Definition of done

The change set is complete only when:

- the new contracts are implemented behind the registry/gateway boundaries;
- the current cascade remains a working fallback;
- the Orb greets, converses, clarifies, focuses, pauses, resumes, and wraps correctly;
- `focus_5m` and `study_with_me_42_6_42` are separate tested modes;
- technical answers can cite source evidence when Graph RAG is used;
- all hard eval gates are green;
- real Android and iOS audio traces exist for the latency claims;
- cost, provenance, and tenant isolation are observable;
- the repository verify gate passes with the actual output recorded.

## Related documents

- [`L8-IMPLEMENTATION-DETAIL.md`](L8-IMPLEMENTATION-DETAIL.md)
- [`INTERACTION-MODELS-RESEARCH.md`](INTERACTION-MODELS-RESEARCH.md)
- [`LLM-SELECTION-AND-EVALS.md`](LLM-SELECTION-AND-EVALS.md)
- [`STUDY-WITH-ME-REFERENCE-TEMPLATE.md`](STUDY-WITH-ME-REFERENCE-TEMPLATE.md)
- [`REVIEW-2026-08-06-END-TO-END-FLOW.md`](REVIEW-2026-08-06-END-TO-END-FLOW.md)
