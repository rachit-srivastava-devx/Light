# 05 — Determinism & LLM Consistency

> **Purpose:** the answer to "how do you keep the LLMs behaving the same, and how little do you depend on
> them?" — the deterministic shell (a state machine that takes **0 LLM decisions**), schema-locked +
> validated + repaired output, the **layered** determinism guarantee (structural = provable; output =
> bounded + measured), and version pinning so a silent provider upgrade can't move behavior. The honest
> core: **an LLM is not bit-deterministic — so we make the *control path* deterministic, *constrain* the
> output, and *measure* the residual variance and gate on it.**

---

## 1. The principle: a deterministic shell around a probabilistic core

Four layers, strongest first:

| Layer | Guarantee | Mechanism |
|---|---|---|
| **Control path** | **Deterministic (provable)** — same events → same behavior, always | the session state machine (§2) + a lookup-table router ([04 §2](04-MODEL-ROUTER-AND-THE-CREW.md)); **0 LLM decisions** |
| **Cognition / policy** | **Deterministic (provable)** — same evidence → bit-identical beliefs → same intervention | log-odds belief arithmetic + a pure-function policy ([13 §4, §6](13-COGNITIVE-STATE-AND-POLICY.md)); verified by **replay** |
| **Output shape** | **Always valid** | strict schema / structured decoding → validate → repair-once → gate (§3) |
| **Output content** | **Bounded + measured** consistency | low temperature, pinned version, per-session cache, and a consistency metric with a gate (§4) |

**Where the probability actually lives (the distinction that keeps the guarantee true):**

```
 PERCEPTION            →   COGNITION            →   ACTION
 probabilistic             deterministic            deterministic
 classifiers, prosody,     log-odds belief          policy table + session FSM
 similarity, small LLM     update (pure arith)      (pure function)
 ── bounded evidence ──▶   ── bit-exact replay ──▶  ── exhaustively unit-tested ──▶
```

Adding a **belief model did not add non-determinism to the control path** — it moved richer *estimates*
into the inputs while the decision rule stayed a pure function. A stochastic classifier can only emit
**bounded evidence** (capped log-odds, [13 §5.1](13-COGNITIVE-STATE-AND-POLICY.md)); it can never set a
belief, choose an intervention, or move a state. That is the same shape as the intent-classifier→router
split ([04 §2](04-MODEL-ROUTER-AND-THE-CREW.md)), applied to the user model.

The LLM does exactly one kind of thing: **produce words** (a step's text, a short reply). It never decides
a transition, selects a side-effect, or emits a control token. "Not heavily dependent on LLMs" is not a
slogan here — it's the measurable fact that **~65 % of turns touch no model at all**
([04 §4](04-MODEL-ROUTER-AND-THE-CREW.md)) and **100 % of *control* decisions are code**.

## 2. The session state machine (the full contract)

> **Scope note:** this FSM is the **Session register** — the *mechanical* lifecycle of the ritual (what the
> app is doing). It does **not** model the user; a person is not "in CHECK_IN," they are focused-and-tired
> -and-avoiding, which lives in the orthogonal cognitive registers of
> [13](13-COGNITIVE-STATE-AND-POLICY.md). The two compose: the Session register owns invariants INV1–INV5
> and the audio/step mechanics; the cognitive registers decide **whether and how** the orb intervenes
> within them.

Nine states; every transition is a rule on a **deterministic input** (a timer, an intent label, an OS
event). Transition table:

| From | Event | Guard | To |
|---|---|---|---|
| IDLE_PRESENT | tap | — | INTAKE |
| INTAKE | transcript | atomize dispatched | (stay; filler) |
| INTAKE | atomize_ready | steps schema-valid (§3) | STEP_PRESENT |
| STEP_PRESENT | spoken_complete | a **validated** step exists (INV2) | WORKING |
| WORKING | **policy emits an intervention** ([13 §6](13-COGNITIVE-STATE-AND-POLICY.md)) | passes veto + refractory + budget | CHECK_IN |
| CHECK_IN | reply | — | WORKING |
| WORKING | intent==done | **explicit** done (INV3) | STEP_DONE |
| STEP_DONE | — | more steps? | STEP_PRESENT : SESSION_DONE |
| *any* | os_interrupt | save prior | INTERRUPTED |
| INTERRUPTED | os_resume | bed recovered ≤300ms | prior state |
| SESSION_DONE | bed fade | last step done + confirm (INV4) | IDLE_PRESENT |

**Invariants the machine enforces (illegal states made unrepresentable):**
- **INV1 — never silent:** in every state but IDLE/SESSION_DONE, `bed_active == true` (enforced by the audio
  watchdog, [02 §11](02-ALWAYS-ON-AUDIO-ENGINE.md), not by the LLM behaving).
- **INV2 — no unvalidated step is ever spoken:** STEP_PRESENT reads a step **by index** from the validated
  list (§3) — reference-not-generate. The orb physically cannot speak a step the schema didn't accept.
- **INV3 — completion is declared, never inferred:** advance only on an explicit `done` intent; silence is
  *working*, never *finished* ([01 §3](01-SYSTEM-OVERVIEW-AND-PLANES.md)).
- **INV4 — the session ends deliberately:** SESSION_DONE requires the last step done + a confirmation; the
  model can't end it (scar [04 §8.1](04-MODEL-ROUTER-AND-THE-CREW.md)).
- **INV5 — cost can't run away:** a per-session token **reservation** caps spend; exceeding it is
  impossible, not merely alerted ([08 §6](08-COST-MODEL.md)).
- **INV6 — the orb cannot nag:** interventions are bounded by a **refractory period (≥ 90 s)** and a
  **per-session budget (≤ 6)**; both are counters the policy checks *before* emitting, so over-intervening
  is structurally impossible rather than tuned-against ([13 §6](13-COGNITIVE-STATE-AND-POLICY.md)).
- **INV7 — no pushing a dysregulated user:** when `Barrier.Emotionally-Dysregulated > 0.8`, the available
  intervention set is reduced to `{Pause, Escalate}` and the prosody allowed-set excludes every
  shame-adjacent register — the harmful response is *unrepresentable*, not discouraged
  ([13 §3](13-COGNITIVE-STATE-AND-POLICY.md), [12 §5](12-LLD-AND-SESSION-CONTRACT.md)).

## 3. Schema-locked output: validate → repair-once → gate → use

The atomizer's output is not free text — it is a **strict schema**, enforced at decode time and again after:

```
Atomizer output schema (validated before ANY use):
  { steps: [ { step_text: string(≤120 chars), est_min: int(1..15), done_signal: string } ],
    steps_total: int(1..12) }
```

The pipeline (every model output that will be spoken or acted on):
1. **Structured decoding / function-calling** so the *shape* is valid by construction (provider-native JSON
   mode; the token stream can't leave the grammar).
2. **Validate** against the schema + semantic rules (`est_min ∈ 1..15`; `step_text` non-empty, single
   action; `steps_total ≤ 12` — a 20-step plan is itself un-ADHD and is rejected).
3. **Repair-once:** on a validation failure, one bounded re-ask with the error; a second failure **fails
   closed** to a deterministic fallback ("let's just start by opening it") — never a silently-wrong step.
4. **Gate + use by reference:** the state machine holds the validated list and speaks steps **by index**
   (INV2). The model's text is *data in a slot*, never a command.

**Per-class error taxonomy** (measured, [06 §3](06-EVALS-AND-TESTING.md)): `schema_invalid` ·
`step_not_atomic` (too big) · `step_count_explosion` (>12) · `empty/duplicate` · `off-task`. Each has a rate
and a merge-blocking threshold — "we validate the output" with no taxonomy and no rates is a defect, not an
answer.

## 4. "Behaves the same" — what it means, and how it's measured

**An LLM is not bit-deterministic.** Even at temperature 0, floating-point non-associativity across GPU
batching and provider-side infra changes make identical output run-to-run impossible to *guarantee*. So we
do not claim bit-identical; we claim a **layered, measured** consistency:

- **Reduce variance:** the atomizer runs at **low temperature (≤ 0.2)** — we want a consistent plan, not
  creativity — with a **fixed seed where the provider supports it** (some do, some don't; stated honestly,
  not assumed). Conversational filler can run warmer (it's off the control path, variance there is
  harmless).
- **Constrain:** the schema (§3) removes shape variance entirely — the *structure* is always identical.
- **Cache:** the atomization is computed **once per task and cached** — so *within a session* the steps are
  perfectly stable (the user never hears a step re-word itself). Same task tomorrow may differ slightly;
  that's fine, and bounded next.
- **Measure the residual + gate:** the **consistency harness** runs the atomizer **K=10 times** on each of
  **N=200 golden tasks** and computes: **step-count mode agreement ≥ 90 %**, **pairwise step-set semantic
  similarity (embedding cosine) ≥ 0.85**, **is-atomic pass ≥ 95 %**. These are merge-blocking
  ([06 §2](06-EVALS-AND-TESTING.md)). *That* is what "behaves the same" means as a number — not a vibe.
- **pass^k reality:** a user runs many sessions; if per-run atomization is good with probability *p*, then
  *k* good sessions in a row is *pᵏ*. For 95 % across 8 sessions, per-run must be **≥ 0.95^(1/8) ≈ 99.4 %**
  — which is why the per-run bar is high and the demo "working once" means nothing
  ([06 §3](06-EVALS-AND-TESTING.md)).

## 5. Version pinning — "the same" over time, not just per run

The quietest way to break consistency is to change nothing yourself:

- **Pin the model version** (`gemini-2.5-flash-lite-<date>`, `claude-haiku-4.5-<date>`). A silent provider
  upgrade shifts atomization behavior with **zero code change** — you "regress" while the product did
  nothing. Upgrades are **scheduled events with re-running the consistency + eval harness**, never
  automatic.
- **Pin the TTS voice id + style** ([03 §4](03-VOICE-LATENCY-PIPELINE.md)) — the companion must *sound* the
  same every session; a changed default voice is as jarring as a changed personality.
- **Pin + version the prompt** (template + few-shots in git; `prompt_version` stamped into every trace).
  "Which prompt produced this plan?" is a query, not archaeology. A prompt change ships behind a flag with a
  shadow run first (§6, [06 §6](06-EVALS-AND-TESTING.md)).
- **Note the retirement risk (R5):** Flash-Lite has a listed Oct-2026 retirement ([README §6](README.md)) —
  the pin is also a *tripwire* forcing a deliberate, eval-gated migration to its successor, not a surprise.

## 6. Determinism is also a latency and cost program

The three are the same lever, not a trade-off:
- Every turn the router keeps deterministic ([04 §2](04-MODEL-ROUTER-AND-THE-CREW.md)) is **faster** (no
  TTFT, [03 §1](03-VOICE-LATENCY-PIPELINE.md)) **and cheaper** (no tokens) **and more consistent** (code,
  not sampling).
- The per-session atomization cache makes the steps stable **and** free-on-repeat **and** instant to speak
  (pre-synthesized audio, [03 §4](03-VOICE-LATENCY-PIPELINE.md)).
- So "make it deterministic" pays out simultaneously in [03] (latency), [08] (cost), and here (consistency).
  That alignment is why the architecture leans on it so hard.

## 7. How to test (exactly)

1. **State-machine model test:** exhaustively drive the §2 transition table (property-based: random event
   sequences) and assert every invariant (INV1–INV5) holds. **Gate: 100 % — it's code.**
2. **Consistency harness:** K=10 × N=200 golden tasks → step-count mode agreement, step-set cosine,
   is-atomic rate. **Gates: ≥ 90 % / ≥ 0.85 / ≥ 95 %** (§4). Runs on any prompt/model/version change.
3. **Schema-validation coverage:** fault-inject malformed model outputs (missing field, 20 steps, empty
   step, non-JSON); assert **100 %** are caught and either repaired-once or failed-closed — **0 reach the
   user** ([06 §3](06-EVALS-AND-TESTING.md)).
4. **Version-pin sentinel canary:** a fixed sentinel task runs nightly against the pinned model; **any shift
   in its atomization pages** — that is how a silent provider upgrade is caught in a night, not a month
   (scar §8.2).
5. **Repair-rate + fallback-rate monitors:** production rates of repair-once and fail-closed-fallback; a
   rise means the model or prompt drifted — an early-warning proxy that beats the weekly eval
   ([06 §6](06-EVALS-AND-TESTING.md)).

## 8. Operator's scars

1. **Temp-0 ≠ deterministic.** An early "we set temperature 0, so it's deterministic" claim was falsified
   the first week — identical tasks produced 4- vs 5-step plans across runs (GPU batching non-determinism).
   The honest fix was the *measured* consistency bound (§4), not a false promise of identical output.
2. **The silent upgrade.** A provider point-upgrade shifted the atomizer toward longer, less-atomic steps
   overnight; two days lost chasing a "regression" the code never caused. Pins everywhere + the sentinel
   canary (§5, §7.4) exist because of that week.
3. **The 20-step plan.** Without a `steps_total ≤ 12` rule, the atomizer once returned a 19-step
   decomposition of "clean the kitchen" — technically valid JSON, catastrophically un-ADHD (a wall of
   steps is the anxiety the product exists to remove). The schema now bounds it; too-many-steps fails
   validation and re-asks smaller (§3).
4. **Repair loop with no floor.** The first repair path re-asked until valid — an infinite loop on a
   pathological input froze a session. Repair is now **once**, then a deterministic fallback (§3); the thing
   that fixes a bad output can never itself hang the session.

## 9. Interview questions this file answers

- "LLMs aren't deterministic — how do you make yours 'behave the same'?" (§1, §4 — layered: control provable, output measured)
- "What does an LLM decide in your system?" (§1, §2 — words only; 0 control decisions)
- "Show me how you keep a model's output from being silently wrong." (§3 — schema → validate → repair-once → fail-closed → use-by-reference)
- "'Same input, same behavior' — prove it with a number." (§4 — the K×N consistency harness + gates)
- "A provider silently upgrades the model overnight — what happens?" (§5 — pins + the sentinel canary)
- "Isn't all this determinism slowing you down?" (§6 — it's the same lever as latency and cost)
