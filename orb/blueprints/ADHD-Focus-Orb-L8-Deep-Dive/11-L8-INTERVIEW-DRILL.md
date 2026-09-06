# 11 — L8 Interview Drill

> **Purpose:** the consolidated question bank — the questions each file arms you to answer, in one place,
> with tight answers, plus the meta-questions that decide whether the whole thing reads as L8 or L6. Drill
> until each answer is a reflex with a number attached. The adversarial "break it" pass is
> [10-CROSS-EXAMINATION.md](10-CROSS-EXAMINATION.md).

---

## System & architecture ([01](01-SYSTEM-OVERVIEW-AND-PLANES.md))

- **Walk me through one session end to end.** Tap → bed on → user speaks one task → streaming cloud STT → the
  deterministic state machine dispatches an async atomize → filler covers → speak step 1 → user works
  (bed steady, check-in timer) → "done" → audible win → next step → … → bed fades (the one silence).
- **Why one screen and no list?** A visible backlog is the anxiety that causes ADHD avoidance; the todos
  live in voice, the screen shows presence + the current step only. Cost: a trust gap, paid down with an
  on-demand "what's my step?" ([01 §2](01-SYSTEM-OVERVIEW-AND-PLANES.md)).
- **You chose React Native — how do you hit a real-time audio budget with no native module?** A
  **declaratively scheduled** Web Audio graph (`react-native-audio-api`) rendered on a library-owned C++
  audio thread: JS hands it timestamped instructions and is never in the render loop, so a GC pause can't
  gap the bed. Risk named: a pre-1.0 dependency, pinned + merge-gated, with a 4-step fallback ladder
  ([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)).

## The audio invariant ([02](02-ALWAYS-ON-AUDIO-ENGINE.md))

- **"Never silent" — how do you *guarantee* it?** Structural: a procedurally-generated 30 s crossfaded
  buffer looping in a **scheduled** graph on the audio thread ⇒ JS is never in the render path, so no code
  path can gap it. Guarantee: **0 gap events/session**, asserted by a watchdog *and* by a recorded-output
  loudness-floor probe (the library's own counter missed a real regression once).
- **When does the bed start?** At **app-open**, not session-start — bed audible ≤ 120 ms, warm greeting
  ≤ 250 ms, both local with no network and no model ([12 §2](12-LLD-AND-SESSION-CONTRACT.md)).
- **Phone call mid-session?** INTERRUPTED state → ≤ 300 ms recovery behind a cover ([02 §5](02-ALWAYS-ON-AUDIO-ENGINE.md)).
- **Why generate, not loop a file?** No loop point ⇒ no seam; ~0 storage; < 0.1 % of a core to generate
  ([02 §2](02-ALWAYS-ON-AUDIO-ENGINE.md)).
- **How do you test it?** Continuous-audio watchdog (0 underruns), the interruption matrix as an automated
  suite, an 8 h gapless soak, a perceptual gap-sensitivity panel ([02 §11](02-ALWAYS-ON-AUDIO-ENGINE.md)).

## Voice latency ([03](03-VOICE-LATENCY-PIPELINE.md))

- **Break down the budget.** Two shapes: deterministic "done→next" ~330 ms (no LLM, cached audio);
  conversational **~530 ms p50 / 1.2 s p99** — semantic endpointing removes the silence wait and speculative
  generation removes the thinking wait ([03 §4–6](03-VOICE-LATENCY-PIPELINE.md)).
- **Sub-second with a cloud LLM in the loop?** Most turns skip the LLM (steps pre-fetched); the rest are
  covered by a deterministic filler ≤ 300 ms ([03 §1/§5](03-VOICE-LATENCY-PIPELINE.md)).
- **Empathetic voice without speech-to-speech cost?** A top neural-TTS pipeline (Fish #1 / Cartesia /
  Sarvam) + fixed-phrase caching + next-step pre-synthesis — warmth at ~1/50th the S2S cost ([03 §4](03-VOICE-LATENCY-PIPELINE.md)).
- **Slow provider, green dashboard?** User-clock measurement, per-shape/per-network histograms, provider-TTFT
  routing ([03 §6](03-VOICE-LATENCY-PIPELINE.md)).

## Driving the LLMs ([04](04-MODEL-ROUTER-AND-THE-CREW.md)) & determinism ([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md))

- **How do you route work to different LLMs?** A deterministic lookup table on (state, intent) — ~65 % of
  turns hit no model; the atomizer is the one heavy async worker; Haiku is the cross-provider escalation.
- **Does an LLM ever decide what the app does?** No — the state machine owns every transition; the LLM
  produces words only (0 control decisions).
- **"Behaves the same" — prove it.** Layered: control path provable (code); output shape always valid
  (schema → validate → repair-once → fail-closed); output content bounded + measured (K=10 × N=200
  consistency harness: count-mode ≥ 90 %, cosine ≥ 0.85) ([05 §4](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
- **Silent provider upgrade?** Pinned versions + a nightly sentinel-task canary + scheduled re-calibration
  ([05 §5](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
- **Why not one speech-to-speech model?** Cost (300–1700×), determinism (a black box can't be schema-gated),
  and voice-identity control ([04 §5](04-MODEL-ROUTER-AND-THE-CREW.md)).

## Modelling the user ([13](13-COGNITIVE-STATE-AND-POLICY.md))

- **Why not one state machine for the user?** A flat cross-product is 3,024 states / ~9.1 M transitions and
  *forces a lie* — you must pick one, so "focused + tired + avoiding" gets flattened. Orthogonal registers:
  **30 values, 230 transitions**, all three representable ([13 §1](13-COGNITIVE-STATE-AND-POLICY.md)).
- **Isn't a belief model at odds with your determinism claim?** No: **probabilistic perception →
  deterministic cognition → deterministic action.** Belief updates are pure log-odds arithmetic (bit-exact
  replay); the policy is a pure function; stochastic classifiers emit only **bounded evidence** and can
  never set a belief or pick an action ([05 §1](05-DETERMINISM-AND-LLM-CONSISTENCY.md), [13 §5.1](13-COGNITIVE-STATE-AND-POLICY.md)).
- **Rules, cosine, or a small LLM for state inference?** All three, tiered: arithmetic for the ~70 %
  non-linguistic evidence (a timer is a number), similarity-match for paraphrased self-report, a small LLM
  only on ambiguity — and it's affordable because **belief updates are off the voice hot path**
  ([13 §5.1](13-COGNITIVE-STATE-AND-POLICY.md)).
- **How do you stop it nagging?** Do-no-harm veto (silent while `Focused > 0.9`), **≥ 90 s refractory**,
  **≤ 6 interventions/session**, hysteresis, and a least-power ladder where silence is the default — all
  counters checked *before* emitting, so nagging is structurally impossible (INV6).
- **What can you actually sense?** Timers, prosody, self-report, session mechanics — **not** screen or
  keyboard. So `Novelty Pull` is barely observable in Phase 1, and beliefs with no strong source are
  **confidence-capped** so the policy can't act decisively on them ([13 §5](13-COGNITIVE-STATE-AND-POLICY.md)).
- **How do you test a probabilistic system?** Bit-exact replay of recorded evidence · **Brier ≤ 0.15** +
  reliability diagram on 5,000 labeled windows · **false-interrupt ≤ 5 % reported *with* missed-help** (or
  you can win by going mute) · shadow policy for counterfactuals ([13 §7](13-COGNITIVE-STATE-AND-POLICY.md)).

## Evals ([06](06-EVALS-AND-TESTING.md))

- **How do you measure a "good" step for ADHD?** The atomicity rubric (single action / < 2 min / no
  sub-decision / done-signal), judge-graded with κ ≥ 0.8, first-step-startable ≥ 99 %, plus the
  session-outcome metric (abandon-after-step-N) as ground truth.
- **Samples to claim ≤ 1 % bad first steps?** Rule of three ⇒ ≥ 300 clean; nightly CI ±1 % at n≈1,000.
- **pass^k?** 90 %/run ⇒ 43 % of users fail within 8 sessions ⇒ per-run bar ≥ 99.4 %.
- **Catch a regression before next week's eval?** Real-time proxies: "that's too big" utterances,
  abandon-rate, repair-rate, fallback-rate — each with an SLO ([06 §8](06-EVALS-AND-TESTING.md)).

## Scale & cost ([07](07-CAPACITY-AND-LATENCY-MATH.md), [08](08-COST-MODEL.md))

- **Size it.** 20K users → 6K DAU → 500 peak concurrent sessions (Little's law) → ~1.7 LLM calls/s peak;
  backend 60K req/day fits the free tier.
- **Where's the queueing tail?** There isn't one — ρ≈0, ~3 orders below any provider ceiling; the tail is
  provider TTFT + network, not our infra ([07 §4](07-CAPACITY-AND-LATENCY-MATH.md)).
- **What does a session cost?** **~₹2.5–3.2** — **voice ~85–90 %** (STT ~55 %, TTS ~25 %), **LLM only ~7 %**,
  relay ~₹0.02; **~₹75–96/user/mo** under a ≤ **₹120** ceiling, and the free tier must be session-capped at
  5/mo or it's upside-down ([08 §1/§1.1/§4](08-COST-MODEL.md)).
- **Self-host or API for voice?** Start API (elasticity), migrate to self-hosted Fish/Kokoro by ~50K users;
  self-host is also the price-risk hedge ([08 §3](08-COST-MODEL.md)).
- **Prove no rewrite to 200K.** Tier swap + one replica + self-hosted TTS; the on-device/cloud boundary and
  the stateless backend don't move ([07 §5](07-CAPACITY-AND-LATENCY-MATH.md)).

## Failure ([09](09-FAILURE-DR-AND-DEGRADATION.md))

- **Safe failure mode?** Present-but-dumb, never absent — presence > intelligence > richness; the bed is
  priority 0 and never sheds.
- **Total cloud outage?** A deterministic body double: bed + cached starter step + timer check-ins — dumb,
  fully present.
- **The one alarm you'd wake for?** The audio-gap / think-time-silence watchdog — the only failure the user
  experiences as the product dying.

---

## The meta-questions (answer these or it isn't L8)

- **What's your weakest link you haven't solved?** The **novelty cliff** — retention past ~week 3. A gapless
  bed and a 99.4 % atomizer make the *session* excellent but don't make the user *return*; the habit loop
  (proactive presence, learned interrupt timing) is deliberately Phase 2 ([09 §5](09-FAILURE-DR-AND-DEGRADATION.md)).
  The second weakest is now narrower but real: the intervention policy is belief-driven rather than a fixed
  timer ([13](13-COGNITIVE-STATE-AND-POLICY.md)), but its **weights are hand-tuned, not learned** — because
  fitting a model before you have labels produces confident nonsense. Until ~100 annotated sessions exist,
  the policy is only as good as my priors. And **without screen sensing, `Novelty Pull` is barely
  observable**, so the "you're about to go scroll" catch — arguably the highest-value intervention — mostly
  can't fire in Phase 1 ([13 §5](13-COGNITIVE-STATE-AND-POLICY.md)).
- **What would you cut under a deadline?** Everything but the three that define the product: the **gapless
  bed** ([02](02-ALWAYS-ON-AUDIO-ENGINE.md)), **one atomic step at a time** ([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md)/[06](06-EVALS-AND-TESTING.md)),
  and a **warm voice that never leaves dead air** ([03](03-VOICE-LATENCY-PIPELINE.md)). Cut cloud STT,
  self-host, Android polish, the premium-max S2S experiment, analytics. Keep presence + one step + no
  silence.
- **Where were you wrong?** (1) Costed the LLM and forgot the voice — the ceiling moved ₹15→₹40 once a
  non-robotic voice became non-negotiable ([08 §1](08-COST-MODEL.md), R5). (2) Claimed "temp-0 ⇒
  deterministic" — falsified in a week; replaced with a *measured* consistency bound ([05 §8](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
  (3) Put the latency-hiding filler *on* the model's path, so a slow model had a slow cover ([03 §8](03-VOICE-LATENCY-PIPELINE.md)).
- **What makes this L8, not L6?** An L6 builds a voice-todo app that calls an LLM and plays a sound. This
  holds the *whole system's invariants* while changing any part: presence is a **structural** 0-gap
  guarantee (not "we play a sound"), "behaves the same" is a **measured** K×N bound (not "temp 0"), the
  control path takes **0 LLM decisions** and the user model is **probabilistic perception → deterministic
  cognition → deterministic action** with bit-exact replay (not "an AI figures out how you're doing"),
  nagging is impossible by counter (not by tuning), a shaming tone is **unrepresentable** (not
  discouraged), the cost ceiling is a **reservation** (not a dashboard), and every headline — 0 ms gap,
  p50 ≤ 600 ms, ₹3/session, 99.4 %/run, Brier ≤ 0.15 — has its derivation and its failure story. And it
  names what it *didn't* solve (retention, learned weights, screen sensing) with the reason, instead of
  pretending.
