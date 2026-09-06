# 06 — Evals & Testing

> **Purpose:** the complete "how do you test this?" — the eval pyramid with sized sets and gates, the
> **product-defining "is-this-step-ADHD-atomic" metric**, the statistics that turn observations into
> guarantees (rule of three, pass^k), the determinism/latency/audio gates each file references, calibrated
> judges, the CI contract, and the online proxies that page in minutes. **In this product the eval isn't QA
> bolted on — the atomizer eval is the spec of the whole thing.**

---

## 1. The eval pyramid (what runs where, with sizes and thresholds)

| Layer | Suite (size) | Metric → gate | Runs on |
|---|---|---|---|
| **Audio invariant** | interruption matrix (§[02](02-ALWAYS-ON-AUDIO-ENGINE.md) §5) + 8 h soak | underruns/session-hr **== 0**; recovery p99 ≤ 300 ms | any audio-module change |
| **Latency** | on-device turn harness × network matrix | Shape-B p50 ≤ 600 ms, p99 ≤ 1.2 s; Shape-A ≤ 250 ms; **think-time-silence == 0** | any voice-path change |
| **Router (deterministic)** | full transition table | **100 %** (it's code, not an eval) | any router change |
| **Intent classifier** | 500 labeled utterances (ADHD + Hinglish) | accuracy ≥ 97 %; `question→next` ≈ 0 | classifier/prompt change |
| **Atomizer quality** | **300 golden tasks** | **is-atomic ≥ 95 %/step · first-step-startable ≥ 99 % · count ≤ 12** (§2) | any atomizer/model/prompt change |
| **Atomizer consistency** | K=10 × N=200 tasks | count-mode ≥ 90 % · step-set cosine ≥ 0.85 (§[05 §4](05-DETERMINISM-AND-LLM-CONSISTENCY.md)) | any model/prompt/version change |
| **Schema safety** | fault-injected malformed outputs | **100 %** caught; 0 reach the user (§3) | every merge |
| **Cost** | 200-session replay | ₹/session ≤ baseline +10 % | every merge |
| **Voice-to-voice (the primary gate)** | **200 spoken tasks**, audio-in → audio-out (§1.1) | v2v latency p50 ≤ 1.1 s · silence-events == 0 · WER ≤ 12 % · **empathy-appropriateness ≥ 95 %** | any change to *any* hop of the voice path |
| **Belief calibration** | **100 sessions × 30 s windows ≈ 5,000 labeled** ([13 §7](13-COGNITIVE-STATE-AND-POLICY.md)) | **Brier ≤ 0.15** · no reliability bin off by > 0.15 | any belief/evidence/weight change |
| **Policy behavior** | replay over the same corpus + nag simulation | **false-interrupt ≤ 5 %** *reported with* missed-help · ≤ 6 interventions/session · none < 90 s apart | any policy/threshold change |
| **Belief replay** | recorded evidence streams | **100 % bit-identical** belief trajectories | every merge |
| **Online** | nightly graded sample + real-time proxies (§8) | atomic-rate CI; abandon-rate; repair-rate | production, always |

### 1.1 Voice-to-voice: the eval that actually matches the product

**Every headline gate runs on audio, not text.** A text-only harness passes while the real product fails,
because it never exercises the hops that actually break: endpointing cutting a user off, STT mangling
"file my taxes" into "file my taxis", TTS mispronouncing a step, a 400 ms gap between filler and content, or
a flat delivery on a win. So the primary suite plays **synthesized user audio into the mic path and grades
the audio that comes out.**

**The spoken-task corpus (N=200):** each item is an audio file of a person stating a task, stratified by
- **accent/language:** Indian English, Hinglish code-mixed, American, British, non-native (40 each);
- **ADHD speech patterns:** mid-thought pauses, restarts ("I need to— actually first I should…"), trailing
  off, rambling 45 s dumps, and terse 3-word tasks;
- **acoustic conditions:** quiet, café noise, TV background, headset vs speakerphone.

**What is measured, end to end:**

| Metric | Definition | Gate |
|---|---|---|
| **True voice-to-voice latency** | last sample of user audio → first sample of orb audio | **p50 ≤ 1.1 s · p99 ≤ 2.0 s** (the honest user-clock number; ≥ the [03 §1](03-VOICE-LATENCY-PIPELINE.md) budget because it includes capture + playback) |
| **Silence events** | any window > 300 ms with no audio during a live turn | **== 0** (invariant #1, measured from the recording) |
| **Loop WER** | transcript of what the orb *heard* vs the reference | ≤ 12 % overall; **≤ 18 % on Hinglish** (the honest per-slice bar) |
| **False-endpoint rate** | orb responds while the user was still mid-thought | ≤ 3 % ([03 §2](03-VOICE-LATENCY-PIPELINE.md)) |
| **Step quality through audio** | is-atomic rubric applied to what was *spoken* | ≥ 95 % (§2) |
| **Empathy-appropriateness** | does the delivered prosody match the moment? (§1.2) | **≥ 95 %**, and **0** shame-toned check-ins |
| **Intelligibility/naturalness** | MOS-style judged on the output audio | ≥ 4.0 / 5 |

### 1.2 Grading empathy (the top-priority surface, made measurable)

Since emotional register is a first-class product surface ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)), it gets
a real metric rather than a vibe check:

- **Empathy-appropriateness** = a rater (human batch, judge-assisted) hears the orb's audio **plus the
  session context** and answers: *is this delivery appropriate for this moment?* — with a **hard-fail list**:
  any celebratory tone on an abandonment, any urgent/stern tone on a check-in or stuck moment, any flat
  delivery on a completion. **Hard-fails gate at 0** — they're the shame-spiral risks that the allowed-set
  in [12 §5](12-LLD-AND-SESSION-CONTRACT.md) makes structurally impossible, so a single occurrence means the
  structural guarantee has a hole.
- **Prosody-variance check (catches the "indifferent voice" failure):** measure pitch/energy variance across
  emotion states; **if a win and a check-in are acoustically indistinguishable, the director is broken** —
  this is the automated version of scar [01 §7.5](01-SYSTEM-OVERVIEW-AND-PLANES.md). Gate: measurable
  separation between celebratory and gentle registers.
- **Human calibration:** the empathy judge is calibrated against a **100-clip human-rated batch** monthly at
  **κ ≥ 0.8**, same discipline as the atomicity judge (§6) — an uncalibrated empathy score is worse than
  none, because it licenses shipping a tone-deaf voice.

## 2. The product-defining metric: "is this step ADHD-atomic?"

Everything rides on the atomizer producing a *startable* step. So "atomic" is defined as a rubric, not a
vibe — a step passes iff **all four** hold:

1. **Single physical action** — one verb, one object ("open the tax portal"), not a project ("do your taxes").
2. **Initiation cost < ~2 min** — you can begin it *now*, no prep, no setup.
3. **No embedded sub-decision** — beginning it doesn't require choosing or planning first.
4. **Observable done-signal** — you know when it's finished ("the portal is open").

- **Grading:** a calibrated LLM-judge scores each criterion 0/1 with an evidence quote (§6); a step is
  atomic iff all four = 1. The **first step is held to a higher bar (≥ 99 % startable)** — the first step is
  the entire point of the product (starting is the hardest moment for ADHD), so a non-startable first step
  is the worst failure and weighted accordingly.
- **Gold set (N=300):** task → reference atomic decomposition, **stratified** by category
  (health/office/coding/chores/admin) and by task size (small/medium/overwhelming). **Provenance:** seeded
  synthetic pre-launch; then **grown from production by rule** — every session where the user got stuck on a
  step, said "that's too big," or abandoned after a step becomes a candidate **hard negative** (a "not
  atomic" example). The set grows from where the product actually fails, not from memory.
- **Session-outcome metric (the ground truth):** **step-completion rate** and **abandon-after-step-N** —
  the real signal that atomization worked. High abandon right after step 1 = steps too big, independent of
  what the judge said. The offline metric predicts; the outcome metric decides.

## 3. The statistics (what makes a guarantee a number)

- **Rule of three:** 0 failures in *n* trials → 95 % upper bound ≈ **3/n**. To claim a ≤ 1 % non-atomic
  first-step rate, the pre-launch smoke needs **≥ 300 clean** first steps; "we tried 20 and they were fine"
  bounds you at 15 % — worthless.
- **pass^k for the monthly user:** a user runs ~30 sessions/month; the product "works for them" only if
  atomization is good *session after session*. For **95 % of users to get 8 consecutive good sessions**,
  per-session atomization must be **≥ 0.95^(1/8) ≈ 99.4 %** — which is why the per-run bar is 95 %+ and
  climbing, and why "it worked in the demo" is meaningless (0.9⁸ ≈ 43 %).
- **Nightly CI:** ~1,000 graded production atomizations at p̂ ≈ 3 % non-atomic → 95 % CI ≈
  1.96·√(0.03·0.97/1000) ≈ **±1.05 %** — tight enough to catch a 3 %→5 % regression overnight.
- Sample sizes are **derived from the guarantee**, never guessed.

## 4. Determinism, latency, and audio gates (consolidated)

These live in their home files; the CI *enforces* them here:
- **Determinism:** the K×N consistency harness + schema-safety, gates in §1 ([05 §7](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
- **Latency:** the on-device turn harness × network matrix + the think-time-silence probe ([03 §7](03-VOICE-LATENCY-PIPELINE.md)).
- **Audio:** the continuous-audio watchdog (0 underruns) + the interruption suite ([02 §11](02-ALWAYS-ON-AUDIO-ENGINE.md)).
- The point of consolidating: a merge is judged against **all** of them at once, so a latency win that
  quietly worsens the atomic-rate is caught (the classic "optimized one number, broke another").

## 5. Schema safety as a test (0 silently-wrong reaches the user)

Fault-inject the atomizer's output path with: missing field · 20-step explosion · empty/duplicate step ·
non-JSON · `est_min` out of range · off-task step. Assert **100 %** are caught and either repaired-once or
failed-closed to the deterministic fallback ([05 §3](05-DETERMINISM-AND-LLM-CONSISTENCY.md)) — **0 reach the
spoken output.** This is a *structural* guarantee (a step is spoken by index from a validated list, INV2),
so the test proves a missing code path, not a low probability.

## 6. LLM-as-judge — calibrated or it's a random-number generator

- **Judge:** a cheap model + the atomicity rubric (§2), scoring each criterion with a required **evidence
  quote** before its verdict (forces it to look; makes the output auditable). It is a **different, pinned**
  model from the atomizer — never grade a model with its own family (self-preference bias is real and
  measured).
- **Calibration:** a monthly **200-sample human-labeled** batch → judge-vs-human **κ ≥ 0.8 per criterion**
  or the rubric is repaired and re-validated before the judge's numbers are trusted. Judge–human
  disagreement is itself a paged metric.
- **Bias controls:** length-control (judges reward verbose steps — the opposite of atomic; corrected),
  position-swap on any pairwise comparison, and **pin the judge version** (a silent judge upgrade moves
  every metric with zero code change).

## 7. What blocks a merge (the CI contract)

```
on PR touching {atomizer, prompts, models, router, voice-path, audio, prosody, TTS/STT provider, RN audio lib}:
  voice-to-voice-200    → v2v p50 >1.1s | silence-events >0 | WER >12%
                          | empathy-appropriateness <95% | ANY empathy hard-fail  = RED
  atomizer-goldens-300  → is-atomic <95% | first-step-startable <99%              = RED
  consistency-K10×N200  → count-mode <90% | step-cosine <0.85                     = RED  (if model/prompt/version)
  classifier-500        → accuracy <97% | question→next spike                      = RED  (if classifier)
  schema-safety         → any malformed output reaches user                        = RED
  audio-suite           → gap events/session-hr >0 | recovery p99 >300ms           = RED  (if audio or lib bump)
  latency-harness       → Shape-B p50 >600ms | Shape-A >250ms | think-time-silence >0 = RED (if voice path)
  belief-replay         → any non-identical trajectory                             = RED
  policy-behavior       → false-interrupt >5% | budget/refractory violated         = RED  (if policy/beliefs)
  belief-calibration    → Brier >0.15                                              = RED  (if beliefs/weights)
  cost-replay-200       → ₹/session > baseline+10%                                 = RED
```
Everything else (abandon-rate delta, repair-rate) **alerts**; those **eleven block**. Gates you'd override
weekly train people to override gates.

**Note the ordering:** voice-to-voice is listed first because it is the only suite that grades the product
**as the user experiences it** — every other suite grades a component. A green component suite with a red
v2v means the *integration* is broken, which is exactly the class of failure text-only evals miss.

## 8. Online: shadow → canary → the real-time proxies

- **Shadow:** a candidate prompt/model runs against **mirrored intake tasks**, output discarded, atomic-rate
  + consistency + cost diffed vs live before any user sees it.
- **Canary:** 5 % of sessions, **auto-rollback** wired to: atomic-rate CI breach, abandon-after-step-1 spike,
  repair-rate spike, ₹/session +15 %, or turn-latency p99 +30 %. Rollback is a flag flip ([05 §5](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
- **Real-time correctness proxies (the headline — you can't wait for the weekly eval):**
  - **"that's too big" / rephrase utterances** — a *direct* user signal the step wasn't atomic.
  - **abandon-after-step-N**, **repair-rate**, **fallback-rate**, **re-atomize-rate** — each with an SLO; a
    regression pages in minutes, not next week.
- **Drift:** intake-task mix shifts (new user segment, new category) → the eval-vs-prod delta monitor;
  refresh the gold set monthly with the mined hard negatives (§2). A green eval on last month's task mix is
  a false comfort.

## 9. Operator's scars

1. **The judge rewarded thoroughness.** An uncalibrated judge scored a detailed 8-step plan *higher* than
   the correct 3-step one — exactly backwards for ADHD. Length-control + the κ calibration (§6) exist
   because the first gold set silently optimized *against* the product.
2. **Deflection-style Goodhart.** Optimizing "steps completed per session" alone pushed the atomizer toward
   trivially-tiny steps that inflated the count while users still abandoned — the metric went up as the
   product got worse. Abandon-rate + "too big"/"too small" utterances now veto (§8).
3. **Eval passed, users bailed.** A green atomizer eval (on the seeded synthetic set) hid that real users'
   tasks were vaguer and messier; abandon-after-step-1 was the only thing that caught it. The gold set now
   grows from production, and the outcome metric outranks the offline one (§2).
4. **pass^1 is a lie.** Early confidence came from "it nails the demo task." At 90 %/run that's 43 % of
   users failing within 8 sessions — the pass^k math (§3) is why the bar is 99 %+, learned by shipping to
   real repeat users.
5. **The text eval was green while the product was unusable.** Every offline suite passed while real users
   hated it: the endpointer clipped Hinglish mid-thought, STT turned one task into nonsense, and the win
   line was delivered flat. **None of those hops exist in a text harness.** The voice-to-voice suite (§1.1)
   exists because the only eval that predicts the product is the one that listens to it.
6. **Measured empathy last, and it silently regressed.** A TTS provider's default style changed between
   versions; the voice went subtly flatter and nobody noticed for weeks — CSAT drifted with no code change.
   The prosody-variance check (§1.2) now catches an indistinguishable win-vs-check-in automatically, and the
   TTS version is pinned like a model version ([05 §5](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).

## 10. Interview questions this file answers

- "How do you test this whole system?" (§1 — the pyramid with sizes + gates)
- "How do you evaluate a *voice* agent — surely not on text?" (§1.1 — audio-in→audio-out, 200 spoken tasks, true v2v latency)
- "How do you measure something as soft as empathy?" (§1.2 — appropriateness with a hard-fail list, prosody-variance, κ-calibrated)
- "How do you even measure whether a step is 'good' for ADHD?" (§2 — the atomicity rubric + the outcome metric)
- "How many samples to claim a ≤1% bad-first-step rate?" (§3 — rule of three, derived)
- "What's pass^k and why does it matter here?" (§3 — the monthly repeat user)
- "LLM-as-judge — why trust it?" (§6 — κ, evidence quotes, pinned, bias controls)
- "What actually blocks a deploy?" (§7)
- "How do you catch a regression before next week's eval?" (§8 — the real-time proxies, incl. 'that's too big')
