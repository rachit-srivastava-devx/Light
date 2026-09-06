# Code traversal guide — how to review this project yourself

Written 2026-08-05, after a full L8 audit + live-key-readiness pass. Everything in this doc was
personally traced and verified this session (live device runs, real API calls, grep for actual
callers) — not inferred from file names or doc comments. Where something is unverified, it says so.

This is a companion to [`../REVIEW-2026-08-04-L8.md`](../REVIEW-2026-08-04-L8.md) (the findings) and
[`../JUNIOR-TRAVERSAL.md`](../JUNIOR-TRAVERSAL.md) (the build-agent onboarding doc, narrower and
partly stale). This doc is for a human reviewing the actual code, end to end.

---

## 1. Read order

1. [`../../AGENTS.md`](../../AGENTS.md) — the product's non-negotiable rules (determinism, C9
   one-model-door, money-as-integer-paise). Skim once; you'll recognize violations faster if you
   know the rules first.
2. [`../BUILD-DIGEST.md`](../BUILD-DIGEST.md) — the spec this was built against, extracted from the
   blueprint. Sections 1–4 are the module map and FSM; you don't need the rest to review code.
3. This doc, for the *actual* wiring (the digest describes intent; this describes what's real).
4. [`../REVIEW-2026-08-04-L8.md`](../REVIEW-2026-08-04-L8.md) — the running findings log, newest
   section first (§10 is the latest).

---

## 2. The real end-to-end flow (voice turn, live-key mode)

This is the path a single spoken utterance actually takes through the code, as of the 2026-08-05
race-condition fix. Read this before reading any individual file — most files only make sense in
context of where they sit in this chain.

```
Mac/device mic
  → OrbMicModule.kt (native Android, AudioRecord)                      [android/app/.../OrbMicModule.kt]
  → NativeMicPort.ts (JS bridge, PCM → VadFrame + ArrayBuffer)         [apps/mobile/src/voice/NativeMicPort.ts]
  → VoiceLoopController.handleAudioFrame (VAD state machine)          [apps/mobile/src/runtime/VoiceLoopController.ts]
      - energy above threshold → relay.sendAudio(frame) each frame
      - energy drops (local VAD end-of-turn) → relay.endOfTurn() ONLY, does NOT process locally
  → RelayClient (WebSocket client)                                    [apps/mobile/src/voice/RelayClient.ts]
  → backend/relay-rs (WebSocket server, session.rs state machine)     [backend/relay-rs/src/session.rs]
  → backend/voice-provider-sidecar (real STT call)                    [backend/voice-provider-sidecar/src/]
      - real provider (Sarvam/Deepgram) OR fake mode, per ORB_STT_PROVIDER
  → relay-rs sends the real transcript back as a 'transcript' ServerFrame (is_final: true)
  → RelayClient.onTranscript → App.tsx wiring → VoiceLoopController.handleTranscript
  → completeTurn(reason, realText)   ← the ONLY place a turn actually completes
  → T0FocusSession.acceptAudio(realText)                              [apps/mobile/src/runtime/T0FocusSession.ts]
      - detectCrisis(transcript) FIRST — short-circuits everything below if it fires
      - state machine: INTAKE/CLARIFY → atomizer; WORKING → intent router → policy
  → AtomizerPort.atomize() → backend/relay-py /v1/atomize                [apps/mobile/src/runtime/AtomizerPort.ts]
  → backend/gateway-sidecar → registry/services/llm-gateway adapter    [backend/gateway-sidecar/src/index.ts]
      - real Gemini/Anthropic call OR memory (fake) mode, per ORB_LLM_GATEWAY_ADAPTER
  → response envelope built, speech text chosen                        [apps/mobile/src/runtime/T0FocusSession.ts]
  → EITHER local native TTS (OrbSpeechModule.kt, always active)
    OR relay-delivered TTS: relay.speak() → relay-rs → voice-provider-sidecar TTS call
      → WAV container stripped server-side (wav.ts extractPcmFromWav) → raw PCM chunks
      → RelayAudioPlayer.ts plays them through the presence bed's voice branch
  → CloudOrb.tsx (Skia aurora visual) reflects state throughout          [apps/mobile/src/orb/]
```

**The two things that make this chain trustworthy, not just plausible:** (1) the local-VAD-vs-relay
race that used to double-process every utterance is fixed — verified by a test that simulates the
real async server round-trip, not a shortcut. (2) TTS audio is WAV-container-stripped server-side
before the client ever sees it, closing the "client assumes raw PCM, provider sends a WAV header"
gap — verified by 6 tests including a round-trip through this repo's own WAV writer.

**What's still unverified in this chain:** every real-provider HTTP call (Sarvam, Deepgram, Fish,
Cartesia, Gemini's response *parsing* on a 200) — the *request* side of each was live-tested against
the real endpoint (confirmed real HTTP errors, real 400s), but no valid key has been available this
session to confirm a real 200 parses correctly end to end.

---

## 3. File map, by plane

Each entry: what it does, whether it has a real caller (not just a test), and its confidence level.

### `apps/mobile/src/presence/` — the audio bed (must never import cognitive/voice)
| File | Role |
|---|---|
| `NoiseEngine.ts` | Procedural pink/brown noise generation. Best-tested file in the repo. |
| `AudioGraph.ts` | Web Audio graph construction; composes Ducking/ColorMorph/Interruption internally. |
| `PresenceBoot.ts` | Boots the bed once per runtime; real caller in `App.tsx`. |
| `PresenceLifecycle.ts` | Binds bed pause/recover to `AppState` foreground/background. |
| `DuckingMixer.ts`, `ColorMorph.ts`, `InterruptionHandler.ts` | Composed into `AudioGraph`'s returned object — not called directly by app code. |

### `apps/mobile/src/voice/` — VAD, endpointing, transport
| File | Role |
|---|---|
| `NativeMicPort.ts` | Bridges the native `OrbMic` module; `energyToSpeechProbability` is the (documented, crude) VAD heuristic — no real ML VAD model exists. |
| `VADGate.ts` | Pure onset/candidate/listening/end-of-turn state machine over frames. |
| `RelayClient.ts` | WebSocket client to `relay-rs`. Real caller: `App.tsx`. |
| `RelaySocket.ts` | Queues frames sent before the socket reaches `OPEN` — a real bug fix from earlier this session, not decoration. |
| `RelayAudioPlayer.ts` | Plays relay-delivered TTS audio through the presence bed. |
| `SemanticEndpointer.ts`, `Backchannel.ts`, `BargeIn.ts` | Pure decision logic, exercised via `VoiceLoopController`. |
| `Utf8.ts` | Hand-rolled UTF-8 codec (avoids relying on Hermes's `TextEncoder` availability). |

### `apps/mobile/src/runtime/` — the actual orchestration (read this plane most carefully)
| File | Role |
|---|---|
| `VoiceLoopController.ts` | **The file that owns the mic-vs-relay race.** If you're auditing for a similar bug elsewhere, this is the pattern to check for: does any path process a result before the async source of truth has actually replied? |
| `T0FocusSession.ts` | The FSM driver: crisis check → intake/clarify → atomizer → router → policy → envelope. Read `acceptAudio` top to bottom; it's the single most important function in the client. |
| `AtomizerPort.ts` | `createRelayAtomizerPort` (real HTTP call, schema-validates the response) vs `createStaticAtomizerPort` (offline fallback). **Composition root check:** confirm `App.tsx` actually wires the relay port, not just the static one — this exact gap existed once already this session. |

### `apps/mobile/src/{session,router,cognitive,lld}/` — deterministic control planes
| File | Role |
|---|---|
| `session/StateMachine.ts` | The 9-state FSM as data (`SESSION_TRANSITION_TABLE`) + a pure reducer. |
| `session/CostReservation.ts` | Integer-paise budget enforcement, fails closed. |
| `router/Router.ts`, `router/IntentClassifier.ts` | Pure intent→action table; rule-based classifier, no model call on this path. |
| `cognitive/CrisisDetector.ts` | Regex-based, precision-first (see its own module doc for the exact false-positive reasoning). **English-only — no Hindi/Hinglish coverage.** Not clinically reviewed. |
| `cognitive/Policy.ts`, `BeliefModel.ts`, `EvidenceTiers.ts` | The intervention policy and belief vector — real caller confirmed in `T0FocusSession.applyPolicy`/`observeTranscript`. |
| `lld/ContextPack.ts` | Lexical-then-cosine retrieval; cosine tier never fires because no real embeddings are generated server-side (a stated, deliberate limitation, not a bug). |

### `apps/mobile/src/orb/` — the visual
| File | Role |
|---|---|
| `AuroraMist.tsx` | Real `@shopify/react-native-skia` component — actual native GPU rendering, verified live on-device (screenshots show real animation drift between frames). |
| `CloudOrb.tsx` | Wraps `AuroraMist`, maps app state → color/speed/scale. |

### `android/app/src/main/java/com/orbmobile/` — native modules
| File | Role |
|---|---|
| `OrbMicModule.kt` / `OrbMicPackage.kt` | Real `AudioRecord`-based capture, 16kHz mono PCM16. Verified live: real permission prompt, real continuous frame capture on-device. |
| `OrbSpeechModule.kt` / `OrbSpeechPackage.kt` | Real Android `TextToSpeech` wrapper. Verified live: real `speak_requested`/`queued` logcat events. |

### `backend/gateway-sidecar/` — the one LLM door (C9)
`src/index.ts` selects `memory` / `anthropic` / `gemini` via `ORB_LLM_GATEWAY_ADAPTER`. This is the
**only** file in the whole repo allowed to import a provider SDK for the LLM — `boundary-lint.mjs`
enforces this across TS/Python/Rust.

### `backend/voice-provider-sidecar/` — the one STT/TTS door
Mirrors the gateway-sidecar's role for voice. `src/stt/{sarvam,deepgram}.ts` and
`src/tts/{fish,sarvam,cartesia}.ts` each carry a `CONFIDENCE NOTE` doc comment stating exactly which
field names/formats are unverified against real docs — **read those comments before trusting a
provider integration**, they're not boilerplate. `src/wav.ts` strips WAV containers before chunking
(the 2026-08-05 fix).

### `backend/relay-py/` — orchestration (atomizer, cost, eval, context store)
| File | Role |
|---|---|
| `proxy/atomizer.py` | Schema → semantic-check → repair-once → fail-closed pipeline. Real caller: `app.py`'s `/v1/atomize`. |
| `cost/meter.py` | Integer-paise reservation, real pricing defaults (Haiku 4.5 / Fish / Sarvam, Aug-2026 sourced). |
| `store/context_store.py` | SQLite persistence for recent tasks — real caller confirmed in `app.py`. |
| `eval/gates.py`, `eval/consistency_replay.py` | The 11 CI gates + a real K×N consistency harness. **Runs at K=5×N=10 against a scripted fake client, not a live model** — proves the harness arithmetic, not model consistency. |

### `backend/relay-rs/` — realtime socket/session data plane
| File | Role |
|---|---|
| `session.rs` | Pure state machine over `ClientFrame`/`Action` — the server-side twin of `VoiceLoopController.ts`. This file was already correct; the bug this session found was entirely client-side. |
| `provider.rs` | `SttProvider`/`TtsProvider` traits + `HttpContractProvider` (real HTTP client to the voice-provider-sidecar) + `FakeProvider` (T0 default). |

---

## 4. How to verify a claim yourself, not just read it

Every "this works" claim in this codebase should be checkable by one of these, in order of rigor:

1. **Grep for a real caller.** A function with tests but no production call site is a scaffold, not
   a feature (this codebase has been burned by this exact pattern — see `docs/adr/LESSONS.md`).
   `grep -rn "functionName" apps/mobile/src backend --include="*.ts" --include="*.py" | grep -v test`
2. **Run the verify gate.** `PATH=$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH npm run
   verify` — lint, typecheck (3 tsconfigs), vitest, pytest, cargo test. Green is the floor, not proof
   of correctness — it doesn't catch composition bugs across files (see §2's race condition, which
   no unit test caught because no test crossed the local-VAD/relay-transcript boundary).
3. **Drive it live.** Boot the Android emulator + all 4 backend processes (`npm run start:sidecar:t0`,
   `start:voice-sidecar:t0`, `start:relay:t0`, `start:relay:rs:t0`), install the built APK, watch
   `adb logcat`. This is the only way the two 2026-08-05 bugs were actually found — reading the code
   made them plausible, driving the flow made them certain.
4. **Hit a real provider endpoint directly.** `curl` the sidecar with a real (or deliberately invalid)
   key and read the actual response — this is how the Gemini adapter's request-construction was
   confirmed real (`curl` → real Google API error, not a mock).

## 5. Known live gaps (condensed — full list in the review doc)

- No valid API keys have been used this session — every real-provider *response-parsing* path is
  unverified, only request-construction and error-handling are.
- iOS is blocked on this host's CoreSimulator version (needs a macOS update + restart).
- Physical-device audio (the ≤120ms/≤250ms/≤300ms budgets) is emulator-only so far.
- Crisis detection is English-only and has not had human safety review.
- Eval/consistency evidence runs at toy scale (K=5×N=10), not the blueprint's K=10×N=200.
