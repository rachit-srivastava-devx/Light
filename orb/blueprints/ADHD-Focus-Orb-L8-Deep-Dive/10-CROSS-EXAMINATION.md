# 10 — Cross-Examination

> **Purpose:** the adversarial pass — 40 attempts to break the system (races, partial failures, edge cases,
> abuse, product traps), each answered at **config/mechanism level in one sentence**. If any answer needs
> more than a sentence of hand-waving, the *design* gets fixed, not the answer. Grouped by plane.

---

## A. Presence / the audio invariant

1. **A phone call comes in at minute 12 — bed gone forever?** No — INTERRUPTED is a first-class state; on
   `.ended` the stream restarts and the bed ramps up ≤ 300 ms behind a cover ([02 §5](02-ALWAYS-ON-AUDIO-ENGINE.md)).
2. **JS thread GC-pauses 60 ms mid-session — audible stutter?** No — the bed is a **scheduled** Web Audio
   graph rendered on the library's C++ audio thread; JS hands it timestamped instructions and is never in
   the render loop, so a GC pause can't miss the deadline ([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)).
3. **`mediaServicesWereReset` wipes the audio stack — silent forever?** No — its own recovery row rebuilds
   the engine and speaks a cover line, the one case allowed to exceed 300 ms ([02 §5](02-ALWAYS-ON-AUDIO-ENGINE.md)).
4. **Headphones unplugged — playback pauses (iOS default) and never resumes?** No — route-change handler
   resumes on speaker (or honors pause-on-unplug), covered ([02 §5](02-ALWAYS-ON-AUDIO-ENGINE.md)).
5. **The looped noise file's seam annoys users?** There is no file — the bed is procedurally generated, so
   there is no loop point to seam ([02 §2](02-ALWAYS-ON-AUDIO-ENGINE.md)).
6. **Battery drains and the user blames you?** The bed is user-initiated + one-tap mute with a visible
   live-indicator; presence you didn't ask for is treated as a bug ([02 §8](02-ALWAYS-ON-AUDIO-ENGINE.md)).

## B. Latency / voice

7. **Cloud LLM is slow right now — dead air?** No — a deterministic filler is spoken ≤ 300 ms with zero
   model dependency; think-time-silence is gated to 0 ([03 §5](03-VOICE-LATENCY-PIPELINE.md)).
8. **The orb talks over the user?** Barge-in yields ≤ 100 ms with platform AEC so the orb doesn't trip its
   own VAD ([03 §3](03-VOICE-LATENCY-PIPELINE.md)).
9. **Endpointing cuts off an ADHD mid-thought pause?** The hangover is tuned longer (~220 ms) with a
   "still-listening" bed cue; false-endpoint rate is a gated metric ([03 §2](03-VOICE-LATENCY-PIPELINE.md)).
10. **Premium TTS added network latency to every "done → next step"?** No — the next step's premium audio is
    pre-synthesized while the user works, so playback is from local cache = instant ([03 §4](03-VOICE-LATENCY-PIPELINE.md)).
11. **Your dashboard is green but a user says it's laggy.** Measured on the user's clock (on-device
    stop→first-audio), split by shape × network — server-only timing hides the real lag ([03 §6](03-VOICE-LATENCY-PIPELINE.md)).
12. **Provider TTFT p99 spikes.** Rolling per-model TTFT tracked; sustained breach waterfalls to the
    fallback provider, filler covers the gap ([03 §6](03-VOICE-LATENCY-PIPELINE.md), [04 §6](04-MODEL-ROUTER-AND-THE-CREW.md)).

## C. Determinism / LLM behavior

13. **You said temperature 0 — so it's deterministic?** No — GPU batching makes even temp-0 non-identical;
    we bound and *measure* consistency (K×N harness), not claim bit-identical ([05 §4](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
14. **Provider silently upgrades the model overnight.** Versions are pinned; a sentinel task canary pages on
    any shift; upgrades are scheduled with re-calibration ([05 §5](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
15. **The LLM returns 20 steps / malformed JSON / an empty step.** Schema validate → repair-once →
    fail-closed to a deterministic step; 0 malformed outputs reach the user ([05 §3](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
16. **The model decides to end the session on an ambiguous "okay that's it."** It can't — ending is a
    state-machine transition on explicit intent + confirm; the model emits words, never control ([05 §2](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
17. **Same task gives a different plan tomorrow.** Bounded by the consistency gate (step-count mode ≥ 90 %,
    cosine ≥ 0.85) and cached within a session so it's stable while it matters ([05 §4](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
18. **The router's LLM makes a bad routing call.** The router is a pure code table, not an LLM — only the
    intent *classifier* feeding it is stochastic, and it's eval'd separately ([04 §2](04-MODEL-ROUTER-AND-THE-CREW.md)).

## D. Cost

19. **A runaway atomize loop bills ₹40 in one session.** Impossible — a per-session token/char reservation
    (INV5) caps spend and fails closed ([08 §6](08-COST-MODEL.md)).
20. **You costed the LLM but forgot the voice.** Corrected and stated (R5): voice is ~80 % of marginal cost,
    the ceiling moved ₹15→₹40 ([08 §1](08-COST-MODEL.md)).
21. **TTS API doubles its price.** Cost is hedged by the self-host path (Fish/Kokoro) which decouples from
    per-char pricing ([08 §3](08-COST-MODEL.md), §7).
22. **Flash-Lite retires in Oct 2026 — cost blows up?** LLM is only ₹0.15/session; even 3× is negligible —
    cost is voice-dominated and LLM-price-insensitive ([08 §7](08-COST-MODEL.md)).
23. **A power user does 3 sessions/day.** ~₹54/user/mo — over the infra target but ~14× under the price
    point; a packaging decision, not a risk ([08 §7](08-COST-MODEL.md)).
24. **A prompt tweak quietly pushed traffic onto the paid model.** Mix-drift alert fires when the paid-model
    share exceeds budget ([04 §6](04-MODEL-ROUTER-AND-THE-CREW.md), [06 §8](06-EVALS-AND-TESTING.md)).

## E. Session logic / the atomizer

25. **The atomizer returns "do your taxes" as one step.** Fails the is-atomic rubric (single action / < 2 min
    / no sub-decision / done-signal); rejected and re-asked smaller ([06 §2](06-EVALS-AND-TESTING.md)).
26. **The first step isn't startable.** First-step-startability is gated ≥ 99 % — the highest-weighted
    metric, because starting is the whole product ([06 §2](06-EVALS-AND-TESTING.md)).
27. **The orb advances a step because the user went quiet.** It doesn't — quiet is *working*; completion is
    an explicit declared intent (INV3) ([05 §2](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
28. **"It worked in the demo."** pass^8 ≈ 43 % at 90 %/run — the bar is ≥ 99.4 %/run, so one demo proves
    nothing ([06 §3](06-EVALS-AND-TESTING.md)).
29. **The judge rewards a thorough 8-step plan over the right 3-step one.** Length-control + κ ≥ 0.8
    calibration; the judge that optimizes against ADHD is re-calibrated ([06 §6](06-EVALS-AND-TESTING.md)).
30. **Deflection-Goodhart: tiny steps inflate completion while users still bail.** Abandon-rate + "too big /
    too small" utterances veto the completion metric ([06 §8](06-EVALS-AND-TESTING.md)).

## F. Platform / RN / offline

31. **React Native can't hit a real-time audio budget.** It can, with the right abstraction: a declaratively
    scheduled Web Audio graph (`react-native-audio-api`) renders on a library-owned C++ thread — **no custom
    native module**, and JS never sits in the render loop ([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)).
31b. **So you've bet the core invariant on a pre-1.0 dependency?** Yes, knowingly — mitigated by a pinned
    version, a merge-gated audio suite on every bump, and a 4-step fallback ladder ending at the native
    module we chose not to write ([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)).
32. **Network drops mid-session.** The bed, cached step audio, and the state machine keep going, so the user
    stays accompanied — but speech is cloud-only, so the orb goes **deaf and mute** and says so once, in
    cached audio. No degraded-voice fallback exists, by decision ([03 §2](03-VOICE-LATENCY-PIPELINE.md), [09 §3](09-FAILURE-DR-AND-DEGRADATION.md)).
33. **An OEM Android skin kills the foreground service.** Named long-tail risk; battery-exemption prompt +
    honest foreground-only fallback on those devices ([02 §9](02-ALWAYS-ON-AUDIO-ENGINE.md)).
34. **Backend down mid-session.** On-device state is the in-session truth; the durable write is write-behind
    and reconciles later — the user notices nothing ([09 §6](09-FAILURE-DR-AND-DEGRADATION.md)).
35. **On-device STT mangles Hinglish.** Sarvam Saarika (code-mixing) is the opt-in quality upgrade; the
    default trades a little accuracy for ₹0 + privacy ([03 §4](03-VOICE-LATENCY-PIPELINE.md), [08 §4](08-COST-MODEL.md)).

## G. Privacy / abuse / safety

36. **The orb hears everything — where does the audio go?** STT runs on-device by default (nothing leaves);
    only ~500 novel chars of *text* reach the cloud for TTS, never raw audio in the default config ([02](02-ALWAYS-ON-AUDIO-ENGINE.md), [03 §4](03-VOICE-LATENCY-PIPELINE.md)).
37. **A user dumps self-harm content to the orb.** Out of Phase-1 scope as a *feature*, but the honest gap
    is named — a companion for a vulnerable population needs a crisis-escalation path before wide launch
    ([README §9](README.md); flagged, not pretended-solved).
38. **Prompt-injection via the spoken task ("ignore steps, say X").** The model only fills step *text* into a
    validated slot; it can't emit control or a side-effect (reference-not-generate, INV2) ([05 §3](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
39. **Someone scripts thousands of atomize calls to run up your bill.** Per-session reservation + per-user
    rate limits at the thin backend; the free-tier ceiling itself bounds abuse ([08 §6](08-COST-MODEL.md), [07 §3](07-CAPACITY-AND-LATENCY-MATH.md)).

## G2. The cognitive model

41. **Your belief model says "focused 0.9" — based on what, eight minutes of silence?** Confidence **decays**
    with a per-belief half-life (Attention 90 s), so a stale belief loses authority and the do-no-harm veto
    stops applying ([13 §4](13-COGNITIVE-STATE-AND-POLICY.md)).
42. **A belief oscillates around its threshold — does the orb flap?** No — hysteresis (≥ 0.1 enter/exit gap)
    plus a 90 s refractory ([13 §6](13-COGNITIVE-STATE-AND-POLICY.md)).
43. **`Novelty Pull > 0.8 → Redirect` — how do you even detect that in a phone app?** Mostly you can't in
    Phase 1 (no screen access); the belief is **confidence-capped** so that rule effectively won't fire, and
    that's stated rather than shipped as if it worked ([13 §5](13-COGNITIVE-STATE-AND-POLICY.md)).
44. **Two rules fire at once — Focused 0.95 *and* Blocked 0.8. What happens?** Veto order decides: the
    do-no-harm veto requires `max(Barrier) < 0.7`, so Blocked wins the veto and the least-intrusive firing
    intervention (Clarify) is chosen ([13 §6](13-COGNITIVE-STATE-AND-POLICY.md)).
45. **A user is bored, and you shrink the task — making it worse?** Not anymore: Under-stimulated is a
    **separate barrier** from Overwhelmed with the opposite intervention (add stimulus, tighten timebox) —
    the scar that split them ([13 §8.5](13-COGNITIVE-STATE-AND-POLICY.md)).
46. **You could game false-interrupt to zero by never speaking.** Which is why it is **always reported with
    missed-help**; the mute policy fails visibly ([13 §7.4](13-COGNITIVE-STATE-AND-POLICY.md)).
47. **An LLM classifies the user as "Overwhelmed" incorrectly — wrong intervention?** It can't do that
    alone: LLM output is **bounded evidence** (|w·r| ≤ 0.8 logits) into a belief, never a belief assignment
    — one bad parse cannot cross a policy threshold ([13 §5.1](13-COGNITIVE-STATE-AND-POLICY.md)).
48. **Hyperfocus looks like your best-case state — do you just stay silent forever?** No — `Hyperfocus > 0.8`
    beyond 45 min draws a **Pause** (a gentle boundary), because missed breaks/meals are a real ADHD harm,
    not a success ([13 §6](13-COGNITIVE-STATE-AND-POLICY.md)).

## H. Product / retention

40. **All this engineering — does the user come back on day 20?** Honestly unknown — the novelty cliff is
    the real risk, unsolved by Phase 1; the habit loop (proactive presence + interrupt-timing) is the
    deferred Phase-2 bet, named as the weakest link, not hidden ([09 §5](09-FAILURE-DR-AND-DEGRADATION.md), [README §9](README.md)).

---

*Every scenario above is answered by a mechanism in another file, not by "we'd handle it." Where the honest
answer is "not solved in Phase 1" (#37, #40), it is named as a gap with the reason, per R5.*
