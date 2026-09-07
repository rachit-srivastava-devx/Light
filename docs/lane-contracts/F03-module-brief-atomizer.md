# F03 — Module-brief atomizer (lane contract)

| | |
|---|---|
| **Feature** | F03 — re-aim the decode→validate→repair-once→fail-closed decomposition pipeline from physical ADHD steps to `ModuleBrief`, for build-mode sessions only |
| **Branch / worktree** | `lane/F03-atomizer` → `Light/.worktrees/F03-atomizer` |
| **Repos touched** | `orb/` only (`backend/relay-py`). Zero files in `fleet/`, `apps/macos/`, `registry/`. |
| **Lane base** | **`6fd66f3`** (`git merge-base HEAD master` — verified, tree clean at contract time) |
| **Depends on** | F02 (merged, `6fd66f3`) · F04 (merged, `dafffe5`) |
| **Blocks** | F05 (freeze protocol), F09 (orb→fleet handoff) |
| **Lead** | lead-architect (F03). **Contract only — the lead wrote no production code** (T1). |
| **Merge class** | Touches `proxy/schemas.py`, a wire-contract surface → **human-merge always (A15 / D4). Ship to a PR and stop.** |

**This file is the spec.** The acceptance suite in §6 is written here, in full, before any
implementation exists. §6.9 states exactly which cases are **red on arrival** and which are
**green-on-arrival regression locks**, with the reason each lock is a lock and not a broken gate.

**The builder may not edit §6's test file.** Any diff touching
`orb/backend/relay-py/tests/test_f03_module_brief_decompose.py` other than creating it verbatim from
§6 is a contract violation — flag it, do not merge it.

Written against `blueprints/Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md` §2 read in
full, plus `docs/lane-contracts/F02-lld-v1-contract.md` and `docs/lane-contracts/F04-server-side-build-mode.md`
(both binding on this lane — see §1.2 and §3.1), plus the live tree.

---

## 0. Restatement — what is actually being asked, in this codebase's own terms

The brief as dispatched: *"repoint `atomizer.py`'s output type from physical ADHD-task steps to
`ModuleBrief` for build-mode sessions only, keeping the pipeline shape unchanged; focus-mode's
atomizer path must be provably untouched."*

**That is not the work, because the repoint already happened.** The C1 pass (§1) found
`orb/backend/relay-py/src/orb_relay/proxy/lld_decomposer.py` — 208 lines, already merged, already
doing exactly what the brief describes:

- same pipeline shape (`MAX_REPAIR_ATTEMPTS = 1`, decode → validate → repair-once → fail-closed),
- output type is `ModuleBrief` (imported from F02's Python mirror `proxy/lld_schemas.py`),
- fail-closed target correctly moved from a guessed `FALLBACK_STEP` to a `clarify_request`, with the
  blueprint's own reasoning quoted in its docstring ("a guessed module brief costs a whole lane, not
  30 seconds"),
- `atomize()` untouched, and deliberately not shared with — the module docstring records the
  sibling-not-a-generic decision explicitly.

So the construction is done. **What is missing is the composition.**

### 0.1 The actual defect, proved not assumed

Two probes, both run against this worktree at `6fd66f3`:

**Probe 1 — grep for callers.**

```
$ grep -rn "lld_decomposer\|decompose(" orb/backend/relay-py/src orb/backend/relay-py/tests
src/orb_relay/proxy/lld_decomposer.py:142:async def decompose(          # the definition
src/orb_relay/proxy/lld_decomposer.py:{10,166,188,197}                  # its own docstring + dev_log strings
tests/test_freeze_store_append_only.py:{30,32,35,37}                    # one unit test
```

**Zero production callers.** Nothing under `src/` imports it. The only external reference in the
entire tree is a single unit test that asserts what `decompose()` does *not* do (it has no fallback).

**Probe 2 — drive the runtime.** `orb/backend/relay-py/src/orb_relay/app.py` is the only process a
client can reach. It imports `atomize` (`app.py:24`) and calls it (`app.py:473`). It does not import
`lld_decomposer` at all. **There is no request any client can send — RN, Swift, or a raw WebSocket
test client — that causes a `ModuleBrief` to be produced.**

This is the exact defect class recorded in memory as *"contract layer excellent, composition layer
missing"* (ADHD Focus Orb L8 review, 2026-08-04), and it is `fleet/PRINCIPLES.md`'s most-repeated
law: **a proxy is not the property.** `lld_decomposer.py` existing and its unit test passing is the
proxy. The property is that a build-mode session actually produces a validated `ModuleBrief`. Today
it cannot.

**F03 is therefore a wiring lane, not a construction lane.** Scope: compose the existing decomposer
into the existing build-mode branch, fix the one real defect inside it (§0.2), and prove focus mode
untouched.

### 0.2 The second defect — the one bounded repair is spent blind

`lld_decomposer._parse_and_validate` (line 128) calls `validate_module_brief(parsed)`, whose
signature is `dict -> ModuleBrief | None`. Every schema violation — a missing `acceptance`, a
`data_owned` naming another node's store, one killed alternative instead of two — collapses to the
same `None`, and is turned into one fixed string:

```python
return DecomposeRejection(detail="module brief failed lld.v1 schema validation")
```

which is then handed to `_repair_prompt`, so the model's single re-ask reads, in full:

```
Goal: <goal>

Your previous answer was rejected: module brief failed lld.v1 schema validation
Return corrected JSON only.
```

It names nothing. Compare `atomizer.py:141`, whose repair prompt names the violation
(`f"[{error.code.value}] {error.detail}"`) and whose docstring states the principle: *"a model that
just failed a constraint needs the constraint named, not restated rules."* `lld_decomposer.py`'s own
docstring claims it shares this shape. It does not.

The consequence is measurable, not stylistic: the repair attempt is a coin flip, so decode failures
fall through to `clarify_request` far more often than they need to, and each one burns a second paid
model call to learn nothing.

**F02 already shipped the fix input.** `validate_module_brief_detailed(raw) -> ValidationResult`
returns `errors: list[ValidationErrorDetail]`, each with a `.path` (`"alternatives[0]"`,
`"guarantees[0].derivation.calc"`) and a `.message`, hand-written precisely so the paths are stable
and cross-language-comparable. F03 switches the repair path onto it.

This is in scope because it is not a shape change — the pipeline stays decode → validate →
repair-once → fail-closed, with the same bound of exactly one repair. Only the repair prompt's
*content* changes, from a constant to the actual violation list. Leaving a known-blind repair in
place while wiring this exact module into production would be negligent.

---

## 1. C1 / L2 registry verdict, said out loud

**Verdict: `install` (everything F03 needs already exists) + one bounded `extract` (the metering
envelope) + zero `build_new`.**

`Light/registry/{features,services}/` are empty (established by F01's and F04's C1 passes), so the
pass runs against the copied tree.

| Capability F03 needs | Already exists? | Verdict |
|---|---|---|
| decode→validate→repair-once→fail-closed emitting `ModuleBrief` | **Yes** — `proxy/lld_decomposer.py`, 208 lines | **install.** Do not rewrite. Wire it. |
| `ModuleBrief` type + validator | **Yes** — `proxy/lld_schemas.py` (F02) | **install.** F02 owns it; F03 adds no field. |
| Detailed, path-carrying validator for the repair prompt | **Yes** — `validate_module_brief_detailed` (F02) | **install.** Swap the call, don't write a validator. |
| `mode=build` on the wire, typed, 422 on unknown | **Yes** — `ResponseMode.BUILD`, `proxy/schemas.py:103` (F04) | **install.** Zero new lines. |
| Build-mode branch in a live route | **Yes** — `app.py:798`, `/v1/respond` (F04) | **install / extend additively.** |
| Append-only build-session evidence store | **Yes** — `store/build_session_store.py` (F04) | **install.** Append through its existing API. |
| Tier-0 evidence + register fold | **Yes** — `cognitive/belief.py` (F04) | **install.** |
| A valid `ModuleBrief` fixture | **Yes** — `fleet/contracts/fixtures/lld/complete_module.json` (F02) | **install.** Do not hand-author a brief. |
| A brief invalid in exactly one named way | **Yes** — `fleet/contracts/fixtures/lld/one_alternative.json` (F02) | **install.** This is T3's whole mechanism. |
| Reserve → call → settle/release budget envelope | **Yes, but inline** — `app.py:441-500` (`/v1/atomize`) and again in `/v1/respond` | **extract**, bounded — see §3.4. |
| A brief→Coverage-evidence deriver | **No** | The only genuinely new logic. ~20 lines, a field read, not NLU. See §3.3. |

**Nothing in this lane is `build_new` at the module level.** If the builder finds itself writing a
new pipeline, a new validator, a new store, or a new fixture, it has left the lane — stop and flag.

### 1.1 Two peer contracts are binding on this lane

Both were read in full. Each constrains F03's design, and one of them kills the option this lane
would otherwise have chosen.

1. **F04 §8.2** — *"**F03** — the atomizer repoint. Do not open `proxy/atomizer.py`."* F04 declared
   `atomizer.py` to be F03's surface and stayed off it. F03 honors that by not touching it either
   (§3.5) — the file ends this lane with a zero diff.
2. **F04 §2.4 — KILLED: "invent a second wire contract for mode selection (a new `/v1/build/...`
   endpoint family)."** Reason given: *"one server-side surface serves both clients... a second
   endpoint family means `apps/macos` would be built against a contract `apps/mobile` does not
   speak, re-creating the fork this feature exists to close."* **This is binding on F03 and it
   overrides this lane's own first instinct** — a dedicated `/v1/build/decompose` route would have
   made "focus mode untouched" trivially provable by leaving `/v1/atomize` at a zero diff, but it
   would re-fork the client contract that F04 exists to unfork. See §2.2, where it is recorded as a
   killed alternative with F04's reasoning, not this lane's.

---

## 2. Killed alternatives

### 2.1 KILLED — mutate `atomize()` itself: add a `mode` parameter, or make it generic over its output type

The literal reading of the dispatched brief, and of blueprint §2.1 ("keep it byte-for-byte in
*structure* and change only the **output type**").

**Why killed.** Blueprint §2.1 was written before `lld_decomposer.py` existed. Now that it does, the
re-aim is achieved by **selection between two siblings**, not by mutation of one — and three
independent reasons say selection is right:

- **It would undo a decision another lane made deliberately.** `lld_decomposer.py`'s docstring is
  explicit: *"A sibling of `proxy/atomizer.py`'s `atomize()`, NOT a generic parameterised over it."*
  Re-parameterizing `atomize()` reverses that on no new evidence.
- **The two pipelines' fail-closed targets are semantically opposite.** `atomize()` must return a
  plausible guess (`FALLBACK_STEP`); `decompose()` must never return a guess. A single function whose
  terminal branch flips between "fabricate something helpful" and "refuse to fabricate" on a boolean
  is the exact shape you do not want one `if` away from each other. Blueprint §2.2 calls this
  distinction "the whole point of the re-aim."
- **It destroys the strongest available proof.** With `atomizer.py` at a literal zero diff,
  "focus-mode's atomizer path is untouched" is proved by `git diff --numstat` — a structural
  argument. Mutate the file and the claim degrades to "our tests didn't catch a regression," a
  behavioral proxy over a path used by every existing focus user.

**Revive trigger:** a third decomposition output type appears *and* the two siblings' shared surface
exceeds ~80% — the inverse of blueprint §2.2's own stated trigger (*"if the `ModuleBrief` schema
diverges so far from `PhysicalStep` that <20% of the pipeline is shared"*, read in the reuse
direction). At that point extract a shared engine as a C2; never hand-merge the two.

### 2.2 KILLED — a dedicated `/v1/build/decompose` route (killed by F04 §2.4, not by this lane)

The design this lane reached first, and would have chosen on its own merits: `/v1/atomize` and its
handler stay at a literal zero diff, `DecomposeResponse` gets its own model with no optional-`output`
breaking change, and "focus mode untouched" becomes provable by pure structure.

**Why killed.** F04 §2.4 already killed the `/v1/build/...` endpoint family, and its reasoning
applies unchanged here: a second endpoint family gives `apps/macos` a contract `apps/mobile` does not
speak, re-creating the client fork F04 exists to close. F04's decision is merged and load-bearing;
this lane does not get to re-litigate a peer contract's decision because the alternative would have
made *this* lane's proof tidier. **A tidier proof for one lane is not worth a forked wire contract
for the product.**

The cost of accepting F04's kill is real and is paid explicitly in §3.5: focus-mode's proof is no
longer "the whole route is untouched" but "`atomizer.py` and `semantic_checks.py` are untouched, AND
`/v1/atomize`'s handler is untouched, AND both directions of the mode branch are asserted with
spies." Three checks instead of one. That is the price of the right architecture.

**Revive trigger:** build-mode decomposition needs a request or response shape sharing no field with
`ConversationRequest` beyond identity — at which point the union envelope is worse than two routes,
and the fork already exists in substance.

### 2.3 KILLED — extend `/v1/atomize` with a `mode` field

The middle option: one decomposition door, mode-selected, matching `/v1/respond`'s existing
discipline.

**Why killed.** `AtomizeResponse.output: AtomizerOutput` is **non-optional**, and two live consumers
depend on that: the RN client, and `app.py:499`'s `_context_store.insert_task(steps=result.output)`.
Serving a `ModuleBrief` from this route forces `output` to become `AtomizerOutput | None`, which is a
**breaking change to focus mode's own response contract** — precisely the thing the brief's
"provably untouched" clause forbids. A feature that can only be added by loosening the type of the
path it must not disturb is being added in the wrong place.

**Revive trigger:** none while `/v1/respond` exists and carries `mode`. If `/v1/respond` were ever
retired, revisit against whatever replaces it.

### 2.4 KILLED — decompose only on the first build turn, gated on session state

The cost-minimizing option: one decompose attempt per session, gated on the F04 store holding no
brief yet.

**Why killed on two independent grounds.** (a) *It needs storage F03 does not own.* "Has this session
produced a brief?" requires persisting briefs — a new table, which is F05's (freeze protocol) or
F12's (freeze ledger) surface, not F03's. (b) **The premise is wrong.** A `clarify_request` early in
a dialogue is not waste — blueprint §2.1's own flowchart routes the fail-closed branch straight back
into the dialogue (`F["FAIL-CLOSED: emit ClarifyRequest"] --> DLG["back to dialogue (§3)"]`), and §3
builds the entire clarify loop on top of exactly that signal. Attempting a decompose every build turn
and getting a clarify request early **is the designed loop running**, not a failure to be optimized
away.

The cost is nonetheless real and is not waved away: a build turn can now cost up to 2 extra
cheap-model calls (`DECOMPOSER_MAX_TOKENS = 768`, one repair). §6.8 requires it to be **measured
against the session reservation and reported as a number**, not assumed acceptable.

**Revive trigger:** §6.8's measured decompose spend exceeds 25% of the ~₹4 session reservation on a
representative build session. Then the "when to decompose" policy becomes real work — and it belongs
to F05's FSM (`INTAKE → DECOMPOSE: goal captured`), which F04 §8.2 already recorded as unported.
Do not solve it here with a magic number.

### 2.5 KILLED — feed `decompose()` only the current turn's text

Simplest wiring: `decompose(body.text, ...)`.

**Why killed.** Build dialogue turns are frequently three words ("across sessions", "yes, Postgres").
Decomposing one of those into a whole `ModuleBrief` cannot succeed, so it would guarantee two wasted
model calls and a `clarify_request` on exactly the turns where the user just supplied real
information. `/v1/respond` already assembles the session's turns (`_conversation_messages`), so the
accumulated build-session text is available at the call site for free. See §3.2.

**Revive trigger:** none. This one is just a bug.

---

## 3. The interface — exactly what changes

Five files. No file is shared with any other open lane.

### 3.1 EDIT — `orb/backend/relay-py/src/orb_relay/proxy/lld_decomposer.py` (the blind-repair fix, §0.2)

Two changes, both inside the existing shape. Nothing about the pipeline's structure, its bound, or
its fail-closed target moves.

1. `_parse_and_validate` switches from `validate_module_brief` to F02's
   `validate_module_brief_detailed`, and builds its `DecomposeRejection.detail` from the returned
   error list — each entry rendered `"<path>: <message>"`, joined, in the validator's own order.
   Keep using `validate_module_brief` (or `ModuleBrief.model_validate`) to produce the typed object
   on the success path; the detailed validator is for the failure path's message only.
2. `DecomposeRejection.detail` is capped at a stated maximum length before it reaches
   `_repair_prompt`, so a brief failing 40 checks cannot push the repair prompt past
   `DECOMPOSER_MAX_TOKENS`' useful input budget. Cap value is the builder's call; it must be a named
   module constant with a comment, not an inline literal.

**Explicitly unchanged in this file:** `MAX_REPAIR_ATTEMPTS`, `DECOMPOSER_MAX_TOKENS`,
`DecomposeKind` (still exactly two outcomes), `DEFAULT_MISSING_ON_DECOMPOSE_FAILURE`, the
`clarify_request` fail-closed branch, `SYSTEM_PROMPT`, and the `GatewayError`-propagates behavior.

### 3.2 EDIT — `orb/backend/relay-py/src/orb_relay/app.py` (additive, inside F04's existing build branch)

The seam is `app.py:798`'s `if body.mode is ResponseMode.BUILD:` block — the branch F04 already
built. F03 extends it; it does not add a branch of its own.

- Import `decompose` / `DecomposeKind` from `.proxy.lld_decomposer`.
- Inside the BUILD branch only, call `decompose(...)` with the **accumulated build-session goal
  text**, not `body.text` (§2.5). The accumulation is the session's user turns in chronological
  order, joined — `_conversation_messages` already assembles the turns; derive the text from that
  same list so there is one source of truth for "what the session has said."
- Meter the call through the same session meter the turn already uses, via §3.4's extracted
  envelope. A `GatewayError` from the decompose call must **not** fail the whole turn: the
  conversational reply has already been produced and is what the user hears. Degrade to a null
  decompose result on the envelope and `dev_log` it at `warn`. This is a disclosed judgment call —
  a build turn that speaks but cannot decompose is strictly better than a 500.
- On `kind == "brief"`, append the derived Tier-0 Coverage evidence (§3.3) to
  `_build_session_store`, *before* `build_turn_state(...)` is rendered, so the registers on this
  turn's envelope reflect this turn's brief.
- Populate the new response field (§3.6).

**Explicitly unchanged in this file:** the entire `/v1/atomize` handler (`app.py:441-527`), every
non-BUILD branch of `/v1/respond`, `_MODE_PROMPT_NAMES`, `session_warmup`, and the
`_MISSING_MODE_PROMPTS` import-time exhaustiveness check F04 added.

### 3.3 NEW — the brief→evidence deriver, in `orb/backend/relay-py/src/orb_relay/build/build_session.py`

The only genuinely new logic in the lane, and it closes a hand-off F04 wrote down explicitly.
`build_session.py`'s module docstring says of its keyword-matching `derive_turn_evidence`:

> *"**Disclosed judgment call**: … real slot extraction is the atomizer/decomposer's job — out of
> scope here, §8.2 … a later lane with real slot extraction replaces this wholesale."*

F03 is that lane, for the half that is a field read. Add a sibling function — name it
`derive_brief_evidence` — that maps a **validated** `ModuleBrief` to `Evidence` entries:

| `ModuleBrief` field | Emits | Condition |
|---|---|---|
| `interface` | `Coverage[interface]` | non-empty (schema already guarantees ≥1) |
| `data_owned` | `Coverage[data_owned]` | non-empty |
| `acceptance` | `Coverage[acceptance]` | always (schema-required) |
| `deps` | `Coverage[deps]` | non-empty |
| `non_goals` | `Coverage[non_goals]` | non-empty |
| `registry` | `Coverage[registry_verdict]` | always (schema-required) |
| `alternatives` | `Alternatives` | always (schema floor is ≥2) |
| `guarantees` | `Depth` | always (schema floor is ≥1) |

All `EvidenceTier.TIER0` — this is a field read on an already-validated object: no model call, no
similarity, no clamp needed. Reuse `_TURN_EVIDENCE_WEIGHT` / `_TURN_EVIDENCE_RELIABILITY`; do not
introduce a second weight scale.

**Three hard constraints:**

1. **`derive_turn_evidence` is not replaced or modified.** It fires on every build turn from the
   user's text; `derive_brief_evidence` fires only when a brief validated. They coexist.
2. **Never emit `Contradiction` evidence.** Cross-turn claim comparison is F05's. A `ModuleBrief`
   from a single decode cannot contradict itself — the schema's cross-field validators already
   refuse that shape.
3. **A `clarify_request` emits nothing.** Stated as its own acceptance case (§6.7b) because it is
   the laundering risk: a failed decode that still moved coverage would let the registers report
   progress the dialogue did not make.

### 3.4 EXTRACT (bounded) — the reserve → call → settle/release envelope

`app.py` performs the same INV5 budget dance in two places today, and F03 would make it three.
Duplicating budget-admission code is how one path eventually forgets to settle.

Extract exactly the envelope — reserve, run, settle on success, release on `GatewayError`, 402 on
`ReservationExceededError` — as one module-level helper in `app.py`, and route all three call sites
through it.

**This is the one change in the lane that touches focus-mode code**, and it is a pure refactor. Its
safety is proved not by inspection but by §6.9's rule: **`tests/test_app_routes.py`'s existing
`/v1/atomize` cases must pass with zero edits.** If making them pass requires editing them, the
refactor changed behavior and must be reverted to a duplicated envelope instead.

If the builder judges the extraction cannot be done without changing `/v1/atomize`'s observable
behavior, **duplicate the envelope in the new call site and flag it** rather than editing a focus
test. An honest duplication beats a silently altered focus path.

### 3.5 UNTOUCHED — the mechanically-checked list

The lane base is **`6fd66f3`** (`git merge-base HEAD master`, verified with a clean tree). Diff
against that explicit hash — not against the lane branch, which compares the branch to itself and
prints nothing no matter what changed. (F04 §3.8 records an earlier draft getting this exact thing
wrong.)

```bash
cd "/Users/rachitsrivastava/youtube/Principal Engineering/Light/.worktrees/F03-atomizer"
git diff --numstat 6fd66f3...HEAD -- \
  orb/backend/relay-py/src/orb_relay/proxy/atomizer.py \
  orb/backend/relay-py/src/orb_relay/proxy/semantic_checks.py \
  orb/backend/relay-py/src/orb_relay/proxy/lld_schemas.py \
  orb/backend/relay-py/src/orb_relay/cognitive/belief.py \
  orb/backend/relay-py/src/orb_relay/store/build_session_store.py \
  orb/backend/relay-py/src/orb_relay/store/freeze_store.py \
  orb/backend/relay-py/src/orb_relay/eval/ \
  orb/domain/agents/focus-companion.v1.md \
  orb/domain/agents/converse.v1.md \
  orb/domain/agents/teach.v1.md \
  orb/domain/agents/build.v1.md \
  orb/apps/mobile/ orb/backend/relay-rs/ \
  apps/macos/ fleet/ registry/
# MUST print nothing. Run before writing code, and again against the committed hash.
```

`atomizer.py` and `semantic_checks.py` are the focus path (F04 §8.2 assigned them here and stayed
off them — F03 does the same). `lld_schemas.py` is F02's. `belief.py` / `build_session_store.py` are
F04's — F03 appends *through* the store's API and adds no field to the belief math.
`freeze_store.py` is F05/F12's. A diff in any of them is a lane collision, not a bonus.

### 3.6 EDIT — `orb/backend/relay-py/src/orb_relay/proxy/schemas.py` (additive only — the human-merge trigger)

One new model plus one new optional field, mirroring F04's own `BuildTurnState` idiom
(`app.py:363`: *"non-null iff mode is BUILD"*).

- `DecomposeOutcome` — the wire mirror of `DecomposeResult`, minus usage:
  `kind: Literal["brief","clarify_request"]`, `brief: ModuleBrief | None`,
  `missing: list[str]`, `rejections: list[str]`.
- `ConversationResponse.decompose: DecomposeOutcome | None`, non-null **iff** `mode is BUILD`.

**Additive only.** No existing field changes type, becomes optional, or is removed. An existing
caller that never sends `mode` sees exactly one new `null` field — the same compatibility bar F04
held itself to and stated in `app.py:384`.

**This file is why the lane is human-merge (A15/D4).** It is the wire contract two clients read.

---

## 4. The property this lane must make true

Stated once, plainly, so the done-definition has something to be checked against — and so nobody
substitutes a green test for it:

> A build-mode session, driven over the real relay by a client with no RN and no Swift involved,
> turns spoken goal text into a **validated `ModuleBrief`** on the response envelope; when the model
> cannot produce a valid one, the session receives a **`clarify_request` and never a fabricated
> brief**; and focus mode's decomposition path produces byte-identical results to `6fd66f3`.

Everything in §6 is an attempt to falsify that sentence. §7 step 3 drives it by hand, because a
green suite is not the property either.

---

## 5. Reuse inside this repo (do not reimplement)

| Need | Use | Do not |
|---|---|---|
| A valid `ModuleBrief` | `fleet/contracts/fixtures/lld/complete_module.json` | hand-author one; the schema has 16 fields and 3 cross-field validators |
| A brief invalid in exactly one named way | `fleet/contracts/fixtures/lld/one_alternative.json` (1 alternative vs the ≥2 floor) | mutate a valid fixture inline |
| A scripted gateway that records prompts | `tests/test_atomizer_pipeline.py`'s `ScriptedGateway` | write a third fake gateway |
| A gateway that raises | `tests/test_atomizer_pipeline.py`'s `RaisingGateway` | — |
| A real stub gateway process | `tests/manual/stub_gateway.py` (F04 §3.7) | — |
| Driving the real app in-process | `tests/test_app_routes.py:52`'s `client_with(gateway) -> TestClient` factory | build a `TestClient` by hand |
| The exact shape of T8a | `tests/test_app_routes.py:388`'s `test_atomize_checks_cost_before_gateway_call` — the existing precedent for "402 with zero gateway calls" | invent a new pattern |

The fixture corpus paths above were **executed, not assumed**, at contract time:
`validate_module_brief(complete_module.json)` returns a `ModuleBrief`;
`validate_module_brief_detailed(one_alternative.json)` returns `ok=False` with exactly one error,
`path='alternatives'`, message *"alternatives must have at least 2 entries (flat floor -- no grain
exemption, F02 §3.4)"*. That single observed error path is what makes T3's assertion real rather
than a hoped-for substring — if it ever stops being the observed path, T3 is measuring nothing and
must be re-derived before being trusted.

`ScriptedGateway` lives in a test module. Import it (`from test_atomizer_pipeline import
ScriptedGateway`) or lift it into a shared `tests/conftest.py` fixture — the builder's call, but
**do not copy-paste a third variant**; F04's contract and this one both depend on prompt-recording
behaving identically.

---

## 6. Acceptance suite — write this FIRST; it is the spec

**File: `orb/backend/relay-py/tests/test_f03_module_brief_decompose.py`** — flat `test_f03_*.py`,
matching the existing `test_f04_*.py` convention (there is no `tests/contract/` or
`tests/acceptance/` directory anywhere under `orb/`; do not invent one).

**Run:** `cd orb && npm run test:py` (→ `.venv/bin/pytest -q`). `asyncio_mode = "auto"` is already
set in `pyproject.toml`, so `async def` tests need no marker.

The nine cases below are the contract. Case IDs are load-bearing — keep them in the test names.

### 6.1 T1 — composition: a build turn produces a real `ModuleBrief` through the real app

Drive `POST /v1/respond` with `mode="build"` through `TestClient`, gateway scripted to return the
conversational reply and then `complete_module.json`.

- HTTP **200**.
- `body["decompose"]` is **non-null**, `["decompose"]["kind"] == "brief"`.
- `validate_module_brief(body["decompose"]["brief"])` returns **non-None** — the payload is
  re-validated through F02's own validator, not merely shape-checked by the test.
- `body["decompose"]["brief"]["node_id"]` equals the fixture's `node_id`. Not "is a string" — the
  actual value, so a stub returning an empty-but-valid brief fails.

This is the anti-orphan case: it fails today because no route can reach `decompose()`.

### 6.2 T2 — fail-closed: a garbage decode never yields a brief

Gateway returns valid conversational text, then unparseable garbage on both decompose attempts.

- HTTP **200** — a failed decode is a normal dialogue outcome, not a server error.
- `["decompose"]["kind"] == "clarify_request"`, `["brief"] is None`, `["missing"]` non-empty.
- **`"node_id" not in json.dumps(body["decompose"])`** — no brief-shaped object anywhere in the
  envelope, checked over the serialized response rather than one field, so a brief smuggled into a
  sibling key still fails.

### 6.3 T3 — the repair prompt names the actual violation (the §0.2 fix)

Gateway returns the conversational reply, then `one_alternative.json`, then `complete_module.json`.

- Exactly **2** decompose gateway calls.
- The **second** prompt contains the string `"alternatives"`.
- The second prompt does **not** consist only of the old constant: assert
  `"module brief failed lld.v1 schema validation" not in prompt`.
- Final `kind == "brief"`.

The fixture pair is the mechanism: `one_alternative.json` is invalid in exactly one named way, so
"the prompt named it" is a real assertion and not a substring coincidence.

### 6.4 T4 — repair is bounded at exactly one

Gateway returns the reply, then `one_alternative.json` **twice**.

- Exactly **2** decompose gateway calls — **not 3**. Assert the exact integer.
- `kind == "clarify_request"`.

### 6.5 T5 — focus mode untouched, asserted in **both** directions

Three parts. Part (a) alone is insufficient and the reason is recorded in memory as
*assert-the-retirement-not-just-the-replacement*: a test that only asserts the new engine ran stays
green through a dual-write regression where **both** engines run.

- **T5a** — `POST /v1/atomize` with the existing body shape: monkeypatch-spy both `atomize` and
  `decompose`; assert `atomize` called **once** and `decompose` called **zero** times; assert the
  response body's key set is byte-identical to `AtomizeResponse.model_fields`.
- **T5b** — `POST /v1/respond` with `mode="build"`: assert `decompose` called **once** and
  **`atomize` called zero times**. This is the direction that catches dual-run.
- **T5c** — structural: `subprocess` `git diff --numstat 6fd66f3...HEAD --` over
  `proxy/atomizer.py` and `proxy/semantic_checks.py`; assert **empty output**. Not a behavioral
  proxy — a proof that the focus decomposition code is the same bytes it was at the lane base.

### 6.6 T6 — mode stays explicit, never inferred

Pins F04's contract B4/C1 against F03's new branch.

- `mode="focus"` → `body["decompose"] is None`. A focus turn must not decompose.
- `mode` **omitted** → `body["decompose"] is None` (the default is FOCUS).
- `mode="banana"` → **422**, not 500, not a silent default.
- `mode="build"` → `body["decompose"]` non-null. All four in one test, so the contrast is the
  assertion.

### 6.7 T7 — the F04 composition: a brief moves the real registers; a clarify request moves nothing

Read the registers from the **store** (`_build_session_store.registers(...)`), not from the response
envelope — the envelope is F04's rendering of them, and asserting on it would test the renderer.

- **T7a** — before/after a successful build turn (fixture brief): `Coverage[interface]`'s
  `confidence` is **strictly greater** after. Same for `Coverage[acceptance]`. Assert strict
  inequality, not `> 0` — a register already nonzero from `derive_turn_evidence` would pass `> 0`
  without `derive_brief_evidence` existing at all.
- **T7b** — the laundering guard: on a turn whose decompose returns `clarify_request`, the store's
  log gains **no** `Coverage` entry attributable to a brief. Concretely: the log length after a
  clarify turn equals the log length after a `derive_turn_evidence`-only turn. A failed decode must
  never report coverage the dialogue did not earn.
- **T7c** — `Contradiction`'s `RegisterState` is **unchanged** across both (§3.3 constraint 2 —
  F05's register, not F03's).

### 6.8 T8 — budget admission is not bypassed, and the added spend is a measured number

- **T8a** — a session whose reservation is exhausted returns **402** with **zero** gateway calls on
  the decompose path. The new call site must sit behind the same INV5 admission as every other paid
  call; asserting zero calls is what proves it, not the 402.
- **T8b** — on a `GatewayError` from the decompose call specifically: HTTP **200** (the
  conversational reply still lands, §3.2), `body["decompose"] is None`, and `spent_paise` does not
  include a settled decompose — the reservation was released, not leaked.
- **T8c** — **record, do not merely assert**: capture `spent_paise` for one build turn with a
  successful decompose and for the same turn with decompose disabled. Write both numbers into the
  evidence file with the delta as a percentage of the ~₹4 session reservation. §2.4's revive
  trigger fires at >25%. This case's job is to produce a number, so the cost of §2.4's decision is
  observed rather than assumed.

### 6.9 Red on arrival vs. green-on-arrival locks — stated, not discovered

*"A new gate that's green on arrival is a broken gate"* (fleet rules §7) — so every green-on-arrival
case must justify itself as a **lock** rather than be quietly counted as a pass.

| Case | On arrival | Why |
|---|---|---|
| T1 | **RED** | no route reaches `decompose()` — the orphan |
| T2 | **RED** | `["decompose"]` does not exist on the envelope |
| T3 | **RED** | the repair prompt is the blind constant (§0.2) |
| T4 | **RED** | envelope does not exist (the ≤1-repair bound itself already holds in the module) |
| T5a | **GREEN — lock** | focus path is correct today; this pins it through §3.4's refactor. Its value is at the *end* of the lane, not the start. |
| T5b | **RED** | `decompose` is never called from anywhere |
| T5c | **GREEN — lock** | trivially true at the lane base; the whole point is that it must still print nothing at the *committed hash* |
| T6 | **PARTIAL** | the 422 and default-FOCUS halves are green locks (F04 shipped them); the `mode="build"` → non-null half is red |
| T7 | **RED** | `derive_brief_evidence` does not exist |
| T8a | **RED** | no decompose call site exists to admit or refuse |
| T8b | **RED** | same |
| T8c | **RED** | produces no number today |

**Before trusting the suite, prove the reds are red.** Run the full file against `6fd66f3` *before*
writing any production code and paste the real failure list into the evidence file. A suite whose
reds were never observed red is a suite nobody has checked.

### 6.10 Mutation table — required before claiming green

Each row: apply the mutation, record the **real failure text**, revert, re-confirm green. A row
whose mutation does not turn a test red means that test is not testing what it claims.

| # | Mutation | Must fail |
|---|---|---|
| M1 | In §3.2's branch, call `decompose` on **every** mode, not just BUILD | T6 (focus turn decomposes) |
| M2 | Revert `_parse_and_validate` to `validate_module_brief` + the constant string | T3 |
| M3 | Change `MAX_REPAIR_ATTEMPTS` to 2 | T4 (call count 3, not 2) |
| M4 | Make the fail-closed branch return `complete_module.json` as a "best-effort" brief | T2 |
| M5 | Call `derive_brief_evidence` on the `clarify_request` path too | T7b — **the laundering mutation; the single most important row** |
| M6 | Move the decompose call *before* the budget reservation | T8a (gateway called despite 402) |
| M7 | Have the BUILD branch also call `atomize()` and discard the result | **T5b only** — M7 is the dual-write regression T5a cannot see. If T5b does not go red here, T5's spies are wired wrong. |
| M8 | Emit `Coverage[interface]` evidence with `interface` empty | T7a, if the fixture is swapped for an empty-interface brief — **or** the builder records honestly that M8 is unreachable because the schema's `min_length=1` already forbids it (a `kills (structural)` result, which is a legitimate outcome, not a skipped row) |

---

## 7. How the orchestrator drives this end to end (NOT optional)

A green pytest run is a proxy. §4's property is about a real process answering a real client.

- **Step 0** — venv per F04 §6 step 0 (same worktree-local `.venv` procedure).
- **Step 1** — terminal A: `tests/manual/stub_gateway.py`, scripted to serve a conversational reply
  then `complete_module.json`.
- **Step 2** — terminal B: the real `uvicorn orb_relay.app:app` process.
- **Step 3** — terminal C: `curl` `POST /v1/respond` with `mode="build"` — **a raw HTTP client, no
  RN and no Swift**. Paste the real response. Confirm by eye that `decompose.brief.node_id`,
  `interface`, and `acceptance` carry the fixture's actual content.
- **Step 4** — same session, second turn, stub now serving garbage: confirm `kind ==
  "clarify_request"`, `brief` null, and that the spoken reply still came back.
- **Step 5** — `curl POST /v1/atomize` against the same live process with the pre-F03 body: paste
  the response and confirm it is shaped exactly as before. **Focus mode driven, not assumed.**
- **Step 6** — the gate: `cd orb && npm run test:py` (all pre-existing relay-py tests + the new
  file), plus §3.5's `git diff --numstat` against the **committed** hash.

---

## 8. Done-definition

F03 is done when **all** hold, each with pasted real output:

1. §6's T1–T8 pass, and the pre-existing relay-py suite still passes with **zero regressions** and
   **zero edits to any existing test file** (§3.4's rule).
2. §6.9's red-on-arrival list was **observed red at `6fd66f3` before any production code was
   written**, and the real failure list is in the evidence file.
3. §6.10's mutation table has been run: each mutation's real failure text recorded, reverted, green
   re-confirmed. **M5 and M7 especially** — they are the two that catch the failure modes this
   lane's design is most exposed to.
4. §3.5's `git diff --numstat` prints **nothing**, run against the committed hash.
5. §7 steps 3, 4 and 5 driven on two real live processes, with the actual terminal output pasted —
   including step 5, focus mode driven against the same running server.
6. T8c's two spend numbers and their delta are recorded, with the §2.4 revive trigger evaluated
   explicitly (fired / did not fire).
7. `docs/evidence/F03-module-brief-atomizer.md` exists with real command output, the real net-new
   line count (not an estimate), and every judgment call disclosed — including §3.2's
   degrade-on-`GatewayError` choice.
8. `status/F03-Module-brief-atomizer.status` → `verifying`, then `verified` by an **independent**
   verifier who re-derived rather than re-read (A17: verifier is Sonnet, never Haiku, never the
   builder).
9. Committed to `lane/F03-atomizer`, **not merged**. Author ≠ integrator. This lane edits
   `proxy/schemas.py`, a wire-contract surface → **human-merge (A15/D4). Ship to a PR and stop.**
10. A dated FLEET-LEARNINGS entry at
    `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md` — **that exact
    absolute path, never one containing `Light/`.**

---

## 9. Explicitly OUT of scope

- **F05 — the freeze protocol.** propose→pushback→❄, the `ClarifyRequest` *conversation* (F03 emits
  the signal; F05 decides what the orb says next), the information-gain stopping rule, `ε_gain` /
  `θ_cov` / `θ_amb`, `FREEZE_ELIGIBLE`, the split trigger, `Contradiction` evidence, and any
  storage of briefs across turns. **F03 emits a `DecomposeOutcome` per turn and stores nothing.**
- **F09 — the orb→fleet handoff.** F03 calls no fleet binary, emits no SOW, opens no PR, and writes
  nothing to `fleet/`.
- **F06 — the `lld-ready` gate and `depth()`.** Already ☑. F03 never computes a depth verdict and
  never sets `depth_score`. Note the collision hazard F04 §8.2 already named: F04's `Depth`
  *register* and F06's `depth()` *predicate* share a word and are different objects. §3.3 emits the
  register; it must not be read as running the predicate.
- **F02 — the `lld.v1` schema.** F03 imports it and adds no field. `lld_schemas.py` is on §3.5's
  untouched list.
- **F12 — the freeze ledger as a session object.** `freeze_store.py` is untouched.
- **The build FSM.** Still unported server-side (F04 §8.2). F03 does not port it, and §2.4's
  when-to-decompose policy is deferred to it rather than approximated with a threshold.
- **Real NLU slot extraction from free conversational text.** `derive_turn_evidence`'s keyword
  stand-in **stays**. §3.3 reads fields off an already-validated object — that is a field read, not
  NLU, and must not be described as having replaced the stand-in.
- **Multi-brief decomposition.** `decompose()` returns exactly one `ModuleBrief`. One goal → N
  modules is not F03.
- **Any change to `atomize()`, `FALLBACK_STEP`, `semantic_checks.py`, or the focus prompts.**
- **The fake eval gates.** F04 §1.1 flagged them; still flagged, still not fixed here.
- **Swift-side work.** `apps/macos` reads the new field or does not; F03 ships the server contract.

---

## 10. Landmines already paid for — do not rediscover

- **`git stash` is one shared stack across every worktree of a repo.** Cost three separate
  incidents last session. For a baseline diff use `git worktree add --detach /tmp/f03-base 6fd66f3`.
- **Diff against the explicit base hash, not the lane branch.** `git diff HEAD...HEAD` prints
  nothing regardless of what changed — F04 §3.8 caught this in its own draft.
- **A subagent cannot receive its own background children's notifications.** Run every command in
  the foreground, or with `timeout N <cmd>`.
- **`Light/FLEET-LEARNINGS.md` is the wrong file.** The real one is at the repo-parent absolute
  path in §8.10. A prior worker lost an entry to exactly this.
- **The bare `build/` gitignore rule** swallows `orb/apps/mobile/src/build/`; the negation is
  already in this copy's `.gitignore`. Don't remove it. (F03 writes no files there, but
  `orb_relay/build/` is a *Python* package — confirm `git status` actually shows §3.3's edit before
  assuming it was committed.)
- **A fresh worktree has no `.venv`.** `npm run test:py` fails outright until it exists — that is
  environment, not a red suite.
- **`grep` in the agent Bash tool is `ugrep`,** with different regex semantics than the
  `/usr/bin/grep` a script sees. Probe the way the script will run.
