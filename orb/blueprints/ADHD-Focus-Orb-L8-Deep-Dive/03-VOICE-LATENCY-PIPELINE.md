# 03 — The Voice Latency Pipeline

> **Purpose:** the hot path — user-stops-speaking → orb-starts-speaking — at the depth the target demands.
> **Everything speech is cloud** (on-device STT/TTS is rejected outright, §2); audio streams to our server
> over a socket; transcripts stream back as partials; **semantic endpointing** and **speculative
> generation** remove the two waits that make cascaded pipelines slow; and **on-device backchannels**
> ("mm-hmm") cover the rest. Benchmarked honestly against speech-to-speech.
>
> **The SLO this file owns:** **deterministic turn p50 ≤ 250 ms · conversational turn p50 ≤ 600 ms,
> p99 ≤ 1.2 s · zero think-time silence.**

---

## 1. The target: what GPT-class speech-to-speech actually achieves, and why we still don't use it

| Approach | Voice-to-voice | Why |
|---|---|---|
| **Speech-to-speech** (GPT realtime, Gemini Live) | **~320 ms** best case; **~800 ms** typical in production | one audio-native model absorbs the STT wait, the LLM TTFT, and the TTS start-up |
| Naive cascaded pipeline (STT→LLM→TTS, batch) | 1–2 s+ | pays a full STT-finalize wait *then* a TTS start-up |
| **This design** (cascaded, fully streamed) | **~530 ms conversational · ~185 ms deterministic** (§4) | removes the same two waits without giving up control |

S2S is genuinely faster at the floor. We still reject it as the default, for reasons that are now *product
requirements*, not preferences:

1. **Prosody control.** Empathy is the top-priority surface ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)) — we
   need one pinned voice with a state-driven emotional register. S2S re-improvises its own delivery each
   call. The industry consensus is explicit here: when **custom prosody or strict tool reliability** matter
   more than the last ~200 ms, the chained pipeline wins.
2. **Determinism.** S2S has no inspectable text intermediate, so schema-locked steps, validation, and
   bit-exact evidence replay ([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md), [13 §7](13-COGNITIVE-STATE-AND-POLICY.md))
   all become impossible. Tool-call accuracy is also measurably lower.
3. **Cost.** ₹47–260/session vs ~₹3 ([08](08-COST-MODEL.md)) — 15–85×.

**The engineering goal that follows:** get the cascade close enough that the remaining gap is inaudible.
Human turn-taking tolerance is ~200–300 ms; **our common turn (185 ms) is already faster than S2S**, because
it invokes no model at all, and the conversational turn lands ~530 ms — inside the same band as production
S2S.

## 2. Speech is cloud-only (on-device STT/TTS is rejected)

**Rejected: platform STT/TTS.** Apple/Android on-device recognition and `AVSpeechSynthesizer`-class voices
fail the product on two counts: they are **unusably bad multilingual** (Indian English, Hinglish
code-mixing, and accented speech degrade badly), and the voices are flat — the "indifferent orb" failure
([01 §7.5](01-SYSTEM-OVERVIEW-AND-PLANES.md)). Since voice *is* the product's face, a degraded voice path is
not an acceptable fallback; it is a broken product.

| Function | Provider | Why |
|---|---|---|
| **STT** | **Sarvam Saarika/Saaras** (India/Indic + code-mixing) · **Deepgram Nova-3** (global; word-level partials < 50 ms) | streaming partials are mandatory for §3; both handle accents the platform cannot |
| **TTS** | **Fish Audio S2 Pro** (default) · **Sarvam Bulbul v3** (India) · Cartesia (latency-critical) | pinned warm voice + emotion control ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)) |
| Noise bed | **on-device, procedural** | the one thing that stays local — it needs no network and must never stop ([02](02-ALWAYS-ON-AUDIO-ENGINE.md)) |
| Backchannels / fixed phrases | **pre-synthesized premium audio, cached on-device** | premium quality at 0 ms and ₹0 — how we keep quality without a local synthesizer |

**The consequence, stated honestly (R5):** there is **no offline speech path**. Without a network the orb
keeps its bed and its cached phrases (so it stays *present*), but it cannot hear new speech or say anything
new. That is a real capability loss versus the earlier on-device-fallback design, accepted deliberately
because a robotic multilingual-failing fallback was worse than an honest "I can't hear you right now"
([09 §3](09-FAILURE-DR-AND-DEGRADATION.md)).

## 3. The transport: stream audio to our server over a socket

The device does **not** call the STT provider directly. Audio streams over a WebSocket to our own relay,
which fans out to the providers:

```
 App (RN)                    Our relay (session-pinned)              Providers
 ─────────                   ──────────────────────────              ─────────
 mic → VAD gate ──Opus 16–24kbps──▶ audio in ──────────────────────▶ STT socket
                                       │                                 │
 backchannel (cached, local) ◀─────────┤◀──── partial transcripts ───────┘
                                       │
                                   evidence → belief model (13)
                                       │
 speaker ◀── audio chunks ─────────────┴◀──── TTS socket ◀── LLM stream
```

**Why relay through our server rather than device→provider direct** (it costs one hop, ~20–40 ms to a
nearby edge):
- **Keys never ship in the app** — a client-side STT/TTS credential is extractable and would be abused.
- **Provider switching is a server config**, not an app release — Sarvam vs Deepgram vs Fish routing
  ([12 §8](12-LLD-AND-SESSION-CONTRACT.md)) changes without shipping a build.
- **The belief model sees the stream** — prosody and timing evidence is extracted server-side where the
  policy runs ([13 §5](13-COGNITIVE-STATE-AND-POLICY.md)).
- **Cost metering and the per-session cap are enforced in-path** ([08 §6](08-COST-MODEL.md)) — a client
  cannot exceed a budget it doesn't hold.

**Socket lifecycle is a cost control, not just plumbing:** the STT socket **opens on VAD onset and closes
~800 ms after endpoint**, so we bill roughly *speech* time, not *session* time. Holding one socket open for
a 25-minute session would bill ~25 min instead of ~3 — an ~8× cost error
([08 §1](08-COST-MODEL.md), scar §8.4). Bandwidth is negligible: Opus at ~20 kbps × ~3 min ≈ **~450 KB/session**.

## 4. The budget (the SLO decomposed — this table IS the design)

**Shape A — deterministic turn** (the common case: "done" → next step, already cached):

| Hop | p50 | p99 | Notes |
|---|---|---|---|
| Semantic endpoint decision | 150 ms | 300 ms | on streaming partials, not a silence timer (§5) |
| Intent match (rule, on-device) | 5 ms | 15 ms | keyword on the partial transcript |
| Response text | 0 ms | 0 ms | steps already in hand ([12 §3](12-LLD-AND-SESSION-CONTRACT.md)) |
| Step audio | **0 ms** | 250 ms | pre-synthesized during the previous step, played from local cache |
| Mix + duck the bed | 5 ms | 12 ms | [02 §4](02-ALWAYS-ON-AUDIO-ENGINE.md) |
| **Deterministic turn** | **~185 ms** | **~450 ms** | **faster than S2S** — no model, no network round trip |

**Shape B — conversational turn** (needs the model):

| Hop | p50 | p99 | Notes |
|---|---|---|---|
| Semantic endpoint decision | 150 ms | 300 ms | −300–500 ms vs a pure VAD hangover (§5) |
| LLM first sentence | **200 ms** | 600 ms | **already generating** — started on partials before the user finished (§6) |
| TTS first audio chunk | 120 ms | 350 ms | streaming socket; Cartesia ~40 ms / Fish / Sarvam ~250 ms |
| Relay + network (device↔edge, both ways) | 60 ms | 150 ms | our added hop (§3) |
| **Conversational turn** | **~530 ms** | **~1.2 s** | within the production S2S band, at ~1/50th the cost |

**Why this is fast, in one line:** we deleted the two waits that make cascades slow — **we don't wait for
silence** (semantic endpointing, §5) and **we don't wait to start thinking** (speculative generation, §6).

## 5. Semantic endpointing — don't wait for the hangover

A pure VAD hangover forces a bad trade: short cuts the user off, long feels laggy — and ADHD speech is full
of mid-thought pauses ("I need to… uh… file the taxes"). Streaming partials let us decide on **meaning**
rather than **silence**:

- A lightweight classifier reads the **partial transcript** and predicts whether the thought is complete.
  This is the single biggest win available: **300–500 ms off the average turn** with no model change.
- **Two thresholds, not one:** semantically-complete + brief pause (~150 ms) → endpoint now;
  semantically-incomplete → keep listening up to a **hard cap of 30 s**, no matter how long the pause.
- **The ADHD payoff:** a trailing "…and then I have to, um—" is *not* an endpoint however long the silence,
  so the user is not cut off mid-thought; and a crisp "done." ends the turn in ~150 ms instead of ~250.
- The endpoint classifier is small, runs on the relay against text, and is **evidence-generating** —
  incomplete-thought rate and pause structure feed Working Memory and Attention
  ([13 §5](13-COGNITIVE-STATE-AND-POLICY.md)).

### 5.1 Backchannels — the orb says "mm-hmm" while it waits

While the user is mid-utterance and pauses, silence from the orb reads as "it stopped listening." So the
device plays a **cached backchannel** — 100–200 ms of premium-voice audio ("mm-hmm", "yeah", "got it") —
locally, instantly, with no network and no model:

| Rule | Value | Why |
|---|---|---|
| Trigger | pause **250–600 ms** *and* the utterance is semantically incomplete | exactly when a human would backchannel |
| Max per utterance | **2** | more reads as interrupting |
| Never | twice consecutively · in the first 3 s · when `Emotional Load` is high | a stressed user doesn't want chirping |
| Duckable | user can talk straight over it | it is not a turn ([§7](#7-barge-in--the-user-can-always-talk-over-the-orb)) |

This is the mechanism that makes a **long hangover free**: we can afford to keep listening through a
20-second rambling brain-dump, because the user is being *audibly heard* the whole time instead of
wondering whether the orb died.

## 6. Speculative generation — start thinking before they finish

Partials go to the model **before** the endpoint fires. On a semantically-complete-looking partial, we begin
generating; if the user continues, the in-flight generation is **cancelled and restarted** (cheap — these
are ~100-token calls on the cheapest model).

- **Payoff:** the LLM's first sentence is typically ready *at* the endpoint rather than ~350 ms after it.
- **Cost of a wasted speculation:** ~₹0.01 — and the mis-speculation rate is a monitored number, not a hope.
  If it exceeds ~30 %, the trigger threshold tightens.
- **Hard rule:** a speculative result is **never spoken before the endpoint is confirmed**. Speculation
  changes *when we start computing*, never *when we start talking* — otherwise the orb interrupts on a guess.

## 7. Barge-in — the user can always talk over the orb

1. Full-duplex capture with **acoustic echo cancellation**, or the orb's own TTS trips its barge-in detector
   and it interrupts itself (scar §8.3).
2. On user voice onset, **yield ≤ 100 ms**: stop the TTS node, restore the bed toward nominal, start
   capturing.
3. **Backchannel ≠ barge-in:** a ~200 ms minimum speech duration distinguishes a real interruption from the
   user's own "mm-hm" — without it, the user agreeing with the orb stops the orb.

## 8. Measuring without lying

- **The user's clock:** last user audio sample → first orb audio sample, timestamped **on the device**.
  Server-side timing hides capture, relay, and playback — where the real lag lives.
- **Split histograms by shape × network × provider** — Shape A (~185 ms) and Shape B (~530 ms) are bimodal;
  their mean describes no real turn.
- **Track the relay hop separately** so "our infrastructure" and "the provider" are never confused.
- **Endpoint quality is two paired metrics**: false-endpoint (cut off mid-thought) and endpoint-lag. Tuning
  one alone always wrecks the other.

## 9. How to test (exactly)

1. **Voice-to-voice harness** ([06 §1.1](06-EVALS-AND-TESTING.md)) is the gate: 200 spoken tasks, audio in →
   audio out. **Gates: Shape-B v2v p50 ≤ 600 ms · p99 ≤ 1.2 s · Shape-A p50 ≤ 250 ms.**
2. **Semantic-endpoint golden set:** utterances with mid-thought pauses, trailing conjunctions, and crisp
   completions. **Gates: false-endpoint ≤ 3 % · endpoint-lag p95 ≤ 300 ms.**
3. **Backchannel behavior:** assert ≤ 2 per utterance, none consecutive, none within the first 3 s, and
   **0** cases where a backchannel is mistaken for a barge-in.
4. **Speculation efficiency:** measure mis-speculation rate (target ≤ 30 %) and assert **0** cases of a
   speculative response being spoken before endpoint confirmation.
5. **Socket-lifecycle cost test:** assert billed STT seconds ≈ speech seconds + margin over a golden session
   — **the guard against the ~8× overbill** (§3, scar §8.4).
6. **Network-condition matrix** {wifi, 4G, 3G, 2 % loss}: assert the SLO holds on 4G, and that degradation
   below it is *presence-preserving* (bed + cached audio continue) rather than silent.
7. **Think-time-silence probe:** inject 2–5 s of artificial model latency; assert audio (backchannel or
   filler) within **≤ 300 ms** and **0** silence gaps.

## 10. Operator's scars

1. **The blended-average lie.** A dashboard showed a healthy ~400 ms "average turn" — a mean of 185 ms
   cached turns and 600 ms model turns, describing neither. Split by shape or the number is fiction (§8).
2. **The filler on the model's path.** v0 generated the intake ack *from the model*, so a slow model produced
   a slow cover for the slow model. Fillers and backchannels are now cached local audio with zero model
   dependency (§5.1) — the thing that hides latency can never be on the latency path.
3. **The orb interrupted itself.** Without AEC, its own TTS tripped the barge-in detector mid-sentence. Then,
   after adding backchannels, the user's own "mm-hm" started stopping the orb — fixed by the 200 ms minimum
   speech duration (§7).
4. **The socket we forgot to close.** Holding the STT socket open for the whole session billed ~25 min per
   session instead of ~3 — an ~8× cost overrun discovered in the first real invoice, not in staging, because
   staging sessions were 90 seconds long. Socket lifecycle is now asserted in CI (§9.5).
5. **Speculation that spoke too early.** An early build let a speculative reply start playing when the
   partial "looked done" — it talked over users mid-sentence. Speculation now changes only *when computing
   starts*, never *when speaking starts* (§6).
6. **Trusted the platform recognizer for one release.** On-device STT tested fine in English and shipped;
   Hinglish and accented users got garbage transcripts and blamed themselves for "not being understood."
   Cloud STT is now the only path, with no silent quality fallback (§2).

## 11. Interview questions this file answers

- "GPT realtime does ~320 ms. Why are you slower, and why not just use it?" (§1 — prosody control, determinism, 15–85× cost; and our common turn is *faster*)
- "Break down your latency budget." (§4 — two shapes, hop by hop)
- "How do you avoid waiting for the silence timeout?" (§5 — semantic endpointing on partials, −300–500 ms)
- "The user pauses mid-sentence — what does the orb do?" (§5.1 — cached backchannels, and why that makes a long hangover free)
- "Do you start the LLM before the user stops talking?" (§6 — speculative generation, and the rule that it never speaks early)
- "Why relay audio through your server instead of straight to the provider?" (§3 — keys, provider switching, evidence extraction, in-path cost caps)
- "What happens with no network?" (§2 — present but deaf/mute, stated honestly; no degraded-voice fallback)
