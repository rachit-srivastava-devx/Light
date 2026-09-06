# 01 — System Overview & Planes

> **Purpose:** the whole of Phase 1 in one file — the session lifecycle traced end-to-end, the module
> decomposition and the **React-Native-shell / native-module boundary** (and why it is where it is), the
> sync/async split, the deterministic session state machine at altitude, and the hop budget overview that
> [03](03-VOICE-LATENCY-PIPELINE.md) makes exact. Read this, then [02](02-ALWAYS-ON-AUDIO-ENGINE.md) (the
> invariant everything protects).

---

## 1. One session, end to end

A "request" in this system is not a message — it is a **25-minute focus session**. Here is the whole
thing, once:

```mermaid
sequenceDiagram
  participant U as User
  participant Bed as Noise engine (native, on-device)
  participant V as Voice loop (native)
  participant FSM as Session state machine (deterministic)
  participant Crew as LLM crew (cheap, cloud)
  U->>Bed: taps orb → session starts
  Bed-->>U: pink/brown bed begins (never stops until §done)
  U->>V: "I need to file my taxes"
  V->>FSM: transcript (cloud STT, streamed partials)
  FSM->>Crew: atomize(task) [async, schema-locked]
  Note over FSM,V: filler ≤300ms if crew slow ("let's break that down…") — never silence
  Crew-->>FSM: {step_1:"open the tax portal", steps_total:6}
  FSM->>V: speak step_1 (bed ducks, then returns)
  loop each step
    FSM->>FSM: start deterministic check-in timer
    U-->>V: (works silently; bed steady)
    alt user quiet past timer OR says "stuck"
      FSM->>V: non-judgmental check-in ("still on the portal?")
    end
    U->>V: "done"
    V->>FSM: intent=step_done
    FSM-->>U: audible win (bed swells briefly)
    FSM->>V: speak next step
  end
  FSM-->>Bed: session complete → bed fades out (the one sanctioned silence)
```

Everything the user *feels* — the bed, the step, the win, the check-in — is driven by the **deterministic
state machine**. The **crew** only supplies words (the step text, the check-in phrasing). That division is
the spine of the whole design ([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).

## 2. The one screen

The entire UI is an **orb** and, when a step is active, **one line of text** (the current step). No list,
no backlog, no dashboard. This is a deliberate ADHD decision, not minimalism for taste: a visible column
of 30 todos is a working-memory and anxiety load that *causes* the avoidance the product exists to fix.
The cost of "no list" is a **trust gap** ("does it still have my stuff?") — Phase 1 pays it down with a
single on-demand read ("what's my step?" → speaks the current step only), and defers the pile entirely
([README §9](README.md)). The orb has exactly three visible states, each a **noise-color + animation**
pair, never a screen change:

| Orb state | Bed | Animation | Meaning |
|---|---|---|---|
| Idle-present | brown, low | slow breathe | "I'm here, not started" |
| Working | brown, steady | faint pulse | "you're heads-down; I'm alongside" |
| Thinking / speaking | pink-ward morph | brighter | "I'm forming your step / talking" |

## 3. The deterministic session state machine (at altitude)

Eight states; every transition is a **rule on a deterministic input** (a timer, an STT intent label, an
OS audio event) — never an LLM's free choice. Full guards/transitions in
[05 §2](05-DETERMINISM-AND-LLM-CONSISTENCY.md); the shape:

```
IDLE_PRESENT ──tap──▶ INTAKE ──transcript──▶ (atomize async) ──▶ STEP_PRESENT
STEP_PRESENT ──spoken──▶ WORKING ──timer_fires|quiet──▶ CHECK_IN ──reply──▶ WORKING
WORKING ──intent:done──▶ STEP_DONE ──win──▶ (more steps? STEP_PRESENT : SESSION_DONE)
any ──OS_audio_interrupt──▶ INTERRUPTED ──OS_resume──▶ (prior state, bed recovered ≤300ms)
SESSION_DONE ──bed fade──▶ IDLE_PRESENT
```

**This FSM is only the *mechanical* register.** It says what the app is doing, not what the *user* is —
because a person is never simply "in CHECK_IN," they are focused **and** tired **and** avoiding, all at
once. That is modelled on **orthogonal cognitive registers** (Goal · Attention · Barrier · Intervention)
over a **belief vector**, in [13](13-COGNITIVE-STATE-AND-POLICY.md). The two compose: this FSM owns the
mechanics and invariants; the policy decides *whether and how* to intervene inside them.

Three design points an interviewer probes:
- **The check-in is a policy decision, not a fixed timer.** `WORKING → CHECK_IN` fires when the
  **intervention policy** ([13 §6](13-COGNITIVE-STATE-AND-POLICY.md)) emits an intervention — after a
  do-no-harm veto (silent while `Focused > 0.9`), a refractory period, a per-session budget, and a
  least-intrusive tie-break. Still fully deterministic (a pure function over beliefs), but now *responsive*:
  it stays quiet through real focus and speaks up on a genuine stall, instead of interrupting on a clock.
- **"Done" is user-declared, not model-inferred.** The orb never decides the user finished a step from
  silence — silence means *working*, and inferring completion from it would be exactly the wrong ADHD
  signal. Completion is an explicit intent (voice "done"/tap), classified by the cheap model but **gated**
  by the state machine (§05).
- **INTERRUPTED is a first-class state, not an error.** A phone call is normal. The bed is paused *by the
  OS*, and recovery is a designed path with a number (≤ 300 ms, [02 §5](02-ALWAYS-ON-AUDIO-ENGINE.md)), not
  a crash.

## 4. Module decomposition — and the RN / native boundary

**The whole app is React Native — we write no custom native module.** The gap- and latency-critical work is
delegated to **off-the-shelf RN libraries that own a C++ audio thread**, and we drive them *declaratively*
from JS. That preserves the structural guarantee (JS is never in the audio render loop, so a GC pause
can't gap the bed — [02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)) without paying the native-code tax that would
undercut RN's reach and a solo dev's velocity:

| Module | Lives in | Why there | Owner file |
|---|---|---|---|
| Orb render, screen, session glue | **RN (JS/TS)** | not latency-critical; RN's reach/velocity is why it was chosen | 01 |
| **Noise engine + mixer + ducking** | **RN + `react-native-audio-api`** (Web Audio; library-owned C++ audio thread) | scheduled graph ⇒ 0 gap budget survives JS stalls, **no native code written** | [02](02-ALWAYS-ON-AUDIO-ENGINE.md) |
| **Voice loop** (mic, VAD, STT, streaming TTS playback) | **RN + audio/speech libraries**, mixed into the same graph | one clock with the bed; barge-in and streaming handled at the graph level | [03](03-VOICE-LATENCY-PIPELINE.md) |
| Session state machine | **On-device TS core** + durable mirror | deterministic, testable in pure TS | [05](05-DETERMINISM-AND-LLM-CONSISTENCY.md) |
| Context/memory pack (RAG) | **On-device in-memory**, warmed at app-open | retrieval without a network hop ([12 §3](12-LLD-AND-SESSION-CONTRACT.md)) | [12](12-LLD-AND-SESSION-CONTRACT.md) |
| Model router + crew calls | **Thin cloud backend** (Workers) + on-device fast path | keep API keys server-side; meter/cap cost centrally | [04](04-MODEL-ROUTER-AND-THE-CREW.md) |
| Durable store (todos, session records) | **Supabase Postgres (free tier)** | cheap, managed, RLS-ready for Phase 2 accounts | [07](07-CAPACITY-AND-LATENCY-MATH.md) §3 |

**The backend is deliberately thin and mostly stateless.** At 20K users it is a request proxy + a cost
meter + a durable mirror — no queue, no GPU, no cluster ([07](07-CAPACITY-AND-LATENCY-MATH.md) §3). The
heavy, always-on work (audio, the mostly-silent 25 minutes, STT/TTS) is on the phone, which is why the
system scales for free and runs offline for everything but the ~15 short brain bursts.

## 5. Sync vs async — what's on the hot path

The **hot path** is exactly one thing: the **voice turn** (user stops speaking → orb starts speaking),
budgeted **p50 ≤ 600 ms / p99 ≤ 1.2 s** ([03 §4](03-VOICE-LATENCY-PIPELINE.md)). Everything else is moved
off it:

| Work | Sync (hot path) | Async / off-path | Mechanism |
|---|---|---|---|
| Endpointing + STT of a user utterance | ✅ | | on-device, streaming |
| Fast conversational reply (turn-taking, acks) | ✅ | | cheap model, capped output |
| **Atomizing the task into steps** | | ✅ | fired at INTAKE; result not needed until STEP_PRESENT; **filler covers it** |
| Next-step pre-fetch | | ✅ | atomize returns all steps once; step N+1 is already in hand |
| Cost metering, telemetry, eval sampling | | ✅ | fire-and-forget to the backend |
| Durable session/todo write | | ✅ | write-behind; on-device state is the truth in-session |

The single most important async move: **atomization overlaps the intake acknowledgement.** The user
finishes "…file my taxes," the orb immediately says a *deterministic* "okay — let's break that down"
(≤ 300 ms, no model needed), and by the time that sentence ends the crew's first step is usually back. The
"keep the conversation going while it thinks" behavior is, mechanically, **latency hiding**
([03 §5](03-VOICE-LATENCY-PIPELINE.md)).

## 6. The hop budget at a glance (authoritative table in [03 §1](03-VOICE-LATENCY-PIPELINE.md))

Where the ~530 ms p50 of a *conversational* turn goes (the common deterministic turn is ~185 ms — [03 §4](03-VOICE-LATENCY-PIPELINE.md)):

| Hop | p50 | Notes |
|---|---|---|
| Semantic endpoint decision | ~150 ms | on streaming partials, not a silence timer ([03 §5](03-VOICE-LATENCY-PIPELINE.md)) |
| STT partials | ~0 ms | already streaming during speech ([03 §3](03-VOICE-LATENCY-PIPELINE.md)) |
| Router decision (deterministic) | ~2 ms | a rule over cached classifier output, not an LLM ([04](04-MODEL-ROUTER-AND-THE-CREW.md)) |
| LLM first sentence | ~200 ms | **already generating** — speculative start on partials ([03 §6](03-VOICE-LATENCY-PIPELINE.md)) |
| TTS first audio | ~0 ms cached / ~120 ms streamed | premium neural (Fish/Sarvam/Cartesia), pre-synth + cached ([03 §2](03-VOICE-LATENCY-PIPELINE.md)) |
| Local mix + duck the bed | ~5 ms | native thread |
| **Conversational-turn p50** | **~530 ms** | semantic endpointing + speculative generation; full budget in [03 §4](03-VOICE-LATENCY-PIPELINE.md) |

## 7. Operator's scars

1. **The JS bridge ate the deadline.** An early prototype ran per-buffer mixing logic *from JS*; a routine
   GC pause (~40–80 ms) produced an audible bed stutter. The fix was **not** writing a native module (the
   first instinct) — it was switching to a **declaratively scheduled** Web Audio graph where JS hands the
   audio thread timestamped instructions and never sits in the render loop
   ([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)). Same guarantee, zero native code.
2. **Inferring "done" from silence** (removed in design): an early state machine advanced steps when the
   user went quiet. For an ADHD user, quiet is *working* — it advanced past steps they were mid-way
   through. Completion must be declared, never inferred from absence of speech (§3).
3. **The filler that wasn't deterministic.** The intake ack was first generated by the model — so when the
   model was slow, the *cover for the model being slow* was also slow. The ack is now a fixed local phrase
   with zero model dependency ([03 §5](03-VOICE-LATENCY-PIPELINE.md)); the thing that hides latency can
   never itself be on the latency path.
4. **Reached for a native module too early.** The first diagnosis of the GC stutter was "RN can't do
   real-time audio; write native." That would have cost weeks and forked the codebase per platform. The
   actual fix was choosing the right *abstraction* (a scheduled Web Audio graph) inside RN — the boundary in
   §4 is that scar encoded. Reach for the abstraction before the platform escape hatch.
5. **The flat voice tested worse than a robotic one.** An early build used a good neural voice with **no
   state-dependent prosody** — every line, from "let's start" to "you finished it," in the same even tone.
   Testers read it as *indifferent*, which is worse than obviously-synthetic: a body double that doesn't
   react feels like it isn't listening. Emotion is now a first-class, deterministically-driven field
   ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)), not a TTS afterthought.

## 8. Interview questions this file answers

- "Walk me through one session end to end." (§1)
- "Why one screen and no list? Isn't that worse UX?" (§2 — the ADHD rationale + the trust cost it pays)
- "Where's the state machine and what does the LLM decide?" (§3 — nothing on the control path)
- "You chose React Native — how do you hit a real-time audio budget on it, with no native module?" (§4 — a scheduled Web Audio graph; JS never in the render loop)
- "What's synchronous vs asynchronous, and how do you hide the model's latency?" (§5)
- "Break down a spoken turn's latency." (§6, and [03 §1](03-VOICE-LATENCY-PIPELINE.md))
