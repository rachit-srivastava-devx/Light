# 13 — Cognitive State, the Belief Model & the Intervention Policy

> **Purpose:** how the orb models the *user* rather than just the session — **orthogonal state registers**
> (a person can be focused **and** fatigued **and** avoiding at once), a **belief vector** with confidence
> that decays, and a **deterministic policy** that turns beliefs into the least-intrusive useful action.
> This file replaces the flat lifecycle FSM as the *user model*; the mechanical session FSM survives as one
> register ([05 §2](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
>
> **The one-line thesis:** *probabilistic perception → deterministic cognition → deterministic action.* The
> beliefs are estimates; the policy over them is a pure function. **No LLM appears anywhere in this file's
> decision path.**

---

## 1. Why orthogonal registers (the combinatorial argument)

A flat FSM that enumerates every combination of goal, attention, barrier, and intervention needs
**6 × 7 × 8 × 9 = 3,024 states** and up to **~9.1 M** transitions — unbuildable, untestable, and full of
states no human is ever in. Worse, it *forces a lie*: you must pick one, so a user who is genuinely
**focused, tired, and quietly avoiding the hard part** gets flattened into whichever the code checked first.

Modelling them as **independent registers** costs **6 + 7 + 8 + 9 = 30 values**, and each register's
transitions are testable exhaustively (**36 + 49 + 64 + 81 = 230** intra-register transitions). The
combination is *represented* without being *enumerated* — which is exactly why impossible transitions stop
existing: there is no edge from "Overwhelmed" to "Focused" to get wrong, because they live on different
axes and move independently.

| | Flat cross-product | **Orthogonal registers** |
|---|---|---|
| States | 3,024 | **30 values across 5 registers** |
| Transitions to specify/test | ~9.1 M | **230** |
| Can express "focused + fatigued + avoiding" | ❌ | ✅ |
| Adding a barrier type | ×8 state explosion | **+1 value, +15 transitions** |

## 2. The five registers

**Register 0 — Session** *(mechanical; the existing FSM, unchanged)* — `IDLE_PRESENT · INTAKE · CLARIFY ·
STEP_PRESENT · WORKING · CHECK_IN · STEP_DONE · INTERRUPTED · SESSION_DONE`. This is *what the app is doing*,
owns invariants INV1–INV5, and stays fully deterministic ([05 §2](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
Registers 1–3 model *the user*; register 4 is *what we choose to do about it*.

**Register 1 — Goal** *(mutually exclusive)*
`No Goal · Goal Formation · **Goal Renegotiation** · Goal Commitment · Goal Maintenance · Goal Completion ·
Goal Decay`

> **Added: Goal Renegotiation** — mid-session resizing ("actually, let's just do the first bit"). ADHD
> sessions routinely need the goal *shrunk* in flight, and that is categorically different from forming a
> new goal or abandoning one. Without it, a healthy scope-cut is misread as Goal Decay and triggers the
> wrong intervention (a rescue, when the user is actually self-correcting well).

**Register 2 — Attention** *(mutually exclusive)*
`**Initiating** · Focused · Exploring · Mind Wandering · Hyperfocus · External Interruption · Recovering`

> **Added: Initiating** — the pre-engagement ramp: trying to start, not yet in. This is *the* ADHD moment and
> the product's entire reason to exist, and its policy is the **inverse** of Focused: maximum support during
> Initiating, maximum silence during Focused. Collapsing it into "Exploring" loses the one state where
> intervention is almost always right.

**Register 3 — Barrier** *(**multi-label** — several active at once, each with its own probability)*
`Blocked · Uncertain · Overwhelmed · **Under-stimulated** · Avoiding · Waiting · Fatigued · Emotionally
Dysregulated`

> **This register is a vector, not a value** — the direct consequence of your point. Fatigued(0.7) +
> Avoiding(0.6) + Uncertain(0.4) is a normal, representable Tuesday.
>
> **Added: Under-stimulated** — boredom-driven stall. It is the *opposite* barrier to Overwhelmed and needs
> the opposite intervention: Overwhelmed → shrink the step; Under-stimulated → add stimulus (raise the bed,
> gamify, tighten the timebox, body-double harder). Treating both as "stuck" and shrinking the task makes
> boredom **worse** — a genuinely ADHD-specific failure that a generic productivity model gets wrong.

**Register 4 — Intervention** *(what the AI does; ordered by intrusiveness)*

| # | Intervention | What it is | Intrusiveness |
|---|---|---|---|
| 0 | **Observe** | do nothing, keep listening | none |
| 1 | **Presence** | non-verbal "still here" — a bed swell, no words | ~none |
| 2 | **Capture** | silently record something the user said | none to the user |
| 3 | **Celebrate** | mark a win, audibly | low, positive |
| 4 | **Clarify** | ask one bounded question | low |
| 5 | **Suggest** | offer the next atomic step / a smaller step | medium |
| 6 | **Redirect** | name the drift, steer back | high |
| 7 | **Pause** | propose a break (fatigue/dysregulation) | high |
| 8 | **Escalate** | stop the session, address the human state | highest |

> **Added: Presence** — the lowest-cost intervention above silence, and the product's actual core (body
> doubling *is* presence). Without it the policy has nothing between "say nothing" and "say something,"
> so it over-talks.
> **Added: Celebrate** — the dopamine event. Folding it into Suggest loses the distinct prosody
> ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)) and the reinforcement that makes the loop work.

## 3. Illegal combinations (orthogonality still needs guards)

Independent axes do not mean *every* combination is legal. A small invariant table is enforced in code, and
a violation is a bug, not a state:

| Illegal | Why | Enforcement |
|---|---|---|
| `Goal = No Goal` ∧ `Intervention = Redirect` | redirect toward *what*? | policy guard: Redirect requires `Goal ∈ {Commitment, Maintenance}` |
| `Session = IDLE_PRESENT` ∧ `Attention = Hyperfocus` | not in a session | register coupling assertion |
| `Barrier.Emotionally-Dysregulated > 0.8` ∧ `Intervention ∈ {Redirect, Suggest}` | pushing a dysregulated user is harm | **hard veto** (§6), same class as the prosody allowed-set ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)) |
| `Goal = Completion` ∧ `Intervention = Clarify` | the work is done | policy guard |

Note what is **legal and important**: `Attention = Hyperfocus` ∧ `Barrier.Avoiding = high` — hyperfocusing
on the *wrong* task is textbook productive procrastination, and the model must be able to say it.

## 4. The belief model

Nine beliefs, each stored as **`(value, confidence, last_evidence_ts)`** — 9 × 3 floats ≈ **108 bytes**,
held on-device:

`Goal · Context · Attention · Execution · Working Memory · Energy · Emotional Load · Trust · Novelty Pull`

**Update rule (deterministic, no randomness, no model call).** Evidence accumulates in **log-odds**
(naive-Bayes-style), which keeps updates bounded and composable:

```
on evidence e:                                   # e = (belief, weight w, reliability r)
    logit(b) ← logit(b) + w · r
    conf     ← min(1, conf + κ·r)                # κ ≈ 0.4

on elapsed Δt with no evidence:                  # staleness must cost confidence
    conf  ← conf · 2^(−Δt / T½)                  # T½ per belief (Attention 90 s, Energy 15 min)
    logit(b) ← logit(b) + (logit(prior) − logit(b)) · (1 − 2^(−Δt / T_rev))
```

- **Cost:** ~50 FLOPs per evidence event at ≤ ~2 events/s ⇒ **~0 ms, ₹0, on-device.** The entire cognitive
  layer is arithmetic — no inference server, no model call, no network.
- **Deterministic by construction:** the same evidence sequence yields a **bit-identical** belief
  trajectory, which is what makes §7's replay test possible. This is a probabilistic *model*, not a
  probabilistic *system*.
- **Honest limitation:** log-odds accumulation assumes conditional independence between evidence sources,
  which is false (a long pause raises *both* "mind wandering" and "blocked"). Phase 1 accepts the bias and
  compensates with conservative weights; the **trigger to upgrade** to a fitted model (or a small learned
  classifier) is *having labels* — ~100 annotated sessions (§7), not before. Fitting a model with no data is
  how you get confident nonsense.

**Why confidence decay is not optional:** without it, a belief formed 8 minutes ago still reads
`Focused = 0.9` and the policy confidently stays silent through a total stall. **Confidence is what
separates "I know they're focused" from "I haven't heard anything in a while,"** and those demand opposite
actions (scar §8.2).

## 5. What we can actually sense in Phase 1 (the observability audit — R5)

A belief is only as real as its evidence. Phase 1 is a **foreground, session-scoped voice app**, so:

| Evidence | Source | Available? | Feeds |
|---|---|---|---|
| Time since last utterance / on step | timers | ✅ strong | Execution, Attention |
| Speech rate, pause length, energy, disfluency | prosody off the mic we already run | ✅ good | Attention, Energy, Emotional Load |
| Explicit self-report ("I'm stuck", "I'm tired") | intent classifier | ✅ **strongest** | every belief |
| Step completion vs estimate; re-ask/stuck counts | session mechanics | ✅ strong | Execution, Working Memory |
| Barge-in frequency, interruption events | audio graph | ✅ | Attention, Emotional Load |
| Session length vs this user's typical | history pack ([12 §3](12-LLD-AND-SESSION-CONTRACT.md)) | ✅ | Energy, Trust |
| **App backgrounded / refocused, and time away** | OS lifecycle → the pause event ([02 §5.1](02-ALWAYS-ON-AUDIO-ENGINE.md)) | ✅ **strong** | **Novelty Pull**, Attention (External Interruption), Goal Decay |
| **Which app they switched to** | — | ❌ not visible | — |
| **Keyboard / typing activity** | — | ❌ | — |
| Time-of-day / calendar | OS | ⚠️ deferred with scheduling ([README §9.3](README.md)) | Energy |

**The privacy decision paid an observability dividend.** Pausing on backgrounding
([02 §5.1](02-ALWAYS-ON-AUDIO-ENGINE.md)) was made for trust — but it also hands us the one drift signal we
*can* legitimately see: **the user left, and for how long.** We can't see *what* they left for (no screen
access), but "left the app 40 s into a step, came back after 6 minutes" is strong evidence of exactly the
drift the product exists to catch. It is honestly observed (the user knows the app noticed — the pause was
visible), which is the right way to earn a signal like this.

**So `Novelty Pull` is upgraded from *barely observable* to *partially observable*:** repeated
short-departure patterns raise it with real confidence; the *pre*-drift moment (about to leave, hasn't yet)
remains unobservable until Screen Time / DeviceActivity integration lands in Phase 2+. Catching the return
is worth a great deal on its own — the re-anchor after a drift is one of the highest-value interventions
the orb has.

**The consequence that must be stated plainly: `Novelty Pull` is barely observable in Phase 1.** Detecting
"they're about to go scroll" essentially requires screen access we don't have. We can infer weak signals
(restless prosody, self-report), so the belief exists — but its **confidence will rarely clear the bar**,
which means the example rule `NoveltyPull > 0.8 → Redirect` will **almost never fire in Phase 1, by design**.
Shipping it as if it worked would be the exact "confident nonsense" failure the audit exists to prevent. It
becomes real when Screen Time / DeviceActivity integration lands (Phase 2+).

**Rule enforced in code:** every belief declares its evidence sources; a belief with **no strong source**
cannot exceed a capped confidence (0.5), so the policy structurally cannot act decisively on a belief we
have no way to know. Unobservable beliefs are *represented*, never *trusted*.

**The return-from-away path** is its own high-value moment: on resume after a pause, the policy has fresh,
strong evidence (duration away, where in the step they left) and the refractory clock has effectively
reset — so a **re-anchor** ("you were opening the tax portal — still there?") is usually the right call and
lands exactly when the user needs it. Time away is graded, not binary: **< 30 s** is a glance and gets
nothing; **30 s–5 min** gets a warm re-anchor; **> 5 min** re-confirms the goal is still the goal (Goal
Decay risk), never with a scolding register ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)).

## 5.1 Turning language into evidence: rules vs cosine vs a small LLM

The evidence in §5 splits into two kinds that deserve **different machinery** — conflating them is how this
layer goes wrong in both directions:

| Evidence kind | Examples | Right tool | Why |
|---|---|---|---|
| **Non-linguistic (~70 % of signal)** | silence duration, time-on-step, speech rate, pause length, barge-ins, stuck counts | **arithmetic + thresholds** | "silent for 4 min" *is a number*. An LLM adds latency, cost, and variance to a subtraction. Embedding it is meaningless. |
| **Linguistic (~30 %, but the highest-value)** | "I've been staring at this for ten minutes and I keep opening Twitter" | **cosine, then a small LLM** | keyword rules catch none of this; it carries Avoiding + Novelty Pull + frustration + a scope problem in one sentence |

**The gap this exposes (R5):** the intent classifier specified in [04 §3](04-MODEL-ROUTER-AND-THE-CREW.md) has
**6 labels** (done/next/stuck/pause/question/chit-chat). §5 calls self-report the *strongest* evidence source
— yet 6 coarse labels cannot populate **9 beliefs × 8 barrier dimensions**. So the answer to "would
cosine/a small LLM make this better?" is **yes, and specifically here** — the language path was the weakest
part of the model.

**The decision: a three-tier cascade** (the same least-power discipline as the model router):

| Tier | Mechanism | Latency | Cost | Deterministic? | Handles |
|---|---|---|---|---|---|
| **0** | rules/arithmetic over non-linguistic evidence — **always runs** | ~0 ms | ₹0 | ✅ bit-exact | timers, prosody, mechanics |
| **1** | **similarity match** against a curated **exemplar bank** (~150 labeled utterances → belief deltas), every utterance | ~1–5 ms on-device (lexical) · ~0 extra hop (server embedding) | ₹0 | ✅ | paraphrased self-report: "I'm knackered" · "no idea where to start" · "this is way too much" |
| **2** | **small LLM**, schema-locked multi-label — **only** when Tier-1's top-2 margin < 0.15, or the utterance is long/compositional (~10–20 % of utterances) | ~300 ms, **off the hot path** | ~₹0.02/call | ⚠️ bounded (below) | negation, sarcasm, compositional states, novel phrasings |

**Why the LLM tier is affordable here even though I rejected it elsewhere:** *belief updates are not on the
voice hot path.* The user speaks → the orb answers on the existing fast path
([03 §1](03-VOICE-LATENCY-PIPELINE.md)); the state inference runs **asynchronously** and only has to land
before the next policy tick (≥ 90 s away, §6). A 300 ms call that blocks nothing costs no SLO — this is the
rare place where an LLM is genuinely free of latency risk.

**Cosine vs LLM, honestly:** cosine is deterministic, offline-capable, and free, but weak on **negation**
("I'm *not* stuck"), sarcasm, and multi-label compositional utterances. The LLM handles exactly those, at
the price of variance. Hence: **cosine first, LLM only on ambiguity** — the ~80 % of utterances that are
short self-reports never touch a model.

**Three structural protections so Tier 2 cannot break the determinism guarantee:**
1. **Bounded influence.** LLM output is *evidence*, never a belief assignment and never an intervention. It
   enters through the same clamped log-odds update (§4) with a **cap on total contribution per event**
   (|w·r| ≤ 0.8 logits), so a wrong classification can only *nudge* a belief — it can never jump one across
   a policy threshold on its own. Compare: a design where the LLM sets `Barrier = Overwhelmed` directly is
   one bad parse away from a wrong intervention.
2. **Classification cache.** Keyed by normalized-utterance hash → the same sentence always yields the same
   evidence, within and across sessions. Repeats are exactly deterministic and free.
3. **Replay stays bit-exact** because §7.1 records the **evidence stream**, not the raw audio — you replay
   what the classifier emitted, so the belief trajectory remains perfectly reproducible even with a
   stochastic component upstream. *Determinism of the system under test is preserved by where the recording
   boundary sits.*

**Does it actually make it better? The experiment that decides, not an assertion.** All three tiers are
graded on the same 5,000-window labeled corpus (§7.3), ablation-style — Tier 0 alone → +Tier 1 → +Tier 2 —
scored on **Brier + false-interrupt rate**. Ship the cheapest tier whose Brier gain is **≥ 0.02** over the
tier below; a tier that doesn't clear that bar is deleted, not kept for sophistication (§8.4). My *prior* is
that Tier 1 pays for itself easily (paraphrase coverage is the current hole) and Tier 2 earns its place only
on long/compositional utterances — but the corpus decides that, not this paragraph.

## 6. The policy (deterministic, least-power, do-no-harm)

`decide(beliefs, session, history) → intervention` is a **pure function** — same inputs, same output,
always, and unit-testable exhaustively. The order matters more than the rules:

```
1. HARD VETOES  (evaluated first, in order)
   a. Barrier.Dysregulated > 0.8   → Pause|Escalate ONLY        # never push a dysregulated user
   b. Session ∈ {INTAKE, CLARIFY}  → yield to session mechanics
   c. refractory active            → Observe   (unless severity = CRITICAL)
   d. intervention budget spent    → Observe   (unless severity = CRITICAL)
   e. Attention.Focused > 0.9 AND conf > 0.6 AND max(Barrier) < 0.7
                                   → Observe   ← THE DO-NO-HARM VETO

2. CANDIDATE GUARDS  (all evaluated; each may fire)
   Attention.Initiating           > 0.6            → Suggest (smallest possible step)
   WorkingMemory                  < 0.3            → Suggest (re-state ONE action, drop context)
   Barrier.Overwhelmed            > 0.7            → Suggest (shrink the step)
   Barrier.Under-stimulated       > 0.7            → Presence+ (raise bed, tighten timebox)  ← opposite fix
   Barrier.Blocked                > 0.7            → Clarify (one question)
   Barrier.Uncertain              > 0.7            → Clarify
   Execution stalled > 3 min AND conf > 0.5        → Clarify
   Barrier.Fatigued               > 0.7            → Pause
   Attention.MindWandering        > 0.7            → Redirect
   Attention.Hyperfocus > 0.8 AND elapsed > 45 min → Pause (gentle boundary, not redirect)
   Goal.Decay                     > 0.6            → Clarify ("are we still on this?")
   Step completed                                  → Celebrate
   nothing fires, session live                     → Presence or Observe

3. RESOLVE: choose the LEAST intrusive intervention that fired      # least-power ladder, §2
4. COMMIT: start refractory, decrement budget, log (belief snapshot, rule id, chosen action)
```

**The four guards that keep it from becoming a nag** — this is where a belief-driven agent usually fails:

| Guard | Value | Why |
|---|---|---|
| **Refractory period** | **90 s** min between interventions; **240 s** while `Focused > 0.7` | prevents pile-on |
| **Intervention budget** | **≤ 6 per 25-min session** (step transitions don't count — they're mechanics) | structural anti-nagging; can't be exceeded, not merely discouraged |
| **Hysteresis** | enter/exit thresholds differ by **≥ 0.1** (Schmitt trigger) | a belief oscillating at 0.7 must not flap the intervention (scar §8.1) |
| **Least-power tie-break** | lowest intrusiveness wins | the ladder in §2; silence is the default, not the fallback |

**The asymmetric cost that shapes every threshold:** interrupting a focused ADHD user is far worse than
missing a chance to help — re-entering focus costs minutes, a missed nudge costs nothing. Phase 1 assumes a
**~5:1 cost ratio (false-interrupt : missed-help)** — stated as the tunable assumption it is, and it is why
the do-no-harm veto sits above every candidate rule.

**Intervention → prosody:** the chosen intervention selects the emotional register
([12 §5](12-LLD-AND-SESSION-CONTRACT.md)) — Celebrate → celebratory, Clarify → curious, Pause/Escalate →
calm-never-stern, Redirect → gentle. The dysregulated/stuck allowed-set still excludes every shame-adjacent
tone, so even a maximum-intrusiveness intervention cannot *sound* punitive.

## 7. How to test a probabilistic system (exactly)

1. **Deterministic replay (the foundation):** record every session's evidence stream; replay it and assert
   a **bit-identical belief trajectory**. Because §4 is pure arithmetic, this is a normal unit test — the
   belief model is *fully* testable despite being probabilistic. **Gate: 100 % reproducible.**
2. **Policy unit tests:** exhaustive over the guard table + veto order, including every illegal combination
   in §3. **Gate: 100 % — it's a pure function.**
3. **Calibration against a labeled corpus:** **100 sessions**, human-annotated in **30 s windows**
   (≈ **5,000 labeled windows**), each labeled with ground-truth attention/barrier. Measure per-belief
   **Brier score** and plot a **reliability diagram**: *when the model says 0.8 focused, is the user focused
   ~80 % of the time?* **Gate: Brier ≤ 0.15 and no reliability bin off by > 0.15.** An uncalibrated belief is
   worse than no belief, because the policy's thresholds are meaningless without it.
4. **False-interrupt rate (the metric that matters most):** fraction of interventions fired while
   ground-truth = Focused. **Gate: ≤ 5 %.** Paired with **missed-help rate** (stalled ≥ 5 min, no
   intervention) so the policy can't win by going mute — **the two are reported together, always.**
5. **Shadow policy (honest counterfactuals):** you can never observe what would have happened had you not
   intervened. Mitigation: run candidate policies in **shadow** — compute the intervention, log it, **don't
   fire it** — and compare intervention *distributions* before promoting. Any policy change ships shadow →
   canary → rollback, like a model change ([06 §8](06-EVALS-AND-TESTING.md)).
6. **Evidence ablation:** drop each evidence source and re-measure Brier + false-interrupt. A source that
   moves nothing is deleted — this is what prevents a belief vector of impressive-looking numbers that no
   signal actually drives (scar §8.4).
7. **Nag simulation:** replay the worst-case session (user silent 25 min) and assert the intervention budget
   and refractory hold — **≤ 6 interventions, none closer than 90 s.**

## 8. Operator's scars

1. **Belief flapping produced a chatty orb.** `Blocked` hovering at 0.70 crossed the threshold repeatedly and
   the orb asked essentially the same question four times in two minutes. Fixed with hysteresis + refractory
   (§6). Any threshold on a continuous belief needs a Schmitt trigger, or you've built an oscillator.
2. **Confident on stale evidence.** The model held `Focused = 0.9` for eight minutes after the user had
   silently gone to make tea, so the do-no-harm veto kept it mute through a total stall — the model was
   *confident about the past*. Confidence decay (§4) exists because of that session; silence is ambiguous
   evidence, and its meaning must fade.
3. **Tried the flat cross-product first.** The enumerated FSM hit ~3,000 states and was abandoned mid-build:
   most states were unreachable, several were nonsense, and no one could review the transition table. The
   orthogonal split (§1) was the fix, and it also made "focused + fatigued + avoiding" expressible for the
   first time.
4. **Novelty Pull was theatre.** The belief existed, the rule fired, the dashboard looked sophisticated —
   and the only real input was a weak prosody heuristic. It fired on noise and redirected focused users. Now
   every belief must declare its evidence sources and is **confidence-capped when it has none** (§5). A
   belief you can't sense is a lie with a number attached.
5. **Shrank the task for a bored user.** Under-stimulated was originally folded into Overwhelmed, so the
   policy shrank an already-trivial step — which made the boredom worse and the user quit. Splitting the two
   barriers (§2) reversed the intervention, and it's the clearest example of why an ADHD model can't be a
   generic productivity model.
6. **Optimized false-interrupts to zero by doing nothing.** An early tuning pass drove false-interrupts to
   ~0 — by making the orb almost never speak, which tested as "it stopped caring." The paired
   missed-help metric (§7.4) exists so that degenerate solution is visibly a failure.

## 9. Interview questions this file answers

- "Why not one state machine for the user?" (§1 — 3,024 states vs 30 values, and the flattening lie)
- "Can your model represent someone focused, tired, and avoiding at once?" (§2 — orthogonal registers, multi-label Barrier)
- "You said the control path is deterministic — isn't a belief model probabilistic?" (§4, §6 — probabilistic *perception*, deterministic cognition and action; bit-identical replay)
- "How do beliefs update, and what stops stale ones from driving behavior?" (§4 — log-odds + confidence half-life)
- "What can you actually sense from inside a phone app?" (§5 — the observability audit; why Novelty Pull barely works in Phase 1)
- "How do you stop it from nagging?" (§6 — do-no-harm veto, refractory, budget, hysteresis, least-power)
- "How do you test something probabilistic?" (§7 — replay, Brier/reliability, false-interrupt paired with missed-help, shadow policy)
- "Would a small LLM or embedding classifier infer state better than rules?" (§5.1 — yes for language, no for timers; the three-tier cascade and the ablation that decides it)
- "Doesn't putting an LLM in the loop break your determinism story?" (§5.1 — bounded influence, classification cache, and recording evidence rather than audio so replay stays bit-exact)
- "How do you know a belief is worth keeping?" (§7.6 — evidence ablation)
