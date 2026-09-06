# Build digest — the only spec surface build agents should need to read

Extracted in full from `../../../blueprints/ADHD-Focus-Orb-L8-Deep-Dive/` (README + AGENTS.md +
docs 01–13) on 2026-08-04. Every interface, number, and rule below is quoted or paraphrased directly
from those source docs. **Read this file, not the 13-file blueprint**, unless a specific number's
derivation or a cross-examination answer is needed — then open the one numbered doc that owns it
(see the "owns" references inline).

If anything here conflicts with the live blueprint files, the blueprint wins — flag the drift in an
ADR (`docs/adr/`) rather than silently reconciling.

---

## 1. File / module layout

| File | Responsibility | Plane |
|---|---|---|
| `apps/mobile/src/presence/NoiseEngine.ts` | Procedural pink/brown noise generation: leaky-integrator brown (`y = clamp(y·0.998 + w·0.02)`), Paul Kellet 7-coefficient pink cascade; builds a 30s buffer with tail↔head crossfade (~250ms) at startup | PRESENCE |
| `apps/mobile/src/presence/AudioGraph.ts` | Declares the `react-native-audio-api` (Web Audio) graph: `AudioBufferSourceNode(noiseBuffer, loop=true) → GainNode → BiquadFilterNode(lowpass, sweepable) → destination`, plus a mixed-in voice/cue branch. All scheduling via `setTargetAtTime`/`linearRampToValueAtTime`/`start(when)` — JS never touches the render loop | PRESENCE |
| `apps/mobile/src/presence/DuckingMixer.ts` | Ramps bed gain −18→−30 dBFS over 80–150ms when voice plays; never stops/recreates the bed source node | PRESENCE |
| `apps/mobile/src/presence/InterruptionHandler.ts` | Classifies OS audio events transient vs user-intent; drives recovery (≤300ms + cover) or PAUSE (bed stop, mic release, socket close) | PRESENCE / SESSION |
| `apps/mobile/src/presence/ColorMorph.ts` | Sweeps lowpass cutoff to morph brown↔pink bed color as an ambient status channel (idle/thinking/step-ready) | PRESENCE |
| `apps/mobile/src/voice/VADGate.ts` | Mic capture + voice-activity detection; opens/closes STT socket on speech onset/~800ms-post-endpoint | VOICE I/O |
| `apps/mobile/src/voice/SemanticEndpointer.ts` | Classifier over streaming STT partials deciding "thought complete" vs "keep listening" (two-threshold: complete+150ms pause vs incomplete up to 30s hard cap) | VOICE I/O |
| `apps/mobile/src/voice/Backchannel.ts` | Plays cached 100–200ms "mm-hmm"/"yeah" clips locally on 250–600ms mid-utterance pauses (max 2/utterance, never consecutive/first 3s/high Emotional Load) | VOICE I/O |
| `apps/mobile/src/voice/BargeIn.ts` | AEC-based barge-in detection; yields ≤100ms, stops TTS, restores bed; 200ms min speech duration to distinguish barge-in from backchannel | VOICE I/O |
| `apps/mobile/src/voice/RelayClient.ts` | WebSocket client: mic Opus 16–24kbps out, STT partials in, TTS audio chunks in; talks only to our relay, never providers directly | VOICE I/O |
| `backend/relay-rs/src/provider.rs` | STT/TTS provider adapter boundary; T0 fake emits transcripts and binary TTS chunks | VOICE I/O (cloud) |
| `backend/relay-rs/src/session.rs` | Realtime socket session loop; tenant/session identity, pause contract, binary audio chunk emission | VOICE I/O (cloud) |
| `apps/mobile/src/session/StateMachine.ts` | The 9-state deterministic FSM (Register 0) — see §2 | SESSION CONTROL |
| `apps/mobile/src/session/StepGate.ts` | Holds the validated step list; speaks steps **by index**, never generates (INV2) | SESSION CONTROL |
| `apps/mobile/src/session/CheckInTimer.ts` | Invoked by policy emission (not a fixed timer); mechanical hook that transitions WORKING→CHECK_IN | SESSION CONTROL |
| `apps/mobile/src/session/CostReservation.ts` | Per-session token/char hard budget (e.g. ₹4); fails closed, never exceeds (INV5) | SESSION CONTROL / COST |
| `apps/mobile/src/router/Router.ts` | Pure function `route(state, intent, stuck_count) → action`, the lookup table in §04 | COGNITIVE |
| `apps/mobile/src/router/IntentClassifier.ts` | Rule-first keyword match on done/next/stuck/pause/question/chitchat; falls to cheap LLM only when ambiguous | COGNITIVE |
| `backend/relay-py/src/orb_relay/proxy/atomizer.py` | `task → schema-locked steps[]`, via `@pe/llm-gateway` through the gateway sidecar | COGNITIVE |
| `backend/relay-py/src/orb_relay/proxy/phrasers.py` | Capped/check-in phraser surface; model only on repeated stuck | COGNITIVE |
| `apps/mobile/src/cognitive/BeliefModel.ts` | 9 beliefs `(value, confidence, last_evidence_ts)`; log-odds update + confidence half-life decay | COGNITIVE / SESSION |
| `apps/mobile/src/cognitive/EvidenceTiers.ts` | 3-tier cascade: Tier0 rules/arithmetic, Tier1 similarity/exemplar-bank cosine, Tier2 small schema-locked LLM (only on ambiguity, off hot path) | COGNITIVE |
| `apps/mobile/src/cognitive/Policy.ts` | `decide(beliefs, session, history) → intervention`, pure function with veto order, guards, refractory, budget, hysteresis | COGNITIVE / SESSION |
| `apps/mobile/src/lld/ContextPack.ts` | In-memory RAG pack loaded at app-open (`recent_tasks[]`, `open_loops[]`, embeddings, `open_session`); lexical-then-cosine local match, no vector DB | CROSS/COGNITIVE |
| `apps/mobile/src/lld/ClarifyProtocol.ts` | Rule-driven slot filling (scope/first_context/blocker/time_box), max 2 questions | SESSION/COGNITIVE |
| `apps/mobile/src/lld/ProsodyDirector.ts` | State→(emotion, rate, energy) deterministic lookup table with a structurally-excluded shame-adjacent register set | SESSION/COGNITIVE |
| `apps/mobile/src/lld/ResponseEnvelope.ts` | Typed versioned envelope builder (`speech`, `orb`, `widgets`, `session`, `meta`) | CROSS-CUTTING (contract) |
| `apps/mobile/src/lld/VoiceRouter.ts` | `selectVoice(region, locale)` — deterministic geo routing, pinned per session | VOICE I/O |
| `backend/relay-py/src/orb_relay/cost/meter.py` | Meter→attribute→cap→alert cost-control loop with reservation before spend | CROSS-CUTTING (cost) |
| `backend/relay-py/src/orb_relay/eval/*` | Local fail-closed eval gates plus future voice-to-voice harness owner | CROSS-CUTTING (eval) |
| `backend/relay-py/src/orb_relay/observability/*` | Per-hop histograms, audio-gap watchdog, loudness-floor probe, sentinel canary | CROSS-CUTTING (observability) |
| `backend/relay-py/src/orb_relay/app.py` (`POST /v1/session/warmup`, `POST /v1/cache/prime`) | Serves the T0 context pack; primes prompt-cache prefix without provider claims | CROSS-CUTTING/backend |

## 2. Interfaces / contracts

**Session FSM (Register 0) — 9 states**: `IDLE_PRESENT · INTAKE · CLARIFY · STEP_PRESENT · WORKING · CHECK_IN · STEP_DONE · INTERRUPTED · SESSION_DONE`.

```
IDLE_PRESENT --tap--> INTAKE
INTAKE --transcript--> (stay; atomize dispatched, filler covers)
INTAKE --atomize_ready [steps schema-valid]--> STEP_PRESENT
STEP_PRESENT --spoken_complete [validated step exists, INV2]--> WORKING
WORKING --policy emits intervention [passes veto+refractory+budget]--> CHECK_IN
CHECK_IN --reply--> WORKING
WORKING --intent==done [explicit, INV3]--> STEP_DONE
STEP_DONE --[more steps?]--> STEP_PRESENT : SESSION_DONE
any --os_interrupt--> INTERRUPTED
INTERRUPTED --os_resume [bed recovered ≤300ms]--> prior state
SESSION_DONE --bed fade [last step done + confirm, INV4]--> IDLE_PRESENT
```

Invariants: INV1 (bed_active==true except IDLE/SESSION_DONE) · INV2 (steps spoken by index only,
reference-not-generate) · INV3 (completion only on explicit `done` intent) · INV4 (session ends only
on last-step-done + confirm) · INV5 (per-session cost reservation, cannot exceed) · INV6 (refractory
≥90s + budget ≤6 interventions/session) · INV7 (Dysregulated>0.8 ⇒ intervention set reduced to
`{Pause, Escalate}` and shame-adjacent prosody excluded).

**Atomizer schema** (validated before ANY use):
```ts
interface AtomizerOutput {
  steps: Array<{ step_text: string /* ≤120 chars */; est_min: number /* 1..15 */; done_signal: string }>;
  steps_total: number; // 1..12
}
```
Pipeline: structured decoding → validate (schema + semantic rules) → repair-once on failure → fail-closed
to deterministic fallback ("let's just start by opening it") → gate + use by reference. Error taxonomy:
`schema_invalid`, `step_not_atomic`, `step_count_explosion` (>12), `empty/duplicate`, `off-task`.

**Router** — pure function:
```
intent ∈ {done, next, pause}            → DETERMINISTIC: advance/pause; speak next step from cache
intent==stuck AND stuck_count<2         → DETERMINISTIC: templated re-anchor (cached audio)
intent==stuck AND stuck_count>=2        → CREW.reatomize(step)  [async, filler covers]
intent==question OR chitchat            → FAST_VOICE(short, capped)  [hot path, rare]
state==INTAKE                           → CREW.atomize(task)  [async, filler covers]
else                                     → FAST_VOICE(short, capped)
```
Intent labels: `{done, next, stuck, pause, question, chitchat}`.

**Belief vector** — 9 beliefs, each `(value, confidence, last_evidence_ts)`: `Goal · Context ·
Attention · Execution · WorkingMemory · Energy · EmotionalLoad · Trust · NoveltyPull`.
```
on evidence e=(belief, weight w, reliability r):
    logit(b) ← logit(b) + w·r
    conf     ← min(1, conf + κ·r)                # κ ≈ 0.4
on elapsed Δt with no evidence:
    conf  ← conf · 2^(−Δt/T½)                    # T½ per belief; Attention 90s, Energy 15min
    logit(b) ← logit(b) + (logit(prior)−logit(b))·(1 − 2^(−Δt/T_rev))
```
Bounded LLM evidence: `|w·r| ≤ 0.8 logits` per event.

**Registers**: R1 Goal (7, exclusive) · R2 Attention (7, exclusive) · R3 Barrier (8, multi-label) ·
R4 Intervention (9, ordered by intrusiveness): `Observe(0) · Presence(1) · Capture(2) · Celebrate(3) ·
Clarify(4) · Suggest(5) · Redirect(6) · Pause(7) · Escalate(8)`.

**Policy** — `decide(beliefs, session, history) → intervention`, in order:
```
1. HARD VETOES:
   a. Barrier.Dysregulated > 0.8       → Pause|Escalate ONLY
   b. Session ∈ {INTAKE, CLARIFY}      → yield to session mechanics
   c. refractory active                → Observe (unless severity=CRITICAL)
   d. intervention budget spent        → Observe (unless severity=CRITICAL)
   e. Attention.Focused>0.9 AND conf>0.6 AND max(Barrier)<0.7 → Observe   [DO-NO-HARM]
2. CANDIDATE GUARDS (all evaluated, least-intrusive-that-fired wins):
   Attention.Initiating>0.6                     → Suggest (smallest step)
   WorkingMemory<0.3                            → Suggest (re-state ONE action)
   Barrier.Overwhelmed>0.7                      → Suggest (shrink step)
   Barrier.Under-stimulated>0.7                 → Presence+ (raise bed, tighten timebox)
   Barrier.Blocked>0.7 / Uncertain>0.7           → Clarify
   Execution stalled>3min AND conf>0.5          → Clarify
   Barrier.Fatigued>0.7                         → Pause
   Attention.MindWandering>0.7                  → Redirect
   Attention.Hyperfocus>0.8 AND elapsed>45min   → Pause (gentle boundary)
   Goal.Decay>0.6                               → Clarify
   Step completed                                → Celebrate
   nothing fires, session live                   → Presence or Observe
3. RESOLVE + COMMIT: start refractory, decrement budget, log
```
Guards: refractory 90s (240s while Focused>0.7); budget ≤6/session; hysteresis ≥0.1 enter/exit gap;
asymmetric cost ~5:1 false-interrupt:missed-help.

**Evidence tiers**: Tier0 rules (~0ms, ₹0, bit-exact) → Tier1 cosine/exemplar-bank (~150 labeled
utterances, ~1–5ms) → Tier2 small schema-locked LLM (only when Tier-1 top-2 margin <0.15 or
long/compositional utterance, ~10–20% of utterances, off hot path, bounded influence).

**Response envelope**:
```jsonc
{
  "v": 1, "session_id": "…", "turn_id": "…", "seq": 7,
  "speech": { "text": "Nice — that's one down.",
    "audio": { "mode": "cached", "phrase_id": "win.small.v2" },
    "voice": { "provider": "fish", "voice_id": "orb.warm.v1" } },
  "orb": { "emotion": "celebratory", "intensity": 0.7, "animation": "swell",
    "bed": { "color": "brown", "gain_db": -18 } },
  "widgets": [ { "type": "step_card", "index": 2, "total": 6, "text": "Open the tax portal" },
    { "type": "progress", "done": 1, "total": 6 } ],
  "session": { "state": "STEP_PRESENT", "step_index": 2, "steps_total": 6 },
  "meta": { "model_version": "…", "prompt_version": "atomizer.v4",
    "source": "cache_hit", "latency_ms": 312, "cost_paise": 0, "trace_id": "…" }
}
```
Clients must-ignore-unknown fields/widget types; `orb` always present; `meta.source ∈ {cache_hit,
reuse, model}` mandatory.

**Context pack** (`GET /v1/session/warmup`, ~47–60KB): `profile` (~1KB), `recent_tasks[]` (last 50,
~25KB), `open_loops[]` (~5KB), `embeddings[]` (50×384-dim int8, ~19KB), `open_session` (~1KB),
`phrase_manifest` (~1KB). Lexical trigram/token-overlap first (≥0.85 → reuse), else cosine over int8
vectors (≥0.80 → seed atomizer with prior steps), else cold atomize.

**Clarify slots**: `scope`, `first_context`, `blocker`, `time_box` — rule-first extraction, model
asked to phrase question only if rule can't tell what's missing, hard cap 2 questions.

**Audio graph node config**:
```
AudioContext
 └── AudioBufferSourceNode(noiseBuffer, loop=true) → GainNode → BiquadFilterNode(lowpass, sweepable) → destination
 └── AudioBufferSourceNode(voice/cue chunks) → GainNode → destination   [mixed over bed]
```

**Cost-metering event shape**: per-session tally of `{llm_tokens_in, llm_tokens_out,
tts_chars_novel, stt_seconds_billed}`, tagged `{user, session, line: 'voice'|'llm'}`, checked
against a hard per-session reservation before any spend.

## 3. Hard numeric targets

**Scale**: 20,000 users · ~6,000 DAU · ~500 peak concurrent sessions; session 25min, ~2.5min speech,
~15 turns, ~5 real LLM calls, ~500 novel TTS chars/session. No-rewrite envelope to 200,000 users.

**Audio invariant**: 0ms unasked-for silence · 0 gap events/session · bed audible ≤120ms after
app-open · orb greets ≤250ms · transient interruption recovery ≤300ms behind a cover · 30s noise
buffer, 48kHz, crossfaded · duck −12dB / 80–150ms.

**Voice latency**: deterministic turn p50 ≤250ms, p99 ≤450ms · conversational p50 ≤600ms, p99 ≤1.2s ·
zero think-time silence: audio within ≤300ms always · backchannel max 2/utterance · barge-in yield
≤100ms · voice-to-voice eval gate: p50 ≤1.1s, p99 ≤2.0s, silence events == 0, WER ≤12% (≤18%
Hinglish), empathy-appropriateness ≥95%.

**Cost**: ceiling ≤₹120/active-user/month at 20K (lands ~₹75–96) · marginal ~₹2.5–3.2/session, voice
~85–90%, LLM ~7% · free tier capped 5 sessions/month · per-session hard reservation e.g. ₹4 · FX
$1=₹95.

**Determinism gates**: consistency K=10×N=200, step-count mode agreement ≥90%, cosine similarity
≥0.85, is-atomic ≥95% · schema-safety 100% caught · first-step-startable ≥99%, steps_total ≤12 ·
intent classifier accuracy ≥97% · belief calibration Brier ≤0.15 · policy false-interrupt ≤5%, ≤6
interventions/session, none <90s apart.

## 4. Determinism rules

**Deterministic (0 LLM decisions)**: session state transitions · the router · belief updates (pure
log-odds arithmetic) · the intervention policy · completion detection (only on explicit `done`) ·
session ending (only on explicit intent + confirm) · cost enforcement (reservation, not post-hoc) ·
prosody selection (state-derived lookup, model never picks tone).

**Allowed to call an LLM (always boxed)**: atomizer (schema-locked, validated, repair-once,
fail-closed) · ambiguous-intent classification only · conversational responder (capped length) ·
Tier-2 belief evidence classifier (only on ambiguity, off hot path) · re-atomization on repeated
stuck.

**Schema-locking**: structured decoding/function-calling → validate (schema + semantic) →
repair-once → fail-closed to deterministic fallback → gate + use strictly by reference/index (never
free text/command). Belief contribution capped `|w·r| ≤ 0.8 logits`. Pin model version string, TTS
voice id + style, prompt version (git + stamp); upgrades scheduled + eval-gated, never silent.

## 5. Failure / degradation matrix

Philosophy: presence > intelligence > richness. Silence is the one true outage.

**Degrade ladder (bed never sheds)**: 1) telemetry/eval sampling 2) speculative generation (wait for
endpoint instead) 3) premium TTS for novel text → cached premium phrases only 4) crew LLM/novel
atomization → memory reuse → templated → deterministic starter step 5) conversational replies →
deterministic acknowledgments + backchannels only 6) cloud STT/hearing → nothing to fall back to,
announce once, hold presence. **Never shed**: noise bed, state machine, cached step audio, the pause
contract.

**The pause contract** — triggered by backgrounding, screen lock, Bluetooth/headphone disconnect, or
another app playing media (all "user-intent"):
1. Bed stops (~250ms fade, never abrupt)
2. Mic released at OS level (platform recording indicator goes out)
3. STT socket closes (nothing billed)
4. UI states "paused — not listening" plainly
5. Nothing captured while paused — no buffering

Resume is explicit (user taps) or automatic-with-cover for transient interruptions only; never
silently resumes on foreground return. Transient interruptions (call, Siri, audio-stack reset):
resume context, ramp bed ≤300ms + soft swell; audio-stack death may exceed 300ms → spoken cover line.

**Network loss**: bed + cached step audio + state machine continue (tap-advance works); STT/TTS have
no on-device fallback — orb goes deaf and mute, states so once via cached audio. No degraded-voice
fallback exists, by decision.

## 6. Eval requirements

Primary gate is voice-to-voice (audio-in → audio-out), never text-only. Golden sets: voice-to-voice
N=200 (stratified by accent/language/ADHD-speech-pattern/acoustic condition) · atomizer N=300
(category × size) · consistency K=10×N=200 · intent classifier 500 utterances · belief calibration
~5,000 labeled windows.

**Atomicity rubric** (a step passes iff all four hold): 1) single physical action 2) initiation cost
<~2min 3) no embedded sub-decision 4) observable done-signal. First step held to ≥99% startable.

**Empathy grading**: hard-fail list (celebratory-on-abandonment, urgent/stern-on-checkin,
flat-on-completion) gated at 0 occurrences; prosody-variance check; judge calibrated κ≥0.8.

**CI contract — eleven gates block merge**: voice-to-voice-200, atomizer-goldens-300,
consistency-K10×N200, classifier-500, schema-safety, audio-suite, latency-harness, belief-replay,
policy-behavior, belief-calibration, cost-replay-200.

## 7. Explicitly OUT of scope for Phase 1

1. Proactive pings/notifications (foreground/session-scoped only)
2. The todo pile + prioritizer
3. Scheduling, calendars, time-of-day awareness
4. Learning the belief-model weights from data (hand-tuned; trigger ~100 annotated sessions);
   screen/app-activity sensing absent
5. Analytics/streaks/progress dashboards
6. Android audio parity at full depth (iOS is the reference target)
7. User-pickable voices, custom noise packs, self-host TTS migration
8. Accounts, cross-device sync, multi-user
9. Crisis/self-harm escalation path (real gap, flagged not solved)
10. MCP on the hot path (context already tiny and deterministic)
11. A vector database (rejected at ≤500 vectors/user)

## 8. Build-order dependency graph

- **Tier 0** (independent): `NoiseEngine.ts` + `AudioGraph.ts` (prove 0-gap in isolation first);
  `ResponseEnvelope.ts` schema.
- **Tier 1** (needs Tier 0): `DuckingMixer.ts`, `ColorMorph.ts`, `InterruptionHandler.ts`;
  `StateMachine.ts` (pure code, unit-testable standalone; INV1 needs the audio graph for e2e meaning).
- **Tier 2** (needs StateMachine): `StepGate.ts`, `Router.ts`, `CostReservation.ts`.
- **Tier 3** (needs Router + relay): `VADGate.ts`, `SemanticEndpointer.ts`, `RelayClient.ts`,
  `Backchannel.ts`, `BargeIn.ts` (stub STT/TTS while building), `IntentClassifier.ts`.
- **Tier 4** (needs schema-locking infra): `Atomizer.ts`, `ConversationalResponder.ts`,
  `CheckInPhraser.ts`.
- **Tier 5** (needs FSM + Atomizer): `ContextPack.ts`, `ClarifyProtocol.ts`.
- **Tier 6** (needs full session loop): `BeliefModel.ts`, `EvidenceTiers.ts`, `Policy.ts` (last —
  consumes nearly everything else), `ProsodyDirector.ts`.
- **Tier 7** (cross-cutting, scaffold early, gate late): `CostMeter.ts`, `EvalHarness/*`,
  `Observability/*`.

The atomizer's JSON schema is a hard prerequisite for the step gate, context-pack reuse format, and
clarify-protocol slot-filling — any change to it is breaking across three modules at once.

## 9. Recorded divergences from the blueprint (see docs/adr/ for the full ADRs)

- **LLM provider**: `@pe/llm-gateway` (the registry service this repo installs per C1/C9) currently
  ships only a `memory` (T0) and an `anthropic` adapter — no Gemini adapter exists yet. Phase 1 uses
  **Claude Haiku** as the atomizer/router's cheap model instead of the blueprint's Gemini
  2.5 Flash-Lite. Cost math changes proportionally (see `docs/adr/0002-llm-provider.md`); the
  ceiling target (≤₹120/user/month) and the boxing/schema-locking rules are unaffected — only the
  provider swaps, which is exactly the tier-swap flexibility C9 exists to buy.
- **Build sequencing**: the Company-OS constitution's default is "one slice at a time, don't
  one-shot" (`AGENTS.md` §6). The human's `/goal` directive for this build explicitly asked for
  maximum worktree parallelism across all tiers at once. Per the constitution's own precedence rule
  ("a one-off human instruction wins for its task — note the divergence"), this build parallelizes
  tiers where the dependency graph in §8 allows it, and serializes only where a real data dependency
  exists (e.g. Router cannot be reviewed before StateMachine's state enum is fixed). See
  `docs/adr/0001-parallel-build.md`.
