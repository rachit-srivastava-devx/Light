# ADR 0013: VAD and endpointing architecture (Track K)

- Status: accepted (interim heuristic), recommended-not-implemented (silero-vad / smart-turn)
- Date: 2026-08-28
- Rules: C1/L2 (registry-first), C3 (no dependency without an ADR), C9 (one model door — N/A, this
  stays 0-LLM), determinism (`docs/BUILD-DIGEST.md` §4), the reuse gate (owner directive 2026-08-28)
- Owns: `apps/mobile/src/voice/NativeMicPort.ts`, `apps/mobile/src/voice/VADGate.ts`,
  `apps/mobile/src/voice/SemanticEndpointer.ts` and their tests only (Track K's scope)

## Context

Audit finding #1 (HIGH): `NativeMicPort.ts`'s `speech_probability` was a single linear-RMS
interpolation between a silence floor and a speech ceiling — a documented, admitted placeholder.
Real consequence: any sufficiently loud non-speech sound (HVAC, a fan, another conversation) reads
as speech (false session opens, false barge-ins, burning the ~₹4 session reservation), and soft or
far-field speech reads as silence (the user gets cut off). Finding #2 (MEDIUM-HIGH): a second,
independent defect existed in `SemanticEndpointer.ts` — its `CONTINUATION_MARKERS` was
English-only, so a Hindi/Hinglish speaker trailing off on "और"/"aur" was endpointed while still
speaking. **That specific defect was already fixed by a concurrent worker on this same tree before
this ADR was written** (verified: `SemanticEndpointer.ts`'s `CONTINUATION_MARKERS_EN` /
`CONTINUATION_MARKERS_HI` split, `SemanticEndpointer.test.ts`'s 19-case census, both passing) —
this ADR records the remaining architecture questions: the VAD model itself, a real reuse-gate
evaluation of what a curated marker list should have been replaced with, and a new requirement
raised mid-track: an eager/confirmed two-tier endpoint signal for the filler-covering track (J) to
subscribe to, plus a measured latency-vs-false-cutoff tradeoff curve.

**Binding constraint on this ADR's decision:** this track owns exactly three client TypeScript
files and their tests. It does not own `package.json`, any native project file (`ios/`, `android/`),
`metro.config.js`, or any backend file (`backend/relay-rs/**`, `backend/relay-py/**`,
`backend/voice-provider-sidecar/**`). Every option below that would need a new npm/native/backend
dependency is therefore a **recommendation with a concrete integration plan**, not something this
change executes — consistent with the brief's own instruction: "if (a) requires a dependency you
cannot add, implement behind the existing port interface... and REPORT the exact dependency needed."

## Options considered for the VAD model

| Option | What it is | Latency | Ownership blocker |
|---|---|---|---|
| (a) On-device silero-vad via `onnxruntime-react-native` | Real neural VAD, ONNX, MIT | Best for barge-in (no network hop); silero's own published inference cost is ~1ms/frame on CPU for its 30ms-frame model | New npm dep + native iOS/Android linking + bundled model asset — none in this track's owned files |
| (b) Server-side silero (or Sarvam's own VAD) in the relay | Audio already streams to `backend/relay-rs`/`relay-py` | Adds a network round trip (this product's own blueprint quotes 60-150ms for the relay hop, §3/§4 of `03-VOICE-LATENCY-PIPELINE.md`) to a decision that today is entirely local | `backend/relay-rs/src/provider.rs` / `backend/voice-provider-sidecar/**` — not this track's files |
| (c) Hybrid: cheap local gate decides what to transmit, authoritative endpoint server-side | Closest to today's actual shape (`VADGate.ts` already gates locally; `SemanticEndpointer.ts` already runs client-side, not relay-side as the blueprint assumed — a pre-existing drift, flagged, not fixed here) | Same backend blocker as (b) | Same as (b) |
| **(d) Chosen for THIS change: keep the port fully swappable, ship a real (non-neural) multi-feature heuristic as the interim default, recommend (a) for silero specifically** | See below | Measured, see Evidence | None — pure TypeScript, zero new dependencies |

**Decision: (d) now, (a) recommended next.** On-device silero via `onnxruntime-react-native` is the
right long-term answer specifically *because* barge-in has a 100ms yield budget
(`BARGE_IN_YIELD_BUDGET_MS`) and this pipeline's own numbers show a relay round trip alone can cost
60-150ms (§4) — putting the frame-level speech/silence decision behind a network hop risks that
budget for no latency benefit, and it duplicates work `VADGate.ts` already does locally. (b)/(c) are
rejected as the *primary* VAD placement for the same reason, though see the Sarvam finding below for
where server-side involvement is still the right call for a different, complementary reason.

## What actually shipped in this change

`NativeMicPort.ts` now exposes a typed `VadClassifier<S>` port (`classify(state, pcm, sampleRateHz)
-> {state, result}`) with three implementations:

1. `createRmsVadClassifier()` — the original linear-RMS heuristic, **unchanged**, kept and exported
   by name as the explicit, always-available fallback (`source: 'heuristic-rms'`).
2. `createMultiFeatureVadClassifier()` — **the new default**. Still a classical DSP heuristic, not a
   model: an adaptive noise-floor tracker (asymmetric EMA — rises slowly toward a sustained louder
   background over ~1-2s, falls quickly toward a quieter one) combined with zero-crossing rate and
   spectral flatness (via a hand-written radix-2 FFT, correctness proven by a Parseval
   energy-conservation test and a known-bin-peak test, not just eyeballed). `source:
   'heuristic-multifeature'`.
3. `createSileroVadClassifier()` — **throws `VadDependencyUnavailableError`**, naming the exact
   missing pieces (`onnxruntime-react-native`, the model asset, native linking). This is the
   concrete seam: once Track A adds the dependency, this factory's body is replaced with a real
   ONNX session and `startCapture`'s default swaps with no change to `VADGate.ts` or any caller.

Every `VadFrame` emitted by `startCapture` now carries `vad_source` (an optional field on the
existing type, so no caller elsewhere in the tree needed to change) and `startCapture` logs once,
at capture start, which classifier is active and whether it is a fallback
(`focus-orb.vad_classifier_active`, `is_fallback: boolean`) — the "detectable, never silent"
requirement. A silent, undetectable fallback is this product's defining scar (native TTS standing
in for Fish with nothing surfacing it); this VAD swap cannot repeat that.

## Reuse-gate declaration (G-R1/G-R3) for the multi-feature heuristic

**Branch: build-new, and it should be replaced, not extended.** G-R2 explicitly lists
"Silence/dead-air detection by hand -> use silero-vad · webrtcvad · librosa" as a named defect
pattern. This ADR does not dispute that — the multi-feature classifier shipped here is a bridge,
not the destination. It was hand-rolled because (checked, none fit):
- `silero-vad` (MIT, actively maintained, ONNX) — the correct answer, blocked on the dependency
  above. **This is the recommendation**, not the heuristic.
- `webrtcvad` — dead since 2017; the maintained fork is `webrtcvad-wheels`, still a native
  (Python/C) binding, same npm/native-dependency blocker as silero for an RN client.
- A pure-JS FFT/DSP npm package for the flatness/ZCR features — none is already a dependency of this
  tree, and the FFT itself (a ~40-line textbook radix-2 Cooley-Tukey) is not the kind of "solved
  problem with a named library fix" G-R2 enumerates (WAV parsing, sentence splitting, retry/backoff,
  percentiles, and VAD itself are — a generic FFT primitive is not on that list, and correctness is
  independently proven by a Parseval-theorem test rather than merely asserted).

## Real-audio evidence (old RMS vs new multi-feature, same fixtures)

Ran via `npx vite-node` importing the actual shipped functions (not a re-implementation) over the
repo's real fixtures: `e2e-human-simulator/runs/live-20260807/{hello-how-are-you,next-step,stuck}.wav`
(16kHz mono, real recorded speech) plus three real noise recordings bundled with the `evals/`
Python dependencies (`scenario`'s `voice/assets/noise/{babble,office,street}.wav`, 24kHz mono, 3s
each) as negative controls. 20ms frames, `VAD_SPEECH_PROBABILITY = 0.6` as the onset threshold.

**Noise-only negative control** (office.wav + street.wav — no foreground speaker; babble.wav is
reported separately because it contains real background human speech, which no single-channel
VAD — old, new, or silero — can distinguish from target speech; that is an acknowledged, unsolved
gap, not something claimed fixed here):

| | Denominator | Frames misread as speech-onset-worthy | Rate |
|---|---|---|---|
| OLD (RMS) | 300 frames | 298 | 99.3% |
| NEW (multi-feature) | 300 frames | 260 | 86.7% |

Split by fixture: **street.wav improved substantially** (148/150 -> 110/150, 98.7% -> 73.3% — the
spectral-flatness term doing real work against broadband noise). **office.wav barely moved**
(150/150 -> 150/150, mean probability 1.000 -> 0.993). Root cause, measured directly: both noise
fixtures' overall RMS (~0.134-0.137) already **exceeds `SPEECH_RMS_CEILING` (0.12)** — these
fixtures are louder, in raw RMS terms, than this product's own definition of "confident speech."
The adaptive floor is deliberately capped (`NOISE_FLOOR_MAX = 0.9 x ceiling`) so it can never rise
high enough to swallow genuinely loud speech — the same protection that stops the floor from
suppressing a real loud utterance also caps how much a noise sample *louder than the ceiling itself*
can be suppressed by floor adaptation alone. street.wav's improvement is close to 100% attributable
to the spectral-flatness term (broadband-shaped); office.wav's near-zero improvement says its
spectral shape must be closer to tonal/peaky, where that term does little. **This is exactly the
gap a learned model (silero) closes and a hand-tuned energy/shape heuristic structurally cannot**:
recognizing "this is not speech" from acoustic pattern, independent of whether it happens to be
louder than an arbitrary ceiling constant.

**Sustained-hum decay (the headline scenario from the audit — HVAC/background noise read as
speech):** a 150Hz tone loud enough that the OLD heuristic reads it as speech-onset-worthy
(probability > 0.6) **on every single frame, forever** (a fixed threshold cannot adapt). Fed through
the NEW classifier for 150 consecutive frames (~3s): frame 1 probability > 0.3 (floor hasn't
adapted yet, honestly reported, not hidden), frame 150 probability < 0.15 (floor has caught up and
suppressed it) — proven in `NativeMicPort.test.ts`'s `'THE HEADLINE CASE'` test and reproduced
independently in the real-audio evidence script. The old heuristic, re-verified on the identical
frame, never changes.

**Speech fixtures:** no hand-labeled ground truth exists for `hello-how-are-you.wav` /
`next-step.wav` / `stuck.wav` (no forced alignment, no manual annotation) — reported as an
old-vs-new agreement comparison, not accuracy-vs-truth, to avoid manufacturing a precision this
track does not have. Both classifiers still open exactly one listening episode per fixture (no
regression in the basic "does it still detect the utterance" sense); mean probability is
consistently lower under the new classifier (0.60/0.53/0.49 -> 0.57/0.46/0.42), consistent with the
new classifier being less loudness-trigger-happy in general, which is the intended direction.

**Per-frame inference cost** (320-sample/20ms frame @16kHz, 5,000 iterations, warmed up):

| | mean | p50 | p95 | p99 |
|---|---|---|---|---|
| OLD (`energyToSpeechProbability`) | 2.0us | 2.0us | 2.1us | 2.3us |
| NEW (`classifyMultiFeature`) | 16.0us | 15.2us | 17.9us | 21.0us |

Both are 3-4 orders of magnitude under the 20ms frame period and the 100ms
`BARGE_IN_YIELD_BUDGET_MS` — the new classifier's ~8x cost increase (the FFT) is real but
negligible against either budget. (A few outlier max samples in the 1-3.5ms range were observed in
the benchmark loop, consistent with JIT/GC noise, not steady-state cost — p99 is the number that
matters here and it holds.)

## The eager/confirmed two-tier endpoint signal (added mid-track, per coordinator research)

Two independent research findings changed this track's endpointing design:
1. **Endpointing latency is a published, chosen tradeoff, not a fixed cost.** LiveKit's own
   turn-detector numbers: ~295ms endpoint latency at a 10% false-cutoff (mid-thought interruption)
   rate, ~543ms at 5%. **LiveKit's model itself is NOT adopted anywhere in this product** — its
   weights carry a custom license restricted to LiveKit Agents — reported and checked against the
   LICENSE file by the coordinating research pass for this build, not independently re-verified by
   this track in this session — and this product is commercial, so it is treated as a hard no
   regardless. Only the published latency numbers are used, as an external benchmark.
2. **LLM time-to-first-token is now the dominant term in the voice-to-voice budget** (~490-900ms
   across the providers checked, per the same research pass), which means every millisecond an
   "I'm probably done" signal fires earlier is a millisecond of head start on generation — this
   raises the value of a genuinely early, non-committal signal well above what it would be if
   generation were instant.

Both `VADGate.ts` and `SemanticEndpointer.ts` now emit an eager signal strictly before their
existing confirmed signal, non-committally (state and turn-completion are unaffected by eager
firing — only the confirmed decision ends a turn):

- **`VADGate.ts`: `{kind: 'eager_end_of_turn', reason: 'post_endpoint_quiet_eager'}`** — fires once
  per quiet run, at `VAD_EAGER_POST_ENDPOINT_MS` (150ms, configurable via `reduceVadGate`'s third
  parameter) into post-endpoint silence, well before the confirmed `end_of_turn` at
  `VAD_POST_ENDPOINT_CLOSE_MS` (800ms, `contracts.ts`, not owned by this track). This is the
  **audio-domain** signal — no STT round trip, so it is architecturally the earliest "user has
  probably stopped" signal available anywhere in this pipeline. 150ms was chosen, not measured: the
  owner's hard budget is continuous-audio-within-250ms-of-the-user-stopping, and the modal human
  inter-speaker gap is ~0-200ms (Stivers et al., PNAS 2009) — 150ms leaves real margin before the
  250ms wall.
- **`SemanticEndpointer.ts`: `{kind: 'eager_endpoint', reason: 'complete_thought_eager'}`** — fires
  once `pause_ms` crosses `ENDPOINT_EAGER_PAUSE_MS` (60ms, configurable via `decideEndpoint`'s
  fourth parameter) and the transcript already looks complete, before the confirmed
  `ENDPOINT_COMPLETE_PAUSE_MS` (150ms). This is the **text-domain** signal.
- **Neither is currently wired to anything** — `VoiceLoopController.ts` (Track J's file, not
  editable by this track) does not yet branch on `'eager_end_of_turn'` / `'eager_endpoint'`;
  both currently fall through to existing `ignored`/`hold` handling, so today's behavior is
  byte-for-byte unchanged. Track J's filler scheduler is the intended subscriber — see the Track J
  section below for the exact contract.

**Measured latency-vs-false-cutoff curve, on the real WAV fixtures** (not LiveKit's methodology,
which is model-confidence-based; this is the audio-domain gate's own tunable dial, swept directly):
for each of the 3 real speech fixtures, ran the full VADGate state-machine shape at candidate
post-endpoint-close thresholds and counted how many distinct listening episodes resulted — more
than one means that ONE continuous recorded utterance was split into two turns, i.e. a real,
measured mid-utterance false cutoff at that threshold:

| Threshold | False-cutoff rate | Detail |
|---|---|---|
| 60ms | 100% (3/3 split) | every fixture split at least once |
| 100-200ms | 67% (2/3 split) | `next-step.wav` alone stops splitting by 100ms |
| **250ms+** | **0% (0/3 split)**, flat through 1200ms | all 3 fixtures resolve to one episode |

Denominator: 3 fixtures x 13 threshold values = 39 gate runs. **This small sample (n=3) gives only
33%-step resolution** — it is not a substitute for the blueprint's own N=200 voice-to-voice
false-endpoint gate (`docs/BUILD-DIGEST.md` §6), which remains the authoritative measurement. What
it does show, honestly: this product's actual confirmed audio threshold (800ms) sits comfortably
inside the 0%-false-cutoff zone on these 3 recordings, and even a much shorter 250-300ms would not
have split them — though ADHD speech's characteristic longer trailing pauses (the reason
`VAD_POST_ENDPOINT_CLOSE_MS` is 800ms and not shorter) are exactly the case 3 short clean recordings
cannot stress-test. The text-domain `ENDPOINT_COMPLETE_PAUSE_MS` (150ms) has no equivalent
self-consistent curve to sweep: `looksIncomplete`'s accuracy does not vary with the pause
threshold (it is a binary marker match, not a confidence score), so the only way its false-cutoff
rate changes is via better marker coverage (Defect 2's fix) — a fundamentally different lever than
"wait longer."

## Two off-the-shelf answers researched, not adopted this session

- **Pipecat's `smart-turn` model** — BSD-2-Clause, a real semantic (audio-based, not
  keyword-based) turn-detection model, 14 languages including Hindi. This is the correct long-term
  answer to Defect 2's underlying problem — better than any curated marker list, English or
  bilingual. **Not adopted here**: it is an audio-domain model (same integration shape as silero:
  new dependency + likely an ONNX/native runtime), and this track owns no path to add it. Recorded
  as the recommended next step, same status as silero.
- **Sarvam's `saaras:v3-realtime` STT** already has a real adapter in this tree
  (`backend/voice-provider-sidecar/src/stt/sarvam.ts` — confirmed by reading it, read-only; not
  edited, out of this track's scope) and exposes `codemix` mode for Hinglish plus VAD tuning knobs
  (`silence_duration_ms`, `min_speech_duration_ms`) and true partial transcripts.
  `SARVAM_API_KEY` is funded, so this is real-testable, unlike silero/smart-turn which need new
  dependencies. **Not exercised in this session**: wiring those knobs is
  `backend/voice-provider-sidecar/**` / `backend/relay-rs/src/provider.rs`, not this track's files,
  and a live-API call was judged out of scope for a client-VAD track rather than skipped for lack of
  time. **Recommendation for whichever track owns the provider adapter next**: prefer configuring
  Sarvam's own endpointing knobs over building parallel client-side logic for the same problem —
  the provider already streams the audio and already handles 22 Indian languages' code-mixing; this
  product should not re-derive that.
- **Licence note, stated once for the record:** LiveKit's turn-detector model ships under a custom
  "LiveKit Model License" restricted to LiveKit Agents (per the coordinating research pass's check
  of its LICENSE file) — not adopted, anywhere, in any form, in this product. Only its published latency numbers are used as
  an external benchmark, exactly as this track was directed.

## Track J's subscription contract (the ask: "keep the interface clean, document the exact event")

Track J's filler scheduler (building the 250ms continuous-audio guarantee, in
`apps/mobile/src/runtime/VoiceLoopController.ts` / `backend/relay-py/src/orb_relay/observability/**`,
neither owned by this track) should subscribe to, in order of how early each fires:

1. `VadDecision.kind === 'eager_end_of_turn'` from `reduceVadGate` (`VADGate.ts`) — the earliest
   possible signal, audio-only, no STT dependency. Non-committal: state stays `'listening'`, audio
   keeps flowing exactly as on a plain `hold`. Fires at most once per quiet run.
2. `EndpointDecision.kind === 'eager_endpoint'` from `decideEndpoint` (`SemanticEndpointer.ts`) —
   fires when a transcript partial already looks complete, ahead of the confirmed pause. Currently
   **unreachable in production**: see the finding below.
3. The existing confirmed signals (`VadDecision.kind === 'end_of_turn'`,
   `EndpointDecision.kind === 'endpoint'`) remain the only turn-completion authority; nothing above
   changes when a turn actually ends.

**A related, more urgent finding for whoever wires this next:** `apps/mobile/src/App.tsx:355`
hardcodes `const pauseMs = 0` when calling `voiceLoop.handleTranscript`, with a documented,
deliberate reason ("never endpoints early on a partial, only on `isFinal`"). One consequence,
confirmed by re-reading `decideEndpoint`: **`SemanticEndpointer.ts`'s entire early-completion path
(both the pre-existing bilingual fix and this change's new eager tier) is currently unreachable in
the running app** — `pause_ms < eagerPauseMs` (0 < 60) is always true, so `decideEndpoint` always
returns `too_soon` before `looksIncomplete` is ever consulted, except at the 30s hard cap. Today's
real endpointing authority is exclusively `VADGate.ts`'s audio-level hangover. This is not a defect
introduced by this track (the wiring predates it, and `App.tsx` is not an owned file), but it means
the bilingual/eager work in `SemanticEndpointer.ts` is currently a **correct, tested, but
production-inert** contract — real value only once `App.tsx` computes a genuine `pause_ms` from
frame timestamps instead of the hardcoded 0.

## Consequences

- Shipped: a swappable VAD port, a measurably-better-than-RMS (but explicitly non-neural) default
  classifier, a detectable/labelled fallback, an audio-domain eager/confirmed pair with a measured
  curve, a text-domain eager/confirmed pair (currently inert pending the `App.tsx` wiring above).
- Not shipped, and not claimed shipped: silero-vad or smart-turn actually running; Sarvam's VAD
  knobs actually configured; `SemanticEndpointer.ts`'s early-completion path actually reachable in
  production.
- **Revisit trigger:** the moment `onnxruntime-react-native` (or an equivalent) lands as an owned
  dependency, `createSileroVadClassifier` is where the real model goes — no change needed to
  `VADGate.ts` or any caller. The moment `App.tsx`'s `pauseMs` is wired to a real value, re-run this
  track's eager/confirmed tests against the live app and re-measure the false-cutoff rate for real,
  not on 3 fixtures.
