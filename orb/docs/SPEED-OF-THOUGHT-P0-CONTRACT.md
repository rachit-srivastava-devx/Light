# Speed-of-Thought P0 — Orb-side contract (frozen)

> **Status:** CONTRACT. Written before implementation (L8 / T1). Builders implement against this;
> builders may **not** edit the acceptance tests named in §6 — a diff that touches them is flagged
> and rejected at review.
>
> **Scope:** the Orb half of Speed-of-Thought P0 (`blueprints/Speed-of-Thought-L8-Deep-Dive/PHASES-TO-USABLE.md`
> §2), terminating at a **frozen `lld.v1` artifact on disk**. Nothing downstream of that artifact is
> in scope — see §7.
>
> **Read first:** [`../../../FLEET-LEARNINGS.md`](../../../FLEET-LEARNINGS.md) (hard boundaries),
> [`../SOW-FLEET-2026-09-02.md`](../SOW-FLEET-2026-09-02.md) (Track B), `AGENTS.md`, `CLAUDE.md`.
>
> **Registry decision (C1/L2), stated out loud:** this slice is **extract + build-new**, not install.
> *Extract* — the FSM reducer shape, the clarify decision shape, and the decode→validate→repair-once→
> fail-closed pipeline shape already exist in this repo and are lifted, not rewritten (§1).
> *Build-new* — the `lld.v1` schema, the depth gate, and the freeze ledger have no prior art here or
> in `registry/` (grepped: no `lld`, `freeze`, or `depth` capability). *Nothing* is installed from
> `registry/`: no existing service emits or validates a design artifact.
>
> All current-state facts below are `[M]`, read from the tree on **2026-09-02**. File paths are
> contracts; **line numbers are pointers and will drift**.

---

## 1. Reuse map — what already exists, and what "re-point to module briefs" means as a diff

Every row was read from source, not from a doc claim.

| Blueprint says | The real thing `[M]` | What it actually is | Class |
|---|---|---|---|
| "the 9-state FSM" | `apps/mobile/src/session/StateMachine.ts` + `apps/mobile/src/session/contracts.ts` | `SessionState` (9 literals: `IDLE_PRESENT · INTAKE · CLARIFY · STEP_PRESENT · WORKING · CHECK_IN · STEP_DONE · INTERRUPTED · SESSION_DONE`), `SessionEvent` (17 literals), `TransitionGuardId` (9), a `TransitionTable` **data** table `SESSION_TRANSITION_TABLE`, and a pure reducer `dispatch(current, event, ctx)` with three edge shapes (`to` / `branch` / `resume_prior`). No wall-clock, no randomness. | **reuse the shape, not the table** (§4) |
| "`ConversationPort`" | `apps/mobile/src/runtime/ConversationPort.ts` | `interface ConversationPort { respond(input: ConversationPortInput): Promise<ConversationPortResult> }`, plus `createRelayConversationPort` (POSTs `/v1/respond` on relay-py) and `createStaticConversationPort` (the degrade path). Composition root: `apps/mobile/src/App.tsx:135`. Carries `mode?: OrbMode` on the wire. | **reuse as-is; one new `mode` value** (§1.3) |
| "the atomizer" | `backend/relay-py/src/orb_relay/proxy/atomizer.py` (`atomize()`), schema mirror `apps/mobile/src/shared/atomizer-schema.ts` | `structured-decode → _parse_and_validate (schema + `check_step_atomic` semantic rubric) → _repair_prompt → one retry → fail-closed to `FALLBACK_STEP``. `MAX_REPAIR_ATTEMPTS = 1`. Output type `AtomizerOutput{steps: AtomizerStep[], steps_total: 1..12}`. | **re-point the output type** (§1.2) |
| "the clarifier" | `apps/mobile/src/lld/ClarifyProtocol.ts` | `extractSlots(utterance, existing) → SlotState` (4 regex-driven slots), `decideClarify(slots, questions_asked) → {ask, slot} \| {proceed, reason}`, `MAX_CLARIFY_QUESTIONS = 2`, `REQUIRED_SLOTS = ['scope','first_context']`. Pure, no model call decides the slot. | **reuse the decision shape, new slot set** (§1.4) |
| "`conversation_store`" | `backend/relay-py/src/orb_relay/store/conversation_store.py` | `class ConversationStore` over sqlite: `append(...)`, `recent(...)`, `has_own_turns(...)`; `MAX_CONVERSATION_TURNS = 24`; `CARRY_OVER_WINDOW_S = 1800`; scoped by `(tenant_id, user_id, session_id)`; `now` is injected, not read. | **reuse verbatim for L1 transcript; new sibling store for the freeze ledger** (§1.5) |
| "the response envelope" | `apps/mobile/src/lld/ResponseEnvelope.ts` (+ `apps/mobile/src/router/contracts.ts`'s `ModedResponseEnvelope`) | `ResponseEnvelope{v:1, session_id, turn_id, seq, speech?, orb, widgets?, session, meta}`, must-ignore-unknown-fields. `ModedResponseEnvelope extends ResponseEnvelope { mode?: OrbMode }` — the established pattern for extending the envelope **without editing it**. | **extend by superset interface, never edit** (§1.6) |
| "belief model" | `apps/mobile/src/cognitive/BeliefModel.ts` | `initialBeliefs()`, `applyEvidence(...)`, `decay(...)`, `DEFAULT_DECAY_TABLE`. Log-odds over user registers with wall-clock decay. | **OUT of P0** — see §7 |

### 1.1 The one thing the blueprint got wrong about this tree

There is **no `StepGate`-style "atomizer port" on the client that owns the pipeline** — the pipeline
lives in Python (`atomizer.py`) and the client holds only a *shape mirror*
(`shared/atomizer-schema.ts`) and a thin `runtime/AtomizerPort.ts`. So "re-point the atomizer" is
**two coordinated diffs in two languages**, not one. This is why U1 (§6) ships the schema in both
languages from one JSON Schema file rather than adding a third hand-maintained copy — the exact
warning `shared/atomizer-schema.ts`'s own header gives.

### 1.2 "Re-pointing the atomizer to module briefs", diff-shaped

`atomize()` is **not edited.** A sibling function is added in the same module family, reusing the
same four private helpers:

```
NEW  backend/relay-py/src/orb_relay/proxy/lld_decomposer.py
       async def decompose(goal, *, gateway, tenant_id) -> DecomposeResult
       - reuses: _strip_markdown_fence, _repair_prompt shape, MAX_REPAIR_ATTEMPTS = 1
       - new SYSTEM_PROMPT: emits ModuleBrief JSON (lld.v1 §2), not physical steps
       - new validator: validate_module_brief(dict) in proxy/lld_schemas.py
       - THE FAIL-CLOSED TARGET MOVES: there is NO ModuleBrief equivalent of FALLBACK_STEP.
         On second failure it returns DecomposeResult(kind='clarify_request', missing=[...]).
UNCHANGED  proxy/atomizer.py, proxy/schemas.py's AtomizerOutput, shared/atomizer-schema.ts
```

**Why a sibling and not a generic:** `atomize()`'s fail-closed branch returns a `FALLBACK_STEP` —
correct for focus (a wrong step costs the user 30s) and **catastrophic for build** (a guessed
`ModuleBrief` costs a whole lane). A single function parameterized over output type would have one
fail-closed branch with two required behaviours. Splitting the function is what makes
"no code path from an invalid decode to a frozen brief" true by construction rather than by a flag.

Killed alternatives: (a) *generify `atomize()` over an output schema* — killed, one fail-closed
branch cannot serve both targets; revive if a third decomposer appears and the fail-closed
behaviours turn out to coincide. (b) *do the decode client-side in TS* — killed, the client has
never validated model output (`shared/atomizer-schema.ts` header: "the client never validates a
model's output"); moving that boundary for one feature breaks the estate's one-door gateway rule
(C9); revive never.

### 1.3 `ConversationPort` re-point, diff-shaped

```
EDIT apps/mobile/src/router/contracts.ts
       type OrbMode = 'converse' | 'focus' | 'teach'          →  | 'build'
EDIT backend/relay-py/src/orb_relay/proxy/schemas.py
       class ResponseMode(...): CONVERSE/FOCUS/TEACH          →  + BUILD = "build"
UNCHANGED apps/mobile/src/runtime/ConversationPort.ts  (already forwards `mode` verbatim)
```

Both edits ship in **one unit** (U4). The scar this obeys is in this repo's own memory: the `mode`
field was computed everywhere and transmitted nowhere for weeks, and `teach`/`converse` were
unreachable from the real app while fully built and unit-tested. The acceptance test for U4
therefore asserts on **the request body that leaves the client**, not on a computed local value.

### 1.4 `ClarifyProtocol` re-point, diff-shaped

```
NEW apps/mobile/src/build/BuildClarifyProtocol.ts   (ClarifyProtocol.ts is NOT edited)
      type BuildSlot = 'interface' | 'data_owned' | 'acceptance' | 'deps' | 'non_goals' | 'registry_verdict'
      REQUIRED_BUILD_SLOTS = ['interface','data_owned','acceptance','registry_verdict']
      SPLIT_TRIGGER_TURNS = 12          // NOT a stop — a forced "this is two modules" proposal
      decideBuildClarify(slots, turns_taken) -> {ask, slot} | {propose_split} | {proceed}
```

**The cap inverts, and that is the whole point.** Focus caps at 2 and then *proceeds* (protecting
momentum). Build has no proceed-on-cap: at `SPLIT_TRIGGER_TURNS` it emits `propose_split`, because a
module needing >12 questions is over-scoped for the reference shape (≤400 lines / ≤8 files), not
under-explained. `decideBuildClarify` must be pure and must never return `proceed` on a turn count —
only on slots. **P0 uses required-slot ordering, not information-gain (§7).**

### 1.5 L1 working memory, diff-shaped

```
UNCHANGED backend/relay-py/src/orb_relay/store/conversation_store.py   (used as-is for the transcript)
NEW       backend/relay-py/src/orb_relay/store/freeze_store.py
            class FreezeStore  — APPEND-ONLY sqlite, deliberately modelled on ConversationStore:
              - same (tenant_id, user_id, session_id) scoping
              - same injected `now` (no clock read inside) so replay is bit-identical
              - NO prune (ConversationStore's _PRUNE_SQL is NOT copied — a freeze ledger that
                forgets its 25th freeze destroys the record of why a spec changed)
              - NO update/delete method exists on the class at all
            append_freeze(...) / list_freezes(...) / get_by_content_hash(...)
```

`FreezeStore` having no setter for `superseded` and no `UPDATE` statement is the structural half of
"you cannot edit a freeze in place"; supersede is expressed as a **new row whose `supersedes` names
the prior `freeze_id`**.

### 1.6 Envelope extension, diff-shaped

```
NEW  apps/mobile/src/build/BuildEnvelope.ts
       interface BuildResponseEnvelope extends ResponseEnvelope { readonly build?: BuildBlock }
UNCHANGED apps/mobile/src/lld/ResponseEnvelope.ts
```

Same superset pattern `ModedResponseEnvelope` already uses, and for the same stated reason: the
envelope is a frozen surface other worktrees compile against.

---

## 2. `lld.v1` — the frozen artifact

Canonical definition ships as **one JSON Schema**, `contracts/lld.v1.json`, from which the TS types
in `apps/mobile/src/build/lld-v1.ts` and the Python validator in
`backend/relay-py/src/orb_relay/proxy/lld_schemas.py` are generated or hand-mirrored with a
conformance test (§6 U1-T3). TypeScript shown here is the normative reading.

```typescript
// contracts/lld.v1.json  →  apps/mobile/src/build/lld-v1.ts
export const LLD_SCHEMA_VERSION = '1.0' as const;

/** A module is `module` grain; a ≤40-line single-path helper may declare `leaf` (03 §scars). */
export type Grain = 'module' | 'leaf';
export type RegistryVerdict =
  | { readonly kind: 'install';    readonly matched_path: string }
  | { readonly kind: 'extract';    readonly matched_path: string }
  | { readonly kind: 'build_new';  readonly searched: readonly string[] };  // MUST be non-empty

/** R17: a claim carries its working, or it names what it makes unrepresentable. Nothing else. */
export type Derivation =
  | { readonly kind: 'number';     readonly value: string; readonly calc: string; readonly source?: string }
  | { readonly kind: 'structural'; readonly invariant: string; readonly enforced_by: string };

export type GuaranteeLabel = 'kills_structural' | 'kills_mechanical' | 'mitigates';
export interface Guarantee    { readonly claim: string; readonly derivation: Derivation; readonly label: GuaranteeLabel; }
export interface KilledAlt    { readonly option: string; readonly why_killed: string; readonly revive_trigger: string; }
export interface FailureStory { readonly trigger: string; readonly blast_radius: string; readonly fail_safe: string; }
export interface InterfaceDecl { readonly name: string; readonly signature: string; }
export interface DataDecl      { readonly store: string; readonly owned_by_node: string; }  // owned_by_node MUST === node_id
export interface AcceptanceLine {
  readonly given: string; readonly when: string; readonly then: string;
  readonly oracle_kind: 'test' | 'property' | 'metric';
  /** A path this module produces. The predicate must reference it (§3 C3). */
  readonly artifact: string;
}

/** The proposer-authored half. Everything here is content; none of it is authority. */
export interface ModuleBrief {
  readonly node_id: string;               // ^[a-z0-9][a-z0-9-]{2,63}$
  readonly grain: Grain;
  readonly purpose: string;               // 1..200 chars
  readonly owner: string;                 // MUST resolve in contracts/owners.v1.json (§3 C2)
  readonly owner_path: string;            // the ONE repo path prefix this module owns
  readonly interface: readonly InterfaceDecl[];
  readonly data_owned: readonly DataDecl[];
  readonly deps: readonly string[];       // other node_ids, interface-only
  readonly registry: RegistryVerdict;
  readonly acceptance: AcceptanceLine;
  readonly non_goals: readonly string[];
  readonly open_questions: readonly string[];   // MUST be [] to freeze
  readonly guarantees: readonly Guarantee[];
  readonly alternatives: readonly KilledAlt[];  // ≥2 at module grain, ≥1 at leaf
  readonly failure_story: FailureStory;
}

/** The gate-authored half. NO proposer-emittable form exists — see §2.1. */
export interface FreezeRecord {
  readonly schema_version: typeof LLD_SCHEMA_VERSION;
  readonly freeze_id: string;             // ^fz-[0-9a-f]{16}$
  readonly version: number;               // ≥1, monotonic per node_id
  readonly node_id: string;
  readonly content_hash: string;          // ^[0-9a-f]{64}$ — blake3(canonical_json(ModuleBrief))
  readonly decision: string;              // == brief.purpose, canonicalised
  readonly why: string;
  readonly killed_alternatives: readonly KilledAlt[];   // minItems 2 (module) / 1 (leaf)
  readonly acceptance_test: AcceptanceLine;
  readonly owner: string;
  readonly depth_score: DepthScore;
  readonly supersedes: string | null;     // prior freeze_id, or null for v1
  readonly stamped_by: 'orb:lld-ready';   // const — no proposer path writes this string
}

/** REPORTED, NEVER THE PASS CRITERION. See §3.4. */
export interface DepthScore {
  readonly checks_total: number;
  readonly checks_passed: number;
  readonly ratio: number;                 // checks_passed / checks_total, 3dp
  readonly failed_check_ids: readonly string[];
}

export type LldV1 = { readonly brief: ModuleBrief; readonly freeze: FreezeRecord };
```

### 2.1 What is absent by design

`ModuleBrief` — the only object a proposer (the decomposer, the dialogue, an LLM) may construct —
has **no `content_hash`, no `freeze_id`, no `depth_score`, no `stamped_by`, no state field.** They
are not optional; they are not in the type. A proposer cannot express "this is frozen." The gate
constructs `FreezeRecord`, and `stamped_by` is a `const` literal. This is the `submission.v1`
absent-by-design pattern and it is what makes a hand-authored freeze structurally impossible rather
than merely linted. `kills (structural)`.

`freeze_bytes` = `canonical_json(brief)` only. Narration, transcript, register, and timing are **not
inputs**, so two sessions reaching the same decisions produce the same `content_hash` (U1-T4).

---

## 3. The `lld-ready` gate — deterministic pass/fail rules

Signature (pure; no I/O, no clock, no network, no model — enforced by U3-T5):

```typescript
export function lldReady(brief: ModuleBrief, refs: GateRefs): GateVerdict;

export interface GateRefs {                      // checked-in JSON, loaded by the caller, passed in
  readonly owners: readonly string[];            // contracts/owners.v1.json
  readonly registry_paths: readonly string[];    // contracts/registry-index.v1.json
  readonly known_node_ids: readonly string[];    // node_ids already in the freeze ledger
}
export type GateVerdict =
  | { readonly outcome: 'READY';           readonly checked: number; readonly score: DepthScore }
  | { readonly outcome: 'NOT_READY';       readonly checked: number; readonly score: DepthScore;
      readonly reasons: readonly GateReason[] }
  | { readonly outcome: 'MEASURED_NOTHING'; readonly checked: 0 };   // a refusal, never a pass
```

### 3.1 The checks. All mandatory. All must pass.

| id | Rule (exact) | Fails when |
|---|---|---|
| `C1-OPEN` | `brief.open_questions.length === 0` | any open question remains |
| `C2-OWNER` | `refs.owners.includes(brief.owner)` | owner is a string not in `owners.v1.json` (`"me"`, `"team"`, a name) |
| `C3-ACC-PARSE` | `given`, `when`, `then` each `.trim().length ≥ 3`; `oracle_kind ∈ {test,property,metric}` | any is blank or a placeholder |
| `C3-ACC-GROUND` | `brief.acceptance.artifact` starts with `brief.owner_path` **and** `then` contains the `artifact` substring | the predicate names no artifact this module produces |
| `C3-ACC-NONTAUT` | `then !== given` and `then` is not in `TAUTOLOGY_DENYLIST` (`/^(it )?works?$/i`, `/^(is )?correct$/i`, `/^(it )?should work/i`, `/^passes( the)? tests?$/i`, `/^no errors?$/i`) | acceptance is prose that asserts nothing |
| `R17-DERIV` | `brief.guarantees.length ≥ 1` **and** every `g`: `kind==='number' ⇒ calc.trim() ≠ ''` and `calc` matches `/[0-9]/` and contains one of `+ - * / ÷ × = ≈ %`; `kind==='structural' ⇒ enforced_by.trim() ≠ ''` and `enforced_by` matches `/[A-Za-z0-9_\-\/.]+\.(ts|tsx|py|rs|json)(:[0-9]+)?$/` or `^type:` or `^gate:` | a number with no arithmetic in its `calc`; a structural claim naming no file/type/gate |
| `R19-ABSOLUTE` | for every `g` whose `claim` matches `/\b(zero|never|impossible|cannot|no way)\b/i`: `g.label === 'kills_structural'` **and** `g.derivation.kind === 'structural'` | an absolute claim backed by "we're careful" or by a number |
| `R21-ALTS` | `brief.alternatives.length ≥ (grain === 'leaf' ? 1 : 2)`; every alt has `option`, `why_killed`, `revive_trigger` all `.trim() ≠ ''`; `revive_trigger` is **not** in `NON_TRIGGER_DENYLIST` (`/^never$/i` is ALLOWED, `/^(tbd|n\/a|none|-|\?)$/i` is not); all `option` values distinct after casefold | fewer than the floor, or a padded duplicate alternative, or a placeholder trigger |
| `R21-FAIL` | `failure_story.trigger`, `.blast_radius`, `.fail_safe` each `.trim().length ≥ 10` | a one-word or empty failure story |
| `C12-STORE` | every `d ∈ data_owned`: `d.owned_by_node === brief.node_id`; no two briefs in the ledger declare the same `d.store` | a brief names another node's store |
| `C12-DEPS` | every `dep ∈ deps` is in `refs.known_node_ids`; `deps` does not contain `brief.node_id`; no `dep` string appears in any `data_owned[].store` | a dangling dep, a self-dep, or a store smuggled through `deps` |
| `REG-VERDICT` | `kind==='install'\|'extract' ⇒ refs.registry_paths.includes(matched_path)`; `kind==='build_new' ⇒ searched.length ≥ 1` and every entry is in `refs.registry_paths` | a dangling registry match, or a `build_new` that never searched |
| `IFACE` | `brief.interface.length ≥ 1`; every `signature` contains `(` and `)` **or** starts with `type ` / `interface ` | an interface declared in prose |
| `SHAPE` | `node_id` matches `^[a-z0-9][a-z0-9-]{2,63}$`; `purpose` 1..200 chars; `owner_path` non-empty and contains no `..` | malformed identity fields |

`checked` = the number of check ids evaluated (currently **14**). If `checked === 0` the verdict is
`MEASURED_NOTHING`, a refusal — a `READY` with a zero denominator is not expressible.

### 3.2 "Below the depth bar", in checkable terms

A freeze record is **below the depth bar** iff any of `R17-DERIV`, `R19-ABSOLUTE`, `R21-ALTS`,
`R21-FAIL`, `C3-ACC-GROUND`, `C3-ACC-NONTAUT` fails. Concretely, each of these is a real one-line
freeze this gate must refuse:

```
"acceptance": {"given":"a user","when":"they use it","then":"it works","oracle_kind":"test"}   → C3-ACC-NONTAUT
"guarantees": [{"claim":"fast","derivation":{"kind":"number","value":"fast","calc":""}}]        → R17-DERIV
"guarantees": [{"claim":"zero data loss","label":"mitigates",...}]                              → R19-ABSOLUTE
"alternatives": [{"option":"redis","why_killed":"no","revive_trigger":"tbd"}]                   → R21-ALTS ×2
"failure_story": {"trigger":"bug","blast_radius":"","fail_safe":"retry"}                        → R21-FAIL
"owner": "me"                                                                                   → C2-OWNER
```

### 3.3 Fixtures the gate ships with (both required; the gate does not run without them)

- `apps/mobile/src/build/fixtures/one_line_freeze.json` — **must be refused.** Authored to trip
  ≥5 distinct check ids at once.
- `apps/mobile/src/build/fixtures/complete_module.json` — **must be accepted**, driving `checked` to
  its full count. This fixture exists because *a gate whose precondition no input can satisfy is an
  outage, not a safeguard*.
- `fixtures/AUTHORSHIP.md` records, per fixture, `independent | self-authored`. The bad fixture must
  be written by a different unit's builder than the one who writes `LldReadyGate.ts` (U1 authors
  fixtures; U3 authors the gate). A self-authored bad fixture is *unverified*, and unverified is
  reported, not assumed.

### 3.4 `depth_score` is reported and is never the criterion

`ratio` is emitted on every verdict so a human can see *how far* a brief is from the bar, and so
trigger-rate can be measured later. It is **not** a threshold: there is no `ratio ≥ 0.8 ⇒ READY`
path, and U3-T4 asserts that a brief at `ratio = 13/14` is `NOT_READY`. A score threshold would let
a proposer trade a real acceptance test for two padded alternatives — the cheapest green would stop
being the correct fix.

### 3.5 The honest limit, stated here not buried

The gate proves the brief is **structured to L8 shape**. It does not prove the content is deep: a
`calc` can be present and arithmetically wrong; an `enforced_by` can name a real file that enforces
nothing; an alternative can be real-shaped and shallow. So `lldReady` **`kills (structural)` the
*absence* of depth** — the 90% failure mode, the generic answer with no derivation, cannot be
represented — and only **`mitigates` the *invalidity* of depth**, which is left to human review of
the freeze record. Any claim that this gate verifies depth *truth* would be the exact overclaim this
repo's `AGENTS.md` anti-slop rules reject.

Two P0-specific limits, named so they are not a surprise:
- **No blast-radius conjunct.** The blueprint's `blast_radius == graph.impact(interface)` check needs
  a tree-sitter code graph, which does not exist Orb-side. Including it would refuse every brief
  (the outage-not-safeguard failure). Deferred with the graph; **not silently dropped.**
- **No metric-registry resolution for `revive_trigger`.** There is no metric registry in this repo.
  P0 uses the denylist in `R21-ALTS`, which is weaker; recorded as `mitigates`.

---

## 4. Build-mode state addition to the FSM

**Decision: a second, parallel FSM built on an extracted generic core — the 9-state focus table is
not edited.**

```
NEW apps/mobile/src/build/fsm-core.ts
      Transition<S,E,G> / TransitionTable<S,E,G> / makeDispatch(table, evaluateGuard)
      — the same three edge shapes and the same no-op semantics as session/StateMachine.ts,
        parameterised. Includes the MissingGuardContextError discipline for `branch` edges.
NEW apps/mobile/src/build/contracts.ts        (BuildState, BuildEvent, BuildGuardId)
NEW apps/mobile/src/build/BuildStateMachine.ts (BUILD_TRANSITION_TABLE + buildDispatch)
UNCHANGED apps/mobile/src/session/{contracts,StateMachine}.ts
```

**Why not add states to `SessionState`.** `SessionState`/`SessionEvent`/`TransitionGuardId` are
closed unions consumed by exhaustive `switch` statements with `const _exhaustive: never` in
`StateMachine.ts` (`evaluateGuard`, `dispatch`) and by `ResponseEnvelope.SessionBlock`,
`AppModel.ts`, and `router/contracts.ts`'s `RouteFn`. Adding seven literals turns every one of those
into a compile error across files owned by other concurrent workers, and — worse — puts build states
inside the focus FSM's legal reach, where `bed_faded` and `policy_intervention` would become
dispatchable against them. A parallel table is the smaller and safer diff.

*Killed alternative (a): generify `session/contracts.ts` in place and have both tables use it.* This
is the better end state and costs ~40 duplicated reducer lines to skip. Killed for P0 because
`session/contracts.ts` is a frozen surface that parallel worktrees compile against and this repo has
162 uncommitted files right now. **Revive trigger:** the focus FSM has no uncommitted edits for one
full session — then migrate `StateMachine.ts` onto `fsm-core.ts` and delete the duplication (a C2
extract). *Killed alternative (b): one table, one union, a `mode` discriminant on every guard.* Every
guard would take a mode it ignores, and an illegal focus↔build edge becomes a runtime guard failure
instead of an absent table entry. Revive never — it converts a structural property into a checked one.

### 4.1 The build table

```typescript
export type BuildState =
  | 'BUILD_IDLE'        // entered from focus IDLE_PRESENT; symmetric with it, not nested in it
  | 'BUILD_INTAKE'      // mirrors INTAKE — goal captured
  | 'DECOMPOSE'         // lld_decomposer running (filler-covered, same as crew_atomize)
  | 'PROPOSE'           // a ModuleBrief is on the table
  | 'BUILD_CLARIFY'     // mirrors CLARIFY — a required slot is empty
  | 'PUSHBACK'          // human disagrees; orb must defend with a NEW derivation or concede
  | 'FREEZE_CONFIRM'    // gate returned READY; awaiting the human's explicit confirm
  | 'FROZEN';           // FreezeRecord appended to the ledger. Terminal for P0.

export type BuildEvent =
  | 'enter_build' | 'goal_captured' | 'brief_drafted' | 'decompose_failed'
  | 'needs_slot' | 'slot_answered' | 'split_proposed'
  | 'user_pushback' | 'defend' | 'concede'
  | 'gate_ready' | 'gate_not_ready'
  | 'human_confirm' | 'human_reject'
  | 'exit_build';

export type BuildGuardId =
  | 'required_slots_filled'        // BuildClarifyProtocol: every REQUIRED_BUILD_SLOT non-null
  | 'gate_verdict_ready'           // lldReady(brief, refs).outcome === 'READY'
  | 'brief_schema_valid'           // validate_module_brief passed
  | 'defence_carries_new_derivation'; // the defending turn's Derivation is not byte-equal to a prior one
```

| from | event | guard | to |
|---|---|---|---|
| `BUILD_IDLE` | `enter_build` | — | `BUILD_INTAKE` |
| `BUILD_INTAKE` | `goal_captured` | — | `DECOMPOSE` |
| `DECOMPOSE` | `brief_drafted` | `brief_schema_valid` | `PROPOSE` |
| `DECOMPOSE` | `decompose_failed` | — | `BUILD_CLARIFY` |
| `PROPOSE` | `needs_slot` | — | `BUILD_CLARIFY` |
| `PROPOSE` | `user_pushback` | — | `PUSHBACK` |
| `PROPOSE` | `gate_ready` | `gate_verdict_ready` | `FREEZE_CONFIRM` |
| `PROPOSE` | `gate_not_ready` | — | `BUILD_CLARIFY` |
| `BUILD_CLARIFY` | `slot_answered` | `required_slots_filled` | `PROPOSE` |
| `BUILD_CLARIFY` | `slot_answered` (guard false) | — | `BUILD_CLARIFY` (stay) |
| `BUILD_CLARIFY` | `split_proposed` | — | `PROPOSE` |
| `PUSHBACK` | `defend` | `defence_carries_new_derivation` | `PROPOSE` |
| `PUSHBACK` | `concede` | — | `DECOMPOSE` |
| `FREEZE_CONFIRM` | `human_confirm` | — | `FROZEN` |
| `FREEZE_CONFIRM` | `human_reject` | — | `PROPOSE` |
| every state except `FROZEN` | `exit_build` | — | `BUILD_IDLE` |

Three properties the table must have, and U2 asserts each:
1. **`FROZEN` is only reachable via `FREEZE_CONFIRM --human_confirm-->`.** No other edge targets it —
   the human accept edge cannot be routed around.
2. **`FREEZE_CONFIRM` is only reachable via a `gate_verdict_ready` guard.** A proposer event cannot
   reach it.
3. **`BuildState` and `SessionState` share no literal**, so a build state can never be dispatched
   into `SESSION_TRANSITION_TABLE` and vice versa (a type error, not a runtime check).

`buildDispatch` reads no clock and no randomness, exactly like `dispatch`.

---

## 5. Status echo (P0 shape)

The echo reports **only what actually happened Orb-side**: gate verdict, freeze stamped, or the
named reasons a brief was refused. It never says "building", "tested", or "PR #N" — nothing
downstream exists (§7). A fabricated status is a failing state, not a nicety.

```typescript
// apps/mobile/src/build/BuildEnvelope.ts
export type BuildBlock =
  | { readonly kind: 'gate_refused'; readonly node_id: string;
      readonly reasons: readonly GateReason[]; readonly score: DepthScore }
  | { readonly kind: 'awaiting_confirm'; readonly node_id: string; readonly score: DepthScore }
  | { readonly kind: 'frozen'; readonly node_id: string; readonly freeze_id: string;
      readonly content_hash: string; readonly version: number;
      readonly handoff_status: 'artifact_written_no_consumer' };

export interface BuildResponseEnvelope extends ResponseEnvelope { readonly build?: BuildBlock }
```

`handoff_status` is a `const` with exactly one legal value in P0. When a consumer exists, that is a
schema change and a review — not a string edit. The spoken line for `frozen` is generated from a
fixed template that **must not contain** the words `PR`, `merged`, `built`, `deployed`, or
`shipped`; U5-T3 asserts this against the denylist.

---

## 6. Acceptance tests — the units a builder must make pass

Same format as [`../SOW-FLEET-2026-09-02.md`](../SOW-FLEET-2026-09-02.md) Track A. Each unit is one
worktree, one branch `sot-p0/<slug>`, sized to one focused session. **The test files named below are
written first and are frozen**: a builder makes them pass, never edits them.

| # | Slug | Owns (files, no overlap) | Acceptance test file | Done when |
|---|---|---|---|---|
| U1 | `lld-v1-schema` | `contracts/lld.v1.json`, `contracts/owners.v1.json`, `apps/mobile/src/build/lld-v1.ts`, `apps/mobile/src/build/fixtures/*`, `backend/relay-py/src/orb_relay/proxy/lld_schemas.py` | `apps/mobile/src/build/lld-v1.test.ts` + `backend/relay-py/tests/test_lld_schema_conformance.py` | schema validates both fixtures per their label; TS and Py agree on all fixtures; `content_hash` is register-invariant |
| U2 | `build-fsm` | `apps/mobile/src/build/fsm-core.ts`, `build/contracts.ts`, `build/BuildStateMachine.ts` | `apps/mobile/src/build/BuildStateMachine.test.ts` | table matches §4.1 exactly; the 3 reachability properties hold; dispatch is clock-free |
| U3 | `lld-ready-gate` | `apps/mobile/src/build/LldReadyGate.ts` | `apps/mobile/src/build/LldReadyGate.test.ts` | all 14 checks fire independently; both fixtures give the right verdict; purity proven |
| U4 | `build-mode-wire` | `apps/mobile/src/router/contracts.ts`, `backend/relay-py/src/orb_relay/proxy/schemas.py`, `backend/relay-py/src/orb_relay/store/freeze_store.py`, `backend/relay-py/src/orb_relay/proxy/lld_decomposer.py` | `apps/mobile/src/build/BuildModeWire.test.ts` + `backend/relay-py/tests/test_freeze_store_append_only.py` | `mode:'build'` reaches the request body; relay accepts it; ledger is append-only; decomposer fails closed to a clarify request |
| U5 | `status-echo` | `apps/mobile/src/build/BuildEnvelope.ts`, `apps/mobile/src/build/StatusEcho.ts` | `apps/mobile/src/build/StatusEcho.test.ts` | echo carries the real verdict; no downstream vocabulary is emittable |

**Dependency + conflict map (merge order, not build order — build all five in parallel worktrees):**

- **U1 is the foundation.** U2, U3, U5 import its types. Land U1 first; the other three rebase onto it.
  While U1 is in flight the three consumers develop against a local copy of §2's TypeScript and
  delete it on rebase.
- **U3 and U5 both consume `GateVerdict`/`DepthScore` but share no file.** No conflict.
- **U4 is the only unit touching pre-existing shared files** (`router/contracts.ts`,
  `proxy/schemas.py`). Both are hot files other concurrent Track-A work may touch. **Merge U4 last**,
  rebase it on everything else, and re-run `npm run verify` after the rebase, not before.
- **U2 is fully file-disjoint from U3/U4/U5.** It can merge at any point after U1.
- No unit may edit `apps/mobile/src/session/*`, `apps/mobile/src/lld/ResponseEnvelope.ts`,
  `apps/mobile/src/lld/ClarifyProtocol.ts`, or `backend/.../proxy/atomizer.py`. A diff touching those
  is rejected at review regardless of whether tests pass.

Verify gate for every unit: `npm run verify` (lint → typecheck → vitest → pytest → cargo test) green,
plus the unit's own test output pasted real. Evidence under `evidence/sot-p0/<slug>/`.

### U1 — `apps/mobile/src/build/lld-v1.test.ts`

```typescript
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { validateModuleBrief, canonicalJson, contentHash } from './lld-v1';

const load = (n: string) => JSON.parse(readFileSync(`${__dirname}/fixtures/${n}.json`, 'utf8'));

describe('lld.v1 schema', () => {
  it('U1-T1 accepts the complete_module fixture', () => {
    expect(validateModuleBrief(load('complete_module')).ok).toBe(true);
  });

  it('U1-T2 rejects the one_line_freeze fixture, naming ≥3 distinct field paths', () => {
    const r = validateModuleBrief(load('one_line_freeze'));
    expect(r.ok).toBe(false);
    expect(new Set(r.errors.map((e) => e.path)).size).toBeGreaterThanOrEqual(3);
  });

  it('U1-T3 has no proposer-writable authority field', () => {
    // ModuleBrief must not carry any field the gate is supposed to stamp.
    const brief = load('complete_module');
    for (const forbidden of ['content_hash', 'freeze_id', 'depth_score', 'stamped_by', 'state', 'version']) {
      expect(Object.prototype.hasOwnProperty.call(brief, forbidden)).toBe(false);
    }
    // ...and adding one must be a validation error, not a silently ignored extra key.
    expect(validateModuleBrief({ ...brief, stamped_by: 'orb:lld-ready' }).ok).toBe(false);
  });

  it('U1-T4 content_hash is invariant to narration/register/ordering', () => {
    const brief = load('complete_module');
    const reordered = Object.fromEntries(Object.entries(brief).reverse());
    const narrated = { ...brief, purpose: brief.purpose };            // same decision, same bytes
    expect(contentHash(reordered)).toBe(contentHash(brief));
    expect(contentHash(narrated)).toBe(contentHash(brief));
    // and it MUST change when a decision changes
    expect(contentHash({ ...brief, owner_path: 'apps/other/' })).not.toBe(contentHash(brief));
  });

  it('U1-T5 canonicalJson is stable across 100 shuffles', () => {
    const brief = load('complete_module');
    const hashes = new Set(
      Array.from({ length: 100 }, () =>
        contentHash(Object.fromEntries(
          Object.entries(brief).sort(() => Math.random() - 0.5)))),
    );
    expect(hashes.size).toBe(1);
  });
});
```

```python
# backend/relay-py/tests/test_lld_schema_conformance.py
import json, pathlib, pytest
from orb_relay.proxy.lld_schemas import validate_module_brief

FIX = pathlib.Path(__file__).parents[3] / "apps/mobile/src/build/fixtures"

@pytest.mark.parametrize("name,expected_ok", [("complete_module", True), ("one_line_freeze", False)])
def test_u1_t6_py_agrees_with_ts_on_every_fixture(name, expected_ok):
    """The cross-language mirror that shared/atomizer-schema.ts's header warns about.
    A drift here means the client would accept a brief the relay rejects."""
    brief = json.loads((FIX / f"{name}.json").read_text())
    assert (validate_module_brief(brief) is not None) is expected_ok
```

### U2 — `apps/mobile/src/build/BuildStateMachine.test.ts`

```typescript
import { describe, expect, it } from 'vitest';
import { BUILD_TRANSITION_TABLE, buildDispatch } from './BuildStateMachine';
import type { BuildState } from './contracts';
import { SESSION_TRANSITION_TABLE } from '../session/StateMachine';

describe('build-mode FSM', () => {
  it('U2-T1 FROZEN is reachable only from FREEZE_CONFIRM on human_confirm', () => {
    const intoFrozen: string[] = [];
    for (const [from, edges] of Object.entries(BUILD_TRANSITION_TABLE)) {
      for (const [event, t] of Object.entries(edges as Record<string, any>)) {
        const targets = t.kind === 'branch' ? [t.when_true, t.when_false] : [t.to];
        if (targets.includes('FROZEN')) intoFrozen.push(`${from}:${event}`);
      }
    }
    expect(intoFrozen).toEqual(['FREEZE_CONFIRM:human_confirm']);
  });

  it('U2-T2 FREEZE_CONFIRM is only entered behind the gate_verdict_ready guard', () => {
    for (const [from, edges] of Object.entries(BUILD_TRANSITION_TABLE)) {
      for (const [event, t] of Object.entries(edges as Record<string, any>)) {
        if (t.kind === 'to' && t.to === 'FREEZE_CONFIRM') {
          expect(t.guard, `${from}:${event}`).toBe('gate_verdict_ready');
        }
      }
    }
  });

  it('U2-T3 a false gate guard cannot advance out of PROPOSE', () => {
    expect(buildDispatch('PROPOSE', 'gate_ready', { gate_ready: false })).toBe('PROPOSE');
    expect(buildDispatch('PROPOSE', 'gate_ready', { gate_ready: true })).toBe('FREEZE_CONFIRM');
  });

  it('U2-T4 build and session state spaces are disjoint', () => {
    const build = new Set(Object.keys(BUILD_TRANSITION_TABLE));
    const session = new Set(Object.keys(SESSION_TRANSITION_TABLE));
    expect([...build].filter((s) => session.has(s))).toEqual([]);
  });

  it('U2-T5 dispatch is clock-free and replay-identical', () => {
    const path: Array<[BuildState, any]> = [
      ['BUILD_IDLE', 'enter_build'], ['BUILD_INTAKE', 'goal_captured'],
      ['DECOMPOSE', 'brief_drafted'], ['PROPOSE', 'gate_ready'],
    ];
    const run = () => path.map(([s, e]) =>
      buildDispatch(s, e, { gate_ready: true, brief_valid: true })).join('>');
    const now = Date.now; const rand = Math.random;
    (Date as any).now = () => { throw new Error('clock read in dispatch'); };
    (Math as any).random = () => { throw new Error('randomness in dispatch'); };
    try { expect(run()).toBe(run()); } finally { (Date as any).now = now; (Math as any).random = rand; }
  });
});
```

### U3 — `apps/mobile/src/build/LldReadyGate.test.ts`

```typescript
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { lldReady, GATE_CHECK_IDS } from './LldReadyGate';

const load = (n: string) => JSON.parse(readFileSync(`${__dirname}/fixtures/${n}.json`, 'utf8'));
const REFS = {
  owners: ['rachit@devxlabs.ai'],
  registry_paths: ['registry/services/llm-gateway', 'registry/features/cost-control-plane'],
  known_node_ids: ['orb-freeze-ledger'],
};

describe('lld-ready gate', () => {
  it('U3-T1 refuses the one-line freeze, naming ≥5 distinct check ids', () => {
    const v = lldReady(load('one_line_freeze'), REFS);
    expect(v.outcome).toBe('NOT_READY');
    expect(new Set(v.reasons.map((r) => r.check_id)).size).toBeGreaterThanOrEqual(5);
  });

  it('U3-T2 accepts the control fixture with checked driven to full count', () => {
    const v = lldReady(load('complete_module'), REFS);
    expect(v.outcome).toBe('READY');
    expect(v.checked).toBe(GATE_CHECK_IDS.length);
    expect(v.score.checks_passed).toBe(v.score.checks_total);
  });

  it('U3-T3 every check id can be failed in isolation from the control fixture', () => {
    // Each mutation below must trip exactly its own check and no other.
    const base = load('complete_module');
    const mutations: Record<string, (b: any) => any> = {
      'C1-OPEN':        (b) => ({ ...b, open_questions: ['what store?'] }),
      'C2-OWNER':       (b) => ({ ...b, owner: 'me' }),
      'C3-ACC-NONTAUT': (b) => ({ ...b, acceptance: { ...b.acceptance, then: 'it works' } }),
      'R17-DERIV':      (b) => ({ ...b, guarantees: [{ ...b.guarantees[0],
                                   derivation: { kind: 'number', value: 'fast', calc: '' } }] }),
      'R19-ABSOLUTE':   (b) => ({ ...b, guarantees: [{ claim: 'zero data loss', label: 'mitigates',
                                   derivation: { kind: 'number', value: '0', calc: '0 = 0' } }] }),
      'R21-ALTS':       (b) => ({ ...b, alternatives: b.alternatives.slice(0, 1) }),
      'R21-FAIL':       (b) => ({ ...b, failure_story: { ...b.failure_story, blast_radius: '' } }),
      'C12-STORE':      (b) => ({ ...b, data_owned: [{ store: 'x', owned_by_node: 'someone-else' }] }),
    };
    for (const [id, mutate] of Object.entries(mutations)) {
      const v = lldReady(mutate(base), REFS);
      expect(v.outcome, id).toBe('NOT_READY');
      expect(v.reasons.map((r) => r.check_id), id).toContain(id);
    }
  });

  it('U3-T4 a high depth_score is NOT a pass — the ratio is never a threshold', () => {
    const v = lldReady({ ...load('complete_module'), open_questions: ['one'] }, REFS);
    expect(v.outcome).toBe('NOT_READY');
    expect(v.score.ratio).toBeGreaterThan(0.9);   // nearly perfect, still refused
  });

  it('U3-T5 the gate is pure: no clock, no randomness, no network, no fs', async () => {
    const brief = load('complete_module');
    const saved = { now: Date.now, rand: Math.random, fetch: globalThis.fetch };
    (Date as any).now = () => { throw new Error('clock'); };
    (Math as any).random = () => { throw new Error('random'); };
    (globalThis as any).fetch = () => { throw new Error('network'); };
    try {
      const a = lldReady(brief, REFS); const b = lldReady(brief, REFS);
      expect(JSON.stringify(a)).toBe(JSON.stringify(b));
    } finally { Object.assign(Date, { now: saved.now }); Object.assign(Math, { random: saved.rand });
                (globalThis as any).fetch = saved.fetch; }
  });

  it('U3-T6 an empty check set is MEASURED_NOTHING, never READY', () => {
    const v = lldReady(load('complete_module'), { owners: [], registry_paths: [], known_node_ids: [] });
    expect(v.outcome).not.toBe('READY');   // owners is empty ⇒ C2 cannot pass; never a vacuous green
  });
});
```

### U4 — `apps/mobile/src/build/BuildModeWire.test.ts`

```typescript
import { describe, expect, it, vi } from 'vitest';
import { createRelayConversationPort } from '../runtime/ConversationPort';

describe('build mode reaches the wire', () => {
  it("U4-T1 mode:'build' appears in the POST body, not just in local state", async () => {
    // The scar this guards: `mode` was computed everywhere and transmitted nowhere,
    // making teach/converse unreachable from the real app while fully unit-tested.
    const fetchImpl = vi.fn(async () => new Response(
      JSON.stringify({ text: 'ok', source: 'model', latency_ms: 1, spent_paise: 0 }),
      { status: 200, headers: { 'content-type': 'application/json' } }));
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl: fetchImpl as any });
    await port.respond({ session_id: 's', tenant_id: 't', user_id: 'u', text: 'build me a thing', mode: 'build' });
    const body = JSON.parse((fetchImpl.mock.calls[0][1] as any).body);
    expect(body.mode).toBe('build');
  });

  it('U4-T2 an absent mode is omitted, not sent as null (relay would 422)', async () => {
    const fetchImpl = vi.fn(async () => new Response(
      JSON.stringify({ text: 'ok', source: 'model', latency_ms: 1, spent_paise: 0 }), { status: 200 }));
    const port = createRelayConversationPort({ baseUrl: 'http://relay.test', fetchImpl: fetchImpl as any });
    await port.respond({ session_id: 's', tenant_id: 't', user_id: 'u', text: 'hi' });
    expect(Object.prototype.hasOwnProperty.call(
      JSON.parse((fetchImpl.mock.calls[0][1] as any).body), 'mode')).toBe(false);
  });
});
```

```python
# backend/relay-py/tests/test_freeze_store_append_only.py
import pytest
from orb_relay.store.freeze_store import FreezeStore

def test_u4_t3_ledger_is_append_only(tmp_path):
    s = FreezeStore(tmp_path / "fz.db")
    s.append_freeze(tenant_id="t", user_id="u", session_id="s", freeze_id="fz-0000000000000001",
                    node_id="n1", version=1, content_hash="a" * 64, supersedes=None,
                    payload="{}", created_at=1.0)
    # There must be no way to mutate or remove a stamped freeze.
    for forbidden in ("update", "delete", "edit", "set_superseded", "prune"):
        assert not hasattr(s, forbidden), f"FreezeStore exposes {forbidden}"
    s.append_freeze(tenant_id="t", user_id="u", session_id="s", freeze_id="fz-0000000000000002",
                    node_id="n1", version=2, content_hash="b" * 64,
                    supersedes="fz-0000000000000001", payload="{}", created_at=2.0)
    rows = s.list_freezes(tenant_id="t", user_id="u", session_id="s")
    assert [r.version for r in rows] == [1, 2]          # v1 survives; supersede is a link, not a delete
    assert rows[1].supersedes == "fz-0000000000000001"

def test_u4_t4_ledger_never_prunes(tmp_path):
    """ConversationStore prunes at 24 turns. The freeze ledger must NOT inherit that."""
    s = FreezeStore(tmp_path / "fz.db")
    for i in range(1, 41):
        s.append_freeze(tenant_id="t", user_id="u", session_id="s", freeze_id=f"fz-{i:016d}",
                        node_id=f"n{i}", version=1, content_hash=f"{i:064d}", supersedes=None,
                        payload="{}", created_at=float(i))
    assert len(s.list_freezes(tenant_id="t", user_id="u", session_id="s")) == 40

@pytest.mark.asyncio
async def test_u4_t5_decomposer_fails_closed_to_clarify_not_a_guessed_brief(monkeypatch):
    """atomize() falls back to FALLBACK_STEP. decompose() must NOT have an equivalent —
    a guessed ModuleBrief costs a whole lane, not 30 seconds."""
    from orb_relay.proxy import lld_decomposer
    class AlwaysGarbage:
        async def complete(self, **_):
            class C: text = "not json at all"; usage = lld_decomposer.UsageDelta()
            return C()
    result = await lld_decomposer.decompose("build a thing", gateway=AlwaysGarbage(), tenant_id="t")
    assert result.kind == "clarify_request"
    assert result.brief is None
    assert result.missing, "a clarify request must name what it needs"
```

### U5 — `apps/mobile/src/build/StatusEcho.test.ts`

```typescript
import { describe, expect, it } from 'vitest';
import { statusEcho, echoSpeech } from './StatusEcho';

const SCORE = { checks_total: 14, checks_passed: 9, ratio: 0.643, failed_check_ids: ['R17-DERIV'] };

describe('status echo (P0)', () => {
  it('U5-T1 a refusal echoes the real check ids, not a generic apology', () => {
    const b = statusEcho({ outcome: 'NOT_READY', checked: 14, score: SCORE,
                           reasons: [{ check_id: 'R17-DERIV', detail: 'guarantee 0: calc empty' }] }, 'n1');
    expect(b.kind).toBe('gate_refused');
    expect(b.reasons.map((r: any) => r.check_id)).toContain('R17-DERIV');
    expect(b.score.ratio).toBe(0.643);
  });

  it('U5-T2 a frozen echo declares that nothing consumes the artifact', () => {
    const b = statusEcho({ outcome: 'READY', checked: 14,
                           score: { ...SCORE, checks_passed: 14, ratio: 1, failed_check_ids: [] } },
                         'n1', { freeze_id: 'fz-0000000000000001', content_hash: 'a'.repeat(64), version: 1 });
    expect(b).toMatchObject({ kind: 'frozen', handoff_status: 'artifact_written_no_consumer' });
  });

  it('U5-T3 no echo can claim downstream work that does not exist', () => {
    const forbidden = /\b(PR|pull request|merged|merging|built|building|deployed|shipped|attested)\b/i;
    const blocks = [
      statusEcho({ outcome: 'NOT_READY', checked: 14, score: SCORE, reasons: [] }, 'n1'),
      statusEcho({ outcome: 'READY', checked: 14, score: { ...SCORE, checks_passed: 14, ratio: 1,
                   failed_check_ids: [] } }, 'n1',
                 { freeze_id: 'fz-0000000000000001', content_hash: 'a'.repeat(64), version: 1 }),
    ];
    for (const b of blocks) expect(echoSpeech(b)).not.toMatch(forbidden);
  });
});
```

---

## 7. NOT in scope — explicitly

**The fleet-rs boundary (hard).** `fleet-rs/` is owned by another agent, its gate is red, and it has
**zero PR-emit code** (grepped, 0 hits). Per `FLEET-LEARNINGS.md` this contract terminates at a
written `lld.v1` artifact. Out of scope, and a diff proposing any of it is rejected:

1. Any change inside `fleet-rs/` — including reading its contracts into a build step.
2. `lld.v1` → SOW intake, `fleet sow`, `fleet run`, `swarm dispatch`, or any lane spawn.
3. The PR emit, the 8/9-element attestation, `run_with_evidence`, model routing, semgrep/trivy wiring.
4. A status echo that reports building / verifying / attested / merged / "PR #N". U5-T3 makes this a
   test failure, not a style note.
5. `keel`-side authority. In P0 the gate runs **in the Orb** and its verdict is stamped
   `orb:lld-ready`, not `keel:lld-ready`. When keel gains the canonical gate, keel wins and this
   copy becomes advisory — that is a P1 conformance-vector change, not a P0 assumption.

**Deferred Orb-side work (designed in the blueprint, not built in P0):**

6. **Information-gain clarify stopping (`EIG`, `ε_gain`, `θ_amb`).** P0 uses required-slot ordering
   plus the split trigger. The belief registers EIG needs do not exist yet.
7. **The re-aimed `BeliefModel`** (spec-completeness / ambiguity / coverage registers,
   dependency-invalidation decay). `cognitive/BeliefModel.ts` is untouched in P0.
8. **The design graph** (`design-graph.v1`, DAG, edges, node lifecycle, console render). P0 freezes
   one module at a time into a flat append-only ledger; there is no graph object.
9. **Blast-radius and metric-registry conjuncts** in the gate — no code graph, no metric registry
   (§3.5). Named, not silently dropped.
10. **Rubric calibration** against `depth-gold-pos` / `depth-gold-neg`, false-PASS ≤1% / false-FAIL
    ≤10%, and the trigger-rate-vs-defect-rate governor. P0 ships the gate and the two fixtures; the
    labelled corpus is P1. Until then the gate's thresholds are `[A]`, and a fresh gate's first runs
    are mostly false-FAILs by construction — tune on known-good freezes before trusting a red.
11. **Register adaptation** (narration by user register). The freeze is already register-invariant by
    construction (§2.1), so this is prose work with no correctness stake in P0.
12. **Streaming voice.** Build mode in P0 is text-or-push-to-talk on the existing batch path.

---

## 8. Review gates (what the lead checks before any unit merges)

1. The unit's named test file is **byte-unchanged** from this contract. Any diff to it is rejected.
2. `npm run verify` exit 0, output pasted real — including any red on the way.
3. The unit touched **only** the files in its §6 "Owns" column.
4. The forbidden-file list in §6 is untouched.
5. Evidence at `evidence/sot-p0/<slug>/` includes the gate verdict JSON for both fixtures (U1/U3) or
   the real request body (U4), not a summary of them.
6. An entry appended to [`../../../FLEET-LEARNINGS.md`](../../../FLEET-LEARNINGS.md).

Contracts and schemas are **human-merge always** (A15/D4): U1 and U4 ship to a PR and stop.
