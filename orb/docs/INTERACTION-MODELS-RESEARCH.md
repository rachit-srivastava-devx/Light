# Focus Orb — Interaction Models Research

Status: design input
Date: 2026-08-08
Question: how do strong voice products make conversation feel natural, and what should Focus Orb copy?

This is a mechanism review, not a vendor endorsement. The useful unit is the interaction primitive: turn-taking, interruption, waiting, context, progress, and recovery.

## 1. The new primary reference: Thinking Machines interaction models

The previous version of this memo was one generation behind. The most relevant new reference is Thinking Machines Lab's May 2026 research preview, `TML-Interaction-Small`, from the lab founded by Mira Murati. It treats interaction as a model capability rather than as a conventional LLM wrapped in VAD, STT, turn detection, and TTS glue. [Thinking Machines Lab: Interaction Models](https://thinkingmachines.ai/blog/interaction-models/)

### 1.1 Correct latency interpretation

The headline is **200 ms time-aligned micro-turns**, not a proven 200 ms end-to-end spoken reply:

| Measurement | Meaning | Focus Orb requirement |
|---|---|---|
| 200 ms | Input/output micro-turn cadence in the model stream | Use as the interaction clock and transport chunk target |
| 0.40 s | Vendor-reported FD-bench V1 turn-taking latency for TML-Interaction-Small | Treat as a research reference, not our SLA |
| First audible response | Capture → inference → decoder → network → device playback | Measure on real target devices; this is the user-visible SLA |
| Full answer completion | Time to useful, correct, complete response | Keep separate from presence and turn-taking latency |

This distinction matters. Focus Orb can target sub-200 ms **interaction ticks** while the first audible acknowledgement may be slower. We must not claim `<200 ms response latency` until p50/p95/p99 are measured from microphone capture to speaker output on Android and iOS.

### 1.2 What is materially different

Thinking Machines describes a two-model system:

```text
continuous audio/video/text micro-turns
              │
              ▼
  real-time interaction model
  - remains present continuously
  - listens while speaking
  - chooses response, backchannel, interruption, or silence
  - keeps the immediate conversational thread
              │ rich context / events
              ▼
  asynchronous background model
  - deeper reasoning
  - coding analysis
  - search and tool calls
  - longer-horizon work
              │ streamed results
              └──────────────► interaction model integrates them when appropriate
```

The important ideas are:

1. **Interaction is native.** Silence, overlap, interruption, and timing remain in the model's context instead of being discarded by a completed-turn pipeline.
2. **The stream is time-aligned.** Audio, video, and text arrive as repeated small slices; the model can react before a traditional endpoint detector decides that the user is finished.
3. **Speaking is a decision.** The model can speak, backchannel, interrupt, or remain silent. Silence is an output, not merely a missing response.
4. **Presence is decoupled from deep work.** The real-time model stays with the user while the background model searches, reasons, or uses tools.
5. **Context is a package, not a prompt fragment.** The background model receives the live thread, timing, state, prior results, and open questions, then streams findings back into the interaction.
6. **Streaming state is persistent.** Their design appends micro-turns to a persistent session rather than repeatedly starting a fresh request and paying a full prefill cost.

The published model is a 276B-parameter MoE with 12B active parameters and is still a research preview. That makes it an architectural reference first, not an automatic Phase 1 deployment choice.

### 1.3 What Focus Orb should steal now

Steal the **contracts and evaluation shape**, even before we have access to the weights:

```ts
interface InteractionStream {
  pushInput(chunk: InputMicroTurn): void; // target cadence: 200 ms
  onEvent(listener: (event: InteractionEvent) => void): Unsubscribe;
  interrupt(reason: 'user_speech' | 'user_cancel' | 'safety'): void;
  attachBackgroundResult(result: BackgroundResult): void;
  checkpoint(): Promise<InteractionCheckpoint>;
}

type InteractionEvent =
  | { kind: 'audio_chunk'; generation_id: string; bytes: Uint8Array }
  | { kind: 'backchannel'; phrase_id: string }
  | { kind: 'stay_silent'; reason: string }
  | { kind: 'interrupt_user'; confidence: number }
  | { kind: 'background_request'; request_id: string; purpose: string }
  | { kind: 'time_signal'; elapsed_ms: number }
  | { kind: 'response_complete'; response_id: string };
```

The event stream is an adapter boundary. It does **not** give the model authority to advance `T0FocusSession`, mark a step complete, start a timer, or execute a tool. Those remain deterministic product decisions. `interrupt_user` is a suggestion; the product policy may reject it in a safety or accessibility boundary.

### 1.4 Focus Orb's target architecture after this research

```text
mic / optional explicitly-authorized screen context
  → 200 ms input micro-turn transport
  → InteractionModelAdapter
      ├─ immediate audio / backchannel / silence / interruption suggestion
      └─ rich ContextPack → BackgroundReasoningAdapter
                              ├─ code reasoning
                              ├─ retrieval/search
                              └─ tool calls
  → deterministic TurnPolicy + StateMachine + StepGate + FocusTimeBox
  → orb audio + animation + optional transcript/artifact projection
```

The existing `VoiceLoopController` and `RelayClient` should evolve into transport, backpressure, cancellation, checkpoint, and fallback seams. They should not remain the only turn oracle if a native interaction model becomes available.

### 1.5 ADHD-specific adaptations

- **Presence model:** can say a short “I’m with you” or remain silent while the user thinks; it must not turn every pause into a question.
- **Clarification:** ask one bounded question when the current task cannot be safely atomized; keep listening while the user answers or corrects themselves.
- **Focus mode:** suppress generic proactivity; allow only the current step, a due timer check-in, a blocked-state explanation, or an explicit user request.
- **Time awareness:** pass monotonic elapsed time and active-step state into the interaction context. The deterministic timer remains authoritative; the model supplies natural phrasing and timing suggestions.
- **Technical work:** the interaction model handles “stay with me” conversationally while the background model reads code, searches, or prepares a plan. Never make the user wait silently for a tool call.
- **Barge-in:** the user's speech wins. Cancel queued audio immediately, preserve the partial thought, and let the interaction model re-anchor before answering.

### 1.6 What we cannot steal directly

- The native full-duplex weights, early multimodal fusion, and co-trained audio decoder require access to the model or a comparable training program.
- The vendor's benchmark numbers are not acceptance evidence for our network, device, codec, or TTS path.
- A 276B MoE serving footprint is inconsistent with the current T0/Phase 1 cost envelope unless a hosted endpoint or distilled model is available.
- Model-generated timing and task state cannot replace the deterministic safety and ADHD control plane.

### 1.7 Decision

Do not replace the current cascade blindly. Add a provider-neutral `InteractionModelAdapter` and benchmark any TML preview access against a simulated micro-turn adapter backed by the current relay. Keep the existing cascade as the fallback path. The go/no-go decision must use measured device traces for:

- micro-turn processing time;
- first audible output;
- turn-taking latency;
- interruption cancellation time;
- background-result insertion time;
- task-state correctness and technical answer quality.

Until those are measured, the honest product claim is **200 ms interaction cadence target**, not **200 ms response guarantee**.

## 2. The main conclusion

The winning voice experience is not “one very smart model talking continuously.” It is a coordinated interaction system:

```text
audio input
  → activity detection
  → turn policy
  → early response / backchannel
  → streamed generation
  → streamed speech
  → interrupt + discard if the user speaks
  → durable transcript/context checkpoint
```

Focus Orb already has most of the pieces in `VoiceLoopController`, `RelayClient`, `SemanticEndpointer`, `Backchannel`, and `BargeIn`. The missing layer is a first-class `TurnPolicy` that decides how patient, eager, interruptible, and proactive the orb should be for this moment.

## 3. Previous-generation patterns still worth retaining

### 3.1 ChatGPT Voice: voice is a surface over a persistent chat

OpenAI’s current Voice experience keeps voice inside a chat: the user can hear the answer while following streamed text, type when speaking is inconvenient, and review the conversation without starting over. It supports simultaneous listening/speaking, a user instruction to wait while they think, background conversations, and progress updates in Work/Codex contexts. [ChatGPT Voice help](https://help.openai.com/en/articles/20001274)

**Steal:**

- Keep one conversation identity across connection reconnects.
- Make voice the fast input/output surface, not the only memory surface.
- Provide an explicit “wait until I ask” mode for thinking out loud.
- Support typed/context input when speech is a poor channel.
- Treat `started`, `blocked`, `waiting`, `redirected`, and `completed` as first-class progress events.
- Keep listening and speaking concurrent, with hard user priority.

**Do not copy blindly:**

- Do not assume a full chat transcript is compatible with the orb-only UI. Store it for continuity and expose it only when useful or requested.
- Do not enable background listening without explicit consent, clear stop controls, and a visible/aural listening state.
- Do not let a provider’s hidden conversation state become the source of truth for task progress.

### 3.2 OpenAI Realtime: turn detection has an explicit control surface

The Realtime API separates server VAD from semantic VAD. Server VAD uses audio thresholds and silence duration; semantic VAD estimates whether the user has finished speaking and exposes eagerness levels. The API also separates “create a response” from “interrupt the current response,” which allows a client to listen without automatically speaking. [Realtime API reference](https://platform.openai.com/docs/api-reference/realtime)

**Steal:**

- Model turn-taking as configuration, not one fixed timeout.
- Expose `eagerness: eager | normal | patient` internally.
- Separate `activity_start`, `activity_end`, `create_response`, `cancel_response`, and `turn_complete` events.
- Allow the app to receive a transcript without immediately speaking.
- Make interruption a protocol event, not a UI side effect.

**Focus Orb adaptation:**

```text
social_presence      → normal/eager
technical_brain_dump → patient
clarification        → patient
timer_check_in       → normal, deferred while user is speaking
crisis/safety        → non-interruptible until the safe message completes
```

The provider may provide VAD evidence, but the deterministic Focus Orb `TurnPolicy` still owns whether a user turn is complete and whether the orb may speak.

### 3.3 Gemini Live: continuous bidi streaming plus explicit interruption events

Gemini Live models use a persistent bidirectional session with realtime audio/text input and streamed server messages. The API defaults to start-of-activity interruption, reports an `interrupted` event, and expects the client to immediately discard queued playback. Its guidance recommends small audio chunks, context compression, session resumption, and graceful handling of `GoAway` before connection termination. [Live API reference](https://ai.google.dev/api/live) · [Live best practices](https://ai.google.dev/gemini-api/docs/live-api/best-practices)

**Steal:**

- 20–40ms mic chunks at the transport boundary; avoid buffering a second of audio.
- An explicit `interrupted` event that clears the playback queue immediately.
- Session resumption tokens stored independently from the WebSocket.
- A `go_away`/draining state so reconnect is planned before the provider drops the connection.
- Context compression/checkpointing for long-running sessions.
- Separate input transcription from output transcription for debugging and review.

**Do not copy blindly:**

- Do not allow a provider session to decide what the user’s current task or step is.
- Do not treat a resumption token as durable product memory; persist a product-owned checkpoint too.
- Do not use native audio-to-audio as a reason to remove deterministic routing and safety boundaries.

### 3.4 ElevenLabs Conversational AI: soft timeouts and turn eagerness

ElevenLabs exposes configurable silence timeouts, interruption handling, soft-timeout filler, and turn eagerness. Their soft-timeout pattern speaks a short filler only if generation is late, keeps waiting for the real answer, and retains a static fallback when dynamic filler generation fails. Their guidance also distinguishes eager, normal, and patient turn behavior. [Conversation flow documentation](https://elevenlabs.io/docs/eleven-agents/customization/conversation-flow)

**Steal:**

- A soft timeout is a separate state from an answer.
- Filler is one-shot, short, interruptible, and never promises a duration.
- Static local filler is the fallback; never call another LLM to rescue a delayed LLM on the hot path.
- Turn eagerness is adjusted by interaction context.
- The agent can start speaking after enough validated content rather than waiting for a complete paragraph.

**Focus Orb rule:**

```text
start_model_turn
  → wait 450ms for first useful audio
  → if late, play one cached “I’m with you” / “Let me look at that” clip
  → continue generation
  → cancel filler and response immediately on barge-in
```

Do not use “one second”, “almost done”, or other time promises in filler. Provider latency is variable, and a false promise makes the wait feel longer.

### 3.5 Realtime prompting: short rules, examples, and explicit noisy-audio behavior

OpenAI’s Realtime prompting guide recommends short bullet rules over long paragraphs, explicit behavior for unclear/partial audio, language pinning when mirroring fails, varied example phrases, and anti-repetition instructions. [Realtime prompting guide](https://cdn.openai.com/API/docs/realtime-prompting-guide.pdf)

**Steal:**

- Keep voice prompts modular: `identity`, `turn-taking`, `unclear-audio`, `technical-mode`, `focus-mode`, `safety`.
- Write “if/then” behavior in bullets, not prose-heavy persona prompts.
- Give 3–5 varied examples for greeting, repair, clarification, stuck, and completion.
- Tell the model what to do with unintelligible audio; never let it guess.
- Pin the language when the user or product requires it.
- Put response-length and question-count limits in the contract and prompt.

### 3.6 OpenAI GPT-Live: audio-native evals and safety while speaking

OpenAI describes audio-native safety evaluations using generated audio, red-teaming for voice-specific risks, and safeguards that can steer, add safety messaging, or end a voice conversation while the interaction is happening. [GPT-Live announcement](https://openai.com/index/introducing-gpt-live/)

**Steal:**

- Evaluate audio-native failure modes, not only text prompts.
- Test overlapping speech, background speakers, emotional reliance, unsafe content, and long pauses.
- Allow safety controls to act during streaming, not only after a full response.
- Keep high-risk safety messages in a non-interruptible path, while ordinary conversation remains interruptible.

## 4. The interaction model Focus Orb should implement

### 4.1 `TurnPolicy` is the missing abstraction

Add a pure deterministic policy:

```ts
interface TurnPolicyInput {
  readonly mode: 'present' | 'converse' | 'focus' | 'check_in' | 'safety';
  readonly user_activity: 'none' | 'speech' | 'barge_in' | 'backchannel';
  readonly speech_confidence: number;
  readonly semantic_end_probability: number;
  readonly user_requested_wait: boolean;
  readonly emotional_load: number;
  readonly response_kind: 'cached' | 'local' | 'model' | 'safety';
  readonly response_started_at_ms: number | null;
  readonly now_ms: number;
}

type TurnPolicyAction =
  | { kind: 'keep_listening' }
  | { kind: 'create_response' }
  | { kind: 'play_backchannel'; phrase_id: string }
  | { kind: 'cancel_response'; reason: 'barge_in' | 'pause' | 'new_turn' }
  | { kind: 'defer_proactive_message'; reason: string }
  | { kind: 'speak_safety_message' };
```

This policy is where Focus Orb differs from a generic voice assistant:

- technical thinking gets patience, not aggressive turn-taking;
- a user saying “wait” changes the endpointing behavior immediately;
- focus mode is quiet by default and only speaks on explicit task/progress events;
- the five-minute timer is allowed to become due but not allowed to interrupt active speech;
- safety speech is protected from casual barge-in;
- no silence signal ever means “done”.

### 4.2 Two-channel response

Copy ChatGPT’s useful separation of voice and text, adapted to orb-only presentation:

```text
spoken_summary  → immediate TTS, 1–3 short sentences
transcript      → optional on-screen/accessibility channel
structured_state → orb animation/timer/progress, deterministic
technical_artifact → optional file/plan/code view, never read aloud in full
```

The orb remains the default surface. A future transcript panel can show exact code, commands, and citations without forcing the user to hold long audio in working memory.

### 4.3 Connection and session identity

Use two identifiers:

- `session_id`: durable product conversation/task identity;
- `connection_id`: one transport lifetime.

On reconnect:

1. save the last acknowledged turn and task checkpoint;
2. receive/store a provider resumption token if available;
3. create a new `connection_id`;
4. re-establish audio and presence;
5. replay only a short connection repair phrase, not the whole greeting unless the user has explicitly restarted;
6. reconcile provider history with product-owned context;
7. resume listening without duplicating the last response.

Provider session resumption is a transport optimization. It is not the source of truth for task state, completion, timers, or tenant isolation.

### 4.4 Wait modes

Add explicit user-controlled wait modes:

| User says | Behavior |
|---|---|
| “Wait until I ask you to respond.” | Continue capturing/transcribing; suppress response creation. |
| “I’m thinking out loud.” | Use `patient` endpointing; backchannel at most once; do not answer partial thoughts. |
| “You can respond now.” | Process the accumulated turn. |
| “Stay quiet.” | Suppress proactive speech until explicit resume; keep visual presence. |
| “Talk me through it.” | Normal conversational mode; questions are allowed. |

Persist the wait mode in the product session, not only in the model prompt. Reconnects must preserve it.

### 4.5 Soft timeout and backchannel

Backchannels must be local, cached, and policy-selected:

```text
if response_kind == model
and no first_audio after 450ms
and user is not speaking
and no backchannel played for this turn
and mode != safety:
    play one cached backchannel
```

Do not generate the backchannel with a second model call. Do not play it during the first 250ms, during a user pause that may be a continuation, or after the user has requested quiet.

Backchannel quality metrics:

- false backchannel rate during user speech;
- backchannel-to-first-audio overlap rate;
- user barge-in within 500ms of backchannel;
- repeated phrase rate;
- perceived waiting score in human labels.

### 4.6 Progress as an event stream

Borrow the progress vocabulary used by voice-enabled coding workflows, but keep all state transitions deterministic:

```text
task.started
task.step_presented
task.timer_started
task.waiting_on_user
task.waiting_on_system
task.blocked
task.replanned
task.step_confirmed
task.completed
```

The orb may speak a progress update only when:

- the user asked for progress;
- a current step changed;
- a system wait/block requires user awareness;
- a timer check-in is due at a safe boundary;
- an explicit completion occurred.

No timer-driven “are you still there?” message is allowed during probable focus.

## 5. What not to steal

### Do not steal provider authority

Provider-native VAD, tool calling, memory, and session history are useful signals but not product truth. The product still owns `TurnPolicy`, `StateMachine`, `StepGate`, `FocusTimeBox`, and tenant/session context.

### Do not steal hidden long prompts

Long persona prompts are difficult to debug and easy to contradict. Use short mode-specific prompts, typed response schemas, and deterministic policy inputs.

### Do not steal generated filler

A delayed model should not trigger another model call to generate “thinking” speech. Cached phrases are faster, cheaper, and safer.

### Do not steal silent context truncation

Some realtime systems automatically truncate old turns. Focus Orb must checkpoint summaries and task state before context compression and emit an observable `context_compacted` event.

### Do not steal always-on proactivity

ChatGPT’s background voice is an opt-in feature. Focus Orb should be more conservative: presence can continue, but speech requires consent, a safe opening, and a cancellable event.

## 6. Implementation mapping to this repository

| Research pattern | Existing seam | Implementation change |
|---|---|---|
| Semantic/eager/patient turn-taking | `SemanticEndpointer.ts`, `VADGate.ts` | Add `TurnPolicy.ts`; make endpoint threshold mode-dependent. |
| Native interaction-model seam | `RelayClient.ts`, `VoiceLoopController.ts` | Add `InteractionModelAdapter`; accept 200 ms micro-turns and emit model suggestions without giving up deterministic product authority. |
| Real-time/background split | `ContextPack.ts`, `T0FocusSession.ts` | Add `BackgroundReasoningAdapter`; send a rich context package and stream results back without blocking presence. |
| Time-aware interaction | `FocusTimeBox.ts`, `PresenceEventQueue.ts` | Add monotonic elapsed-time signals; let the deterministic timer own due/completion while the interaction model owns natural timing language. |
| Immediate barge-in discard | `BargeIn.ts`, `VoiceLoopController.ts` | Add generation IDs and clear queued TTS on `barge_in`. |
| Soft timeout | `Backchannel.ts`, `PresenceEventQueue.ts` | Add one-shot `soft_timeout` event with static fallback. |
| Bidi event protocol | `RelayClient.ts`, `RelaySocket.ts` | Add explicit `activity_start/end`, `response_started`, `response_cancelled`, `turn_complete`. |
| Session resumption | `RelayClient.ts`, `ContextPack.ts` | Persist provider token plus product checkpoint keyed by `session_id`. |
| Voice plus transcript | `ResponseEnvelope.ts`, `AppModel.ts` | Add optional transcript/artifact projection; keep orb-only default. |
| Progress updates | `T0FocusSession.ts`, `StepGate.ts` | Emit typed progress events; never let model output advance the FSM. |
| Audio-native evaluation | `e2e-human-simulator/evaluator/` | Add turn-taking, overlap, wait-mode, cancel, and backchannel cases. |
| TimeSpeak/CueSpeak-style evaluation | `e2e-human-simulator/evaluator/` | Grade whether the orb speaks in the allowed timing window and whether the utterance is semantically correct; penalize premature speech and silence failures separately. |
| Prompt examples/variation | `domain/agents/` | Split prompts by mode and add 3–5 examples per interaction moment. |

## 7. Interaction-model evaluation additions

Add these scenario families to the LLM golden/e2e corpus:

1. **User still thinking:** 1–4 second pauses, filler words, self-corrections, and “don’t answer yet”.
2. **TimeSpeak:** the orb must speak at a specified relative time—such as a five-minute check-in—inside an allowed window and with the correct content.
3. **CueSpeak:** the orb must recognize a verbal cue such as “I’m stuck” or “I finished” and respond at the right moment without speaking over the user.
4. **Barge-in:** user interrupts during cached phrase, first TTS chunk, long answer, and safety message.
5. **Overlapping speech:** user and orb speak simultaneously; verify the user wins and queued audio is discarded.
6. **Background-result arrival:** deeper coding work completes while the user is speaking, pausing, or changing topics; verify the result is integrated at a safe conversational boundary.
7. **Connection reset:** provider sends `GoAway`; reconnect and resume without duplicate greeting or response.
8. **Quiet focus:** timer becomes due while the user is actively speaking or likely focused; verify safe deferral.

Hard metrics:

- user-priority interruption success: 100% for ordinary responses;
- stale audio after barge-in: 0 chunks;
- duplicate response after reconnect: 0;
- response while `wait_mode=true`: 0, except explicit safety path;
- timer-induced interruption during user speech: 0;
- unplanned proactive speech in quiet focus: 0;
- context/task loss after reconnect: 0 high-risk cases.
- premature or late TimeSpeak delivery outside the declared timing window: 0 hard-gate cases;
- background result inserted while the user is speaking: 0 unless explicitly requested.

Soft metrics:

- naturalness of turn handoff;
- whether the orb waits long enough for technical thinking;
- whether backchannels feel supportive rather than needy;
- perceived latency after a soft timeout;
- clarity of progress and blocked-state speech;
- user preference between `eager`, `normal`, and `patient` modes.

## 8. The first three slices to build

1. **Interaction adapter + event protocol:** add the 200 ms micro-turn contract, pure policy tests for patient/eager/wait/safety, and cancellation generation IDs.
2. **Background handoff + time-aware evals:** add rich context packaging, asynchronous result integration, and TimeSpeak/CueSpeak-style timing/semantic graders.
3. **Soft timeout + reconnect:** add cached backchannel, provider `GoAway`/resume handling, product checkpointing, and duplicate-response tests.

The point is not to reproduce another vendor’s UI. It is to steal their interaction primitives while keeping Focus Orb’s differentiator: quiet, explicit, one-step ADHD support with deterministic task truth.
