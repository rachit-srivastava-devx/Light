# F05 — Freeze protocol: propose → pushback → ❄, server-side (lane contract)

**Lane:** `lane/F05-freeze-protocol` · **Repo:** `orb` (`backend/relay-py`) · **Depends on:** F02, F03, F04, **F06**
**Written by:** lead-architect (Opus 5), 2026-09-07 · **Builder may not edit §5's test files.**
**Blueprint slice:** `blueprints/Speed-of-Thought-L8-Deep-Dive/02-DIALOGUE-PLANE-BUILD-MODE.md` §3 (primary), §4, §5;
`03-SPEC-COMPLETE-GATE-AND-DESIGN-GRAPH.md` §4 (the `freeze.v1` stamp — **consumed, never authored, see §2.1**).

---

## 0. Restatement — what is actually being asked, in this codebase's own terms

FEATURES.md row F05, in one sentence: **make the build-mode dialogue converge on a bound derived from
information gain rather than a question count, and make the point at which it stops be a real,
observable ❄ event over the relay — with a proof that it terminates on both branches.**

The row's acceptance line is the spec:

> A scripted conversation with all 5 slots answered reaches a ❄ freeze in a bounded, provably-terminating
> number of turns, driven over the relay from a raw test client; one with a slot deliberately never
> answered does **not** freeze and does not loop forever.

Two halves, and the second half is the one that is easy to fake. "Does not loop forever" is not proved by
a test that happens to stop; it is proved by a **monotone counter with a hard ceiling** plus a
demonstration that no reachable state has "ask nothing, freeze nothing" as its only option. §3.1 shows
that the naive reading of the blueprint's own predicates **does** have such a state, with the exact
register values that reach it.

### 0.1 What I proved before designing, rather than assumed

Every claim below was checked against the tree, not recalled.

| # | Claim | How checked | Result |
|---|---|---|---|
| a | The worktree is at the state the dispatch describes ("master with F01–F04, F06, F08 merged") | `git log --oneline HEAD..master` | **FALSE.** `lane/F05-freeze-protocol` sits at `20c857e`, **4 commits behind `master`**. `fleet/keel/fleet/src/lld_ready.rs` — F06's whole gate — **does not exist in this worktree.** See §8.3 L1; this is the builder's first action. |
| b | A Python mirror of the `lld-ready` gate exists to call | Read `fleet/tests/acceptance/lld-crosslang.sh` + `lld_schemas.py`'s own docstring | **FALSE.** The "3 mirrors" are the **schema** mirrors (`lld-v1.ts` / `lld_schemas.py` / `lld.rs`). The **gate** exists only in TS (`LldReadyGate.ts`) and Rust (`lld_ready.rs`). `lld_schemas.py` line 23–25 says so explicitly: *"They do not implement the lld-ready gate's business-rule checks … that gate is a separate unit."* This is the single biggest design forcing-function in this lane (§2.2). |
| c | The orb may construct a `Freeze` | `grep stamped_by fleet/contracts/fixtures/lld/*.json` | **FALSE, and it is a fixtured forgery case.** `forged_freeze.json` is an **invalid** fixture whose only defect is `"stamped_by": "orb:lld-ready"`. `lld_ready.rs` holds `pub const STAMPED_BY: &str = "keel:lld-ready"`. See §2.1. |
| d | `FreezeStore` already exists and is the right home for what F05 produces | Read `store/freeze_store.py` + `tests/test_freeze_store_append_only.py` | **Exists; wrong home.** It is an append-only ledger of *stamped* records; F05 produces *proposals*. Writing proposals into it makes a later reader unable to tell a proposal from a stamp. Killed in §2.4. Persistence is F12's row, not this one. |
| e | F04's belief registers are real and readable | Read `cognitive/belief.py`, `build/build_session.py`, `store/build_session_store.py`; `app.py:843-925` | **True.** `BuildSessionStore.registers(...) -> Registers` is live on every BUILD turn. F05 reads it; F05 does not modify `belief.py`. |
| f | `canonical_json` / `content_hash` are reusable | Read `lld_schemas.py:canonical_json` | **True, with a trap.** Both `raise TypeError("numbers are not canonicalizable (F02 §6.3)")` on **any** numeric leaf at any depth. F05 carries floats (`H`, `EIG`, confidences). See §8.3 L4. |
| g | `CoverageSlot` already names the slot set the dispatch asks for | Read `cognitive/belief.py:85-97` | **True, and it has six members, not five.** See §0.2. |
| h | A `.venv` exists in this worktree | `ls orb/backend/relay-py/.venv` | **FALSE** — no venv. F06's verifier lost time to exactly this. §8.3 L2. |

### 0.2 One named divergence from the dispatch's own wording, decided at the lead level

The dispatch says *"new slot set (interface/data_owned/acceptance/deps/non_goals per F02's real
ModuleBrief fields)"* — five slots. Blueprint §3.1 lists **six**: `interface · data_owned · acceptance ·
deps · non_goals · registry_verdict`. F04 already shipped `CoverageSlot` with exactly those six, and
`BuildSessionStore` has persisted evidence keyed by them since F04 merged.

**Decision: reuse F04's six-member `CoverageSlot` verbatim. Do not define a new slot enum.** Defining a
five-member set in F05 would fork the register vocabulary from the one the persisted evidence log is
keyed by, and `fold()` would silently stop covering `registry_verdict` — the C1 slot, which keel's
`REG-VERDICT` check actively gates. The dispatch's five is a paraphrase of the ModuleBrief field names,
not a decision to drop C1.

**But the six are not all *required* for coverage.** §3.2 derives which five are, and why `non_goals` is
deliberately not one of them — a result forced by arithmetic, not preference.

---

## 1. C1 / L2 registry verdict, said out loud

**Verdict: `install` for four of the five pieces; `build-new` for exactly one — the stopping rule.**

I ran the registry pass before designing, per C1/L2. What already exists and must be *used, not rebuilt*:

| Need | Already real | Verdict |
|---|---|---|
| Log-odds registers, κ/λ law, `fold()` over an evidence log | `orb_relay.cognitive.belief` (F04) | **install** — read-only. F05 adds no register and edits no line of `belief.py`. |
| Per-session persisted evidence log + `registers()` | `orb_relay.store.build_session_store` (F04) | **install** — F05 appends `Evidence` through the existing `append_evidence`; no new table. |
| Slot vocabulary + dependency edges + `BuildTurnState` rendering | `orb_relay.build.build_session` (F04) | **install** — `CoverageSlot`, `build_slot_dependencies()`, `render_registers()` reused as-is. |
| `ModuleBrief` / `AcceptanceLine` / `KilledAlt` / `canonical_json` / `content_hash` | `orb_relay.proxy.lld_schemas` (F02) | **install** — a wire contract; F05 imports, never edits. |
| Brief production from accumulated turns | `orb_relay.proxy.lld_decomposer.decompose` (F03), already wired at `app.py:884` | **install** — F05 consumes `decompose_result.brief`; it does not call the gateway itself. |
| Depth verdict (R17–R21 + the 14 checks) | `fleet/keel/.../lld_ready.rs` + `LldReadyGate.ts` (F06) | **install via the CLI seam** (§2.2) — **not** ported. |
| Append-only ledger of stamped freezes | `orb_relay.store.freeze_store` (F02-era) | **not used by this lane** (§2.4) — F12's row. |
| **The information-gain stopping rule: `H(B)`, `EIG`, `FREEZE_ELIGIBLE`, the split trigger, the no-progress escape, `ProposalTurn`** | **nothing** — `ClarifyProtocol.ts` is a 4-slot, `MAX_CLARIFY_QUESTIONS = 2` counter with no entropy, no weights and no EIG anywhere in the tree | **build-new** |

`ClarifyProtocol.ts` is the *reference for the rule-driven, no-LLM-decides-the-slot shape* (blueprint §3.1
keeps that design) and for nothing else. Its actual mechanism — `questions_asked >= 2 → proceed` — is the
thing blueprint §3.1 exists to **replace**, and its scar (`Operator's scars` #1: *"three wasted lanes in
one session"*) is the reason. Grep confirms: no occurrence of `EIG`, `information_gain`, `residual_ambiguity`,
`theta_cov`, or `epsilon_gain` anywhere in `orb/` or `fleet/`.

---

## 2. Killed alternatives

### 2.1 KILLED — F05 emits a `Freeze` (and therefore an `lld.v1`)

The obvious reading of "reaches a ❄ freeze" is that F05 constructs `lld_schemas.Freeze` and hands it on.
**It must not**, and this is not a style call:

- `Freeze.stamped_by` is `Literal["keel:lld-ready"]` — a `const`, and 03 §4.1 says why in as many words:
  *"the one field a proposer would love to forge — `stamped_by` — pinned to a `const` only the gate can
  satisfy (there is no proposer path that writes `keel:lld-ready`). `kills (structural)` a hand-authored
  freeze."*
- `fleet/contracts/fixtures/lld/forged_freeze.json` exists **for this exact case**: a fixture that is
  invalid solely because a proposer wrote `"stamped_by": "orb:lld-ready"`. F02 shipped the negative
  fixture. F05 would be the positive instance of it.
- Blueprint §6.2: the Orb runs the rubric as *"may I offer to freeze?"*; keel runs it as *"you may freeze."*
  An orb that stamps has collapsed propose and refuse into one side, which is the law this whole system
  is built on (`08-DETERMINISM-AND-SAFETY.md`).

**Consequence, stated plainly rather than buried:** *no producer of a stamped `Freeze` exists anywhere in
this tree today.* The CLI `fleet gate lld-ready <file>` **prints a verdict; it does not emit a `Freeze`
JSON.** F05 therefore terminates at a `FreezeProposal` carrying keel's own READY verdict, and the
gap — "a keel path that emits the stamp" — is **flagged forward to F07/F09** in §9, not silently absorbed.
This is the honest boundary FEATURES.md's F09 row already anticipates when it says F09 removes the *"stop
at a frozen `lld.v1` artifact"* boundary.

*Revive trigger:* a keel subcommand exists that emits `freeze.v1` JSON on stdout (then F05's successor
parses it rather than constructing it).

### 2.2 KILLED — port F06's 14 checks to Python so `depth(B)` can be called in-process

Tempting: `FREEZE_ELIGIBLE` needs `depth(B) = PASS`, F05 is Python, and there is no Python gate (§0.1b).

Killed on three grounds:

1. **F06's own contract already killed the equivalent move.** Its §3.3 — *"KILLED — re-derive the 14 checks
   in Rust from the blueprint, independently of `LldReadyGate.ts`"* — because two independently-derived
   copies drift. A third copy is strictly worse than the second one F06 refused to write blind.
2. **There would be no conformance harness holding it honest.** `lld-crosslang.sh` compares *schema*
   mirrors. `lld-ready-crosslang.sh` compares the **TS and Rust gates only**. A Python gate copy would
   ship with zero cross-language coverage — the exact "adopted but never actually run" shape.
3. **It inverts the authority.** Blueprint §6.2: *"If they ever disagree, keel wins and the divergence is a
   conformance-test failure."* A Python copy that keel never sees cannot lose an argument it is never in.

**Chosen instead: a `ReadinessGate` port with keel behind it (§4.2).** The Python side holds a
protocol with one method and *no rule logic at all*; the production implementation shells out to
`fleet gate lld-ready <tmpfile>` and reads the **exit code** (§4.2 pins the exact contract). The orb's copy
of the rubric is therefore *empty* rather than *duplicated* — a stronger version of "advisory" than the
blueprint asks for, and the only version with no drift surface.

*Revive trigger:* keel gains a `--json` verdict output **and** a py mirror lands with a
`lld-ready-crosslang.sh` arm covering it.

### 2.3 KILLED — treat blueprint §3.2's `KEEP_ASKING` as the dialogue loop's continue-condition

This is the reading a builder will reach for, and it **hangs**. Proof, with real numbers, in §3.1. Killed
because the blueprint's own sentence scopes it narrowly — *"Stop asking **on the ambiguity axis** when the
best remaining question is not worth the breath"* — and because §9's failure table states the governing
rule outright: *"coverage is a hard conjunct independent of EIG → slower convergence, never an
under-covered freeze."*

*Revive trigger:* none. It is a misreading, and §5's T5 is the regression pin.

### 2.4 KILLED — write the ❄ event into the existing `FreezeStore`

`FreezeStore` is real, append-only, tenant-scoped, and sitting unused. Reusing it looks like C1 obedience.

Killed: its table is `freeze_records`, its docstring says *"the append-only ledger for lld.v1
FreezeRecords"*, and its `freeze_id` column is `UNIQUE` against the `^fz-[0-9a-f]{16}$` shape of a
**stamped** id. Putting unstamped proposals in it means the next reader (F09's handoff, F12's session
ledger) cannot distinguish "keel certified this" from "the orb thought keel would." That is the
**`assert the retirement, not just the replacement`** failure in miniature: a row that no longer means
what its schema says it means.

FEATURES.md is explicit about ownership anyway: **F12** is *"L1 working memory: freeze ledger as a session
object… the delta is making the freeze ledger a first-class part of it,"* depending on F05. F05 produces
the event; F12 persists the ledger.

*Revive trigger:* F12 lands and defines a proposal-vs-stamp discriminator column.

### 2.5 KILLED — read `c_s` as the register's `value` rather than its `confidence`

Blueprint §3.2 gives two per-slot quantities: *"a confidence `c_s ∈ [0,1]` (is it filled correctly?) and a
residual uncertainty `u_s ∈ [0,1]` (entropy of its current value distribution)."* §4.1's register table
then describes `Coverage[s]` as estimating *"is required slot `s` filled correctly?"* — which reads like
`c_s = value`.

Killed, for three reasons:

1. It leaves `RegisterState.confidence` — the field F04 built the entire `κ = 0.4` accumulation law
   around — with **no consumer in the stopping rule at all**. A shipped field with no reader is the
   wire-nothing defect this estate keeps paying for.
2. `u_s` is *already* the entropy of the value distribution. `(1 − value) · H₂(value)` double-counts the
   same quantity; `(1 − confidence) · H₂(value)` is the textbook "how much do we know" × "how uncertain is
   what we know" product that optimal-experiment-design actually uses.
3. It makes `θ_cov = 0.80` land on a **discrete, checkable, non-arbitrary** bar: with `κ = 0.4` and
   `r = 1.0`, confidence goes `0.4 → 0.8 → 1.0`, so `c_s ≥ 0.80` is exactly *"this slot has been
   evidenced at least twice."* Under the `value` reading, `0.80` is a sigmoid coordinate with no
   operational meaning.

**Chosen: `c_s = registers[(COVERAGE, s)].confidence`, `u_s = H₂(registers[(COVERAGE, s)].value)`** —
a literal, field-by-field mapping onto F04's shipped `RegisterState`, using both of its live fields.

*Revive trigger:* a real slot-extractor lands that sets `value` from extraction quality and `confidence`
from turn count independently, at which point the product may want re-deriving.

### 2.6 KILLED — a new `/v1/build/freeze` endpoint (or any new route)

F04's §2.4 already killed a second `/v1/build/...` endpoint family, and F03 obeyed it by composing the
decomposer into the **same** `/v1/respond` BUILD branch. F05 does the same: the protocol state and the ❄
event ride the existing `ConversationResponse` as additive fields. A raw WebSocket/HTTP test client with
no RN and no Swift can therefore drive the whole thing, which is the FEATURES.md acceptance line's
literal requirement.

*Revive trigger:* the freeze attempt's latency makes a build turn exceed the voice budget (measure before
assuming — the gate subprocess is off the spoken-reply path by construction, §4.4).

### 2.7 KILLED — let the belief registers decide the freeze

Restated here only because F05 is the first lane with both the registers and a freeze in scope, and F04's
§2.6 killed it *in anticipation of this lane*. `FREEZE_ELIGIBLE` reads registers for the **coverage and
ambiguity conjuncts only** — quantities the blueprint defines *in terms of* the registers. The
**depth verdict is a pure function of `ModuleBrief` fields evaluated by keel**, and no belief value is an
input to it. Blueprint §4.1: *"a wrong belief can waste a question but cannot freeze a bad spec."*

---

## 3. The two things this lane exists to get right

### 3.1 The stall state — a reachable "ask nothing, freeze nothing" hang, found by arithmetic

Implement blueprint §3.2 literally (`loop continues ⟺ KEEP_ASKING`, i.e. `max_s EIG ≥ ε_gain`) and this
state hangs. It is not hypothetical; here are its exact register values, all reachable from
`initial_registers()` using only F04's own shipped `_TURN_EVIDENCE_WEIGHT = 1.2`, `r = 1.0`, Tier-0:

| Slot | Evidence applied | `value` | `c_s` (confidence) | `u_s = H₂(value)` |
|---|---|---|---|---|
| `interface` | 3 × (w=+1.2, r=1.0) | `0.973403006423134` | `1.0` | `0.17702772704896477` |
| `data_owned` | **1** × (w=+1.2, r=1.0) | `0.7685247834990175` | **`0.4`** | `0.780574086303188` |
| `acceptance` | 3 × | `0.973403006423134` | `1.0` | `0.17702772704896477` |
| `deps` | 3 × | `0.973403006423134` | `1.0` | `0.17702772704896477` |
| `non_goals` | none (never asked — see §3.2) | `0.5` | `0.0` | `1.0` |
| `registry_verdict` | 3 × | `0.973403006423134` | `1.0` | `0.17702772704896477` |

At that state, with §4.1's constants:

```
H(B)                        = 0.12025166776728692      ≤ θ_amb (0.15)   ✓
∀ required s: c_s ≥ θ_cov   = FALSE   (data_owned is 0.4)               ✗
max_s EIG(open questions)   = 0.04000000000000001  on non_goals   < ε_gain (0.05)
    EIG[data_owned, answer] = 0.035125833883643459                 < ε_gain
```

So `KEEP_ASKING` is **false** (nothing is worth asking) and `FREEZE_ELIGIBLE` is **false** (coverage
unmet). The dialogue asks nothing and freezes nothing, forever. `data_owned` is caught between the two
thresholds precisely because it is a **mid-weight (`w = 0.6`) compositional (`ρ = 0.5`) slot at one
evidence** — a completely ordinary state to be in after three turns.

**The resolution is the blueprint's own §3.4 escape, fired by the right trigger.** Escalating that
slot's ask to `Ask.kind = 'confirm'` (`ρ = 1.0`, *"a binary answer is maximally resolving"*) doubles its
EIG:

```
EIG[data_owned, confirm]    = 0.07025166776728692      ≥ ε_gain (0.05)  ✓  → argmax returns to data_owned
```

So the loop condition is **not** `KEEP_ASKING`. It is:

```
CONTINUE(B) ⟺ ¬FREEZE_ELIGIBLE(B) ∧ clarify_turns < SPLIT_TRIGGER_TURNS
```

and `KEEP_ASKING` retains exactly the role blueprint §3.2 gives it — *the ambiguity-axis stop*, one
conjunct of `FREEZE_ELIGIBLE`, and the trigger that escalates the ask kind when it is false while
`FREEZE_ELIGIBLE` is also false.

**Termination, then, is a two-line proof rather than an assertion:**

> `clarify_turns` is monotone non-decreasing, incremented exactly once per `CLARIFY` turn and never
> decremented (teach turns do not increment it — blueprint §5.2). `CONTINUE` is false whenever
> `clarify_turns ≥ 12`. Therefore the dialogue emits at most 12 clarify turns per module, and every path
> exits to exactly one of `FREEZE_CONFIRM` (eligible) or `SPLIT_PROPOSED` (ceiling). ∎

`kills (structural)`: the unbounded loop is not "prevented by a check," it is unrepresentable — there is
no state transition that leaves `PROPOSE` more than 12 times.

### 3.2 Why `non_goals` is in `S` but not in `REQUIRED` — forced, not chosen

At the pristine state every slot has `c_s = 0`, `u_s = 1`, so `EIG(q_s) = (w_s / Σw) · ρ_s`:

| Slot | `w_s` | ask kind | `ρ_s` | `EIG` at pristine |
|---|---|---|---|---|
| `registry_verdict` | 0.6 | `choose` (closed set: install/extract/build_new) | 0.9 | **`0.135`** ← argmax |
| `interface` | 1.0 | `answer`, compositional | 0.5 | `0.125` |
| `acceptance` | 1.0 | `answer`, compositional | 0.5 | `0.125` |
| `deps` | 0.6 | `answer`, atomic | 0.8 | `0.12` |
| `data_owned` | 0.6 | `answer`, compositional | 0.5 | `0.075` |
| `non_goals` | 0.2 | `answer`, atomic | 0.8 | **`0.04`** — below `ε_gain = 0.05` |

`non_goals` **can never be asked**, from any state, because `0.04 < 0.05` is its *maximum*. That is not a
bug — it is blueprint scar #4 working exactly as designed: *"it was interrogating low-risk slots
(`non_goals`) while passing thin `acceptance` lines."* But if `non_goals` were in the coverage conjunct,
`c_non_goals` would sit at `0.0` forever and **nothing could ever freeze**.

So `REQUIRED = S \ {non_goals}` — five slots. The justification is not "otherwise it deadlocks"; it is
that **the authority does not require it either**:

- `ModuleBrief.non_goals` is `list[str]` with **no `min_length`** — `non_goals: []` is a schema-valid brief.
- keel's 14 check IDs are `C1-OPEN, C2-OWNER, C3-ACC-PARSE, C3-ACC-GROUND, C3-ACC-NONTAUT, R17-DERIV,
  R19-ABSOLUTE, R21-ALTS, R21-FAIL, C12-STORE, C12-DEPS, REG-VERDICT, IFACE, SHAPE`. **There is no
  `non_goals` check.**

Requiring it dialogue-side would make the orb *stricter than keel*, which blueprint §6.2 forbids in
substance: the orb's copy is advisory, so an orb stricter than the authority just refuses to offer
freezes keel would grant. `non_goals` still contributes to `H(B)` (a permanent `0.2/4.0 = 0.05` floor when
unfilled), so an unexplored non-goal keeps residual ambiguity honest without ever blocking.

**Emergent property worth naming, because it was not designed in:** the argmax's first question is the
**C1 registry question** — matching Company-OS C1/L2's own "registry first, before any build" law — purely
because `registry_verdict` is the one slot with a closed option set and therefore the most resolving
question available. §5's T1.4 pins it so a later weight change cannot lose it silently.

---

## 4. The interface — exactly what ships

Five new files, two additive edits. **Nothing else may change.**

### 4.1 NEW — `orb/backend/relay-py/src/orb_relay/build/freeze_protocol.py`

Pure module: **no I/O, no clock, no model call, no subprocess, no `fastapi`, no `store.*` import.** Same
firewall discipline `belief.py` earned in F04, and §5's A6 asserts it by AST parse, not by comment.

```python
# ---- Constants. Every one carries its source; [A] = assumed, blueprint's own label. -------------
EPSILON_GAIN            = 0.05   # blueprint §3.2 [A] — the info-gain floor
THETA_COV               = 0.80   # blueprint §3.2 [A] — per-slot coverage bar == exactly 2×(κ=0.4)
THETA_AMB               = 0.15   # blueprint §3.2 [A] — residual-ambiguity ceiling
SPLIT_TRIGGER_TURNS     = 12     # blueprint §3.4 [A] — the old cap, re-aimed to a split signal
NO_PROGRESS_TURNS       = 3      # blueprint §3.4 [A] — N consecutive ΔH < ε_gain arms the escape
MIN_SLOT_VALUE          = 0.5    # F05 §4.1a — NOT in the blueprint; see the note below

SLOT_WEIGHT: Mapping[CoverageSlot, float] = {   # build-risk weights, §4.1b
    CoverageSlot.INTERFACE:        1.0,
    CoverageSlot.ACCEPTANCE:       1.0,
    CoverageSlot.DATA_OWNED:       0.6,
    CoverageSlot.DEPS:             0.6,
    CoverageSlot.REGISTRY_VERDICT: 0.6,
    CoverageSlot.NON_GOALS:        0.2,
}
SUM_SLOT_WEIGHT = 4.0            # exact; see §4.1b

REQUIRED_SLOTS: tuple[CoverageSlot, ...]   # the five of §3.2 — CoverageSlot order, minus NON_GOALS
COMPOSITIONAL_SLOTS = {INTERFACE, DATA_OWNED, ACCEPTANCE}   # §4.1c
CLOSED_SET_SLOTS    = {REGISTRY_VERDICT}                    # §4.1c

RHO_ANSWER_COMPOSITIONAL = 0.5   # blueprint §3.2 "a compositional slot resolves less"
RHO_ANSWER_ATOMIC        = 0.8   # blueprint §3.2 "a direct self-report … ρ ≈ 0.8"
RHO_CHOOSE               = 0.9   # F05 [A] — between a self-report and a binary
RHO_CONFIRM              = 1.0   # blueprint §3.4 "maximally resolving (ρ → 1)"

# ---- Pure functions ------------------------------------------------------------------------------
class AskKind(str, Enum):  ANSWER = "answer";  CHOOSE = "choose";  CONFIRM = "confirm"

def slot_uncertainty(value: float) -> float
    """u_s = binary entropy H2(value) = -v·log2(v) - (1-v)·log2(1-v); 0.0 at v ∈ {0,1}."""

def ask_kind_for(slot: CoverageSlot, *, escalated: bool) -> AskKind
    """CONFIRM if escalated; else CHOOSE for a closed-set slot; else ANSWER. Derived from the
    ModuleBrief schema's own shape, never from taste."""

def resolving_power(slot: CoverageSlot, kind: AskKind) -> float

def residual_ambiguity(registers: Registers) -> float
    """H(B) = Σ_{s∈S} w_s·(1−c_s)·u_s / Σw   — over ALL SIX slots (blueprint §3.2)."""

def expected_information_gain(registers: Registers, slot: CoverageSlot, kind: AskKind) -> float
    """EIG = (w_s/Σw)·(1−c_s)·u_s·ρ(s,kind)."""

@dataclass(frozen=True, slots=True)
class QuestionCandidate:
    slot: CoverageSlot;  kind: AskKind;  eig: float

def best_question(registers: Registers, *, escalated: bool) -> QuestionCandidate
    """argmax_s EIG. Ties broken by CoverageSlot declaration order (first wins) — deterministic,
    asserted by T1.5."""

def keep_asking(registers: Registers) -> bool
    """Blueprint §3.2's AMBIGUITY-AXIS stop ONLY: max_s EIG(q_s, non-escalated) ≥ ε_gain.
    This is NOT the loop's continue-condition — see §3.1 and `continue_dialogue`."""

def coverage_ok(registers: Registers) -> bool
    """∀ s ∈ REQUIRED_SLOTS: c_s ≥ θ_cov ∧ value_s ≥ MIN_SLOT_VALUE.   (§4.1a)"""

@dataclass(frozen=True, slots=True)
class EligibilityVerdict:
    eligible: bool
    coverage_ok: bool;  ambiguity_ok: bool;  contradiction_clear: bool
    depth_pass: bool;   info_gain_exhausted: bool
    residual_ambiguity: float
    best_question: QuestionCandidate
    blocking: tuple[str, ...]        # e.g. ("COVERAGE:data_owned", "DEPTH:C3-ACC-NONTAUT")

def freeze_eligible(registers: Registers, *, depth_pass: bool) -> EligibilityVerdict
    """Blueprint §3.2's FREEZE_ELIGIBLE, all five conjuncts, each reported separately so a refusal
    always names its reason."""

def continue_dialogue(registers, *, depth_pass: bool, clarify_turns: int) -> bool
    """¬freeze_eligible(...).eligible ∧ clarify_turns < SPLIT_TRIGGER_TURNS.  (§3.1's proof)"""

def escalation_armed(registers, *, depth_pass, clarify_turns, ambiguity_history: Sequence[float]) -> bool
    """True when the §3.4 escape should fire, for EITHER of two independent reasons:
       (i)  continue_dialogue(...) ∧ ¬keep_asking(...)        — the §3.1 stall
       (ii) the last NO_PROGRESS_TURNS deltas of `ambiguity_history` are each < EPSILON_GAIN
    """

def next_move(registers, *, depth_pass, clarify_turns, ambiguity_history) -> Move
    """The one entry point app.py calls. Returns exactly one of:
         Move.Freeze()                       — FREEZE_ELIGIBLE
         Move.Split(reason)                  — clarify_turns >= SPLIT_TRIGGER_TURNS
         Move.Ask(QuestionCandidate)         — otherwise, escalated iff escalation_armed(...)
       Total: every reachable input maps to exactly one variant (T4.4 asserts exhaustiveness)."""
```

**§4.1a — `MIN_SLOT_VALUE`, a disclosed addition to the blueprint.** Under §2.5's reading, a slot driven
by *negative* evidence reaches `c_s = 0.8` (covered) while `value = 0.08317269649392238` — the register is
*confident the slot is not filled*, and coverage would pass. No negative-weight evidence producer exists
in the tree today (F04 emits only `+1.2` / `+0.5`), so the hole is **unreachable in production** — which
is exactly why it must be closed structurally rather than left to that accident. `MIN_SLOT_VALUE = 0.5`
is the register's own prior, so the guard never rejects a state reachable by positive evidence. Labelled
`mitigates`, not `kills`, and T6 makes it **reachable in test** — F06 §4.2's lesson ("`MeasuredNothing`
must be *reachable*") applied to a different unreachable branch.

**§4.1b — where the weights come from.** `w_s` is *"how much a wrong guess there costs a lane"*
(blueprint §3.2). Derived from where each slot's error is caught, not from feel:

| Slot | `w` | A wrong value here is caught… | Cost |
|---|---|---|---|
| `acceptance` | 1.0 | **nowhere upstream** — it *is* the oracle the fleet verify stage runs (F02 `AcceptsWhen` → blind-suite seed), so the lane builds, verifies and attests the wrong thing | full lane + a false attestation |
| `interface` | 1.0 | at integration, after every consumer lane has already been briefed against it | ≥ 1 lane, blast radius > 1 |
| `data_owned` | 0.6 | mechanically, by keel's `C12-STORE` before a lane spawns | one clarify round |
| `deps` | 0.6 | mechanically, by keel's `C12-DEPS` | one clarify round |
| `registry_verdict` | 0.6 | by keel's `REG-VERDICT`, then F25's registry gate | one duplicated build |
| `non_goals` | 0.2 | in human review at PR | scope creep |

`Σw = 4.0` **exactly** — chosen, not coincidental: it makes every `EIG` at the pristine state a short
terminating decimal (§3.2's table), so the acceptance suite's oracle can be *read and checked by a
reviewer* rather than trusted from a script. If a future lane re-tunes a weight, keep `Σw = 4.0`.

**§4.1c — compositional vs. atomic is read off the real schema, not chosen.** A slot is *compositional*
iff its `ModuleBrief` field is a list of **objects** (`interface: list[InterfaceDecl]`,
`data_owned: list[DataDecl]`) or is itself a multi-field record (`acceptance: AcceptanceLine`, five
sub-fields). It is *atomic* iff the field is a list of strings (`deps`, `non_goals`). `registry` is a
discriminated union over three `kind` values → a closed set → `choose`. T1.6 asserts this partition
against `lld_schemas` by introspection, so a schema change that adds a field cannot leave the partition
stale.

### 4.2 NEW — `orb/backend/relay-py/src/orb_relay/build/readiness.py`

The keel seam. **Contains zero rule logic** (§2.2).

```python
class ReadinessOutcome(str, Enum):
    READY = "ready"; NOT_READY = "not_ready"; UNAVAILABLE = "unavailable"

@dataclass(frozen=True, slots=True)
class ReadinessVerdict:
    outcome: ReadinessOutcome
    exit_code: int | None
    stdout: str
    stderr: str
    def is_pass(self) -> bool: return self.outcome is ReadinessOutcome.READY

class ReadinessGate(Protocol):
    def evaluate(self, brief: ModuleBrief) -> ReadinessVerdict: ...

class SubprocessReadinessGate:
    """Authority: keel. Writes brief.model_dump(mode="json") to a NamedTemporaryFile, runs
    `<fleet_bin> gate lld-ready <path>` with a hard timeout, deletes the file, returns the verdict."""
    def __init__(self, fleet_bin: str | Path, *, timeout_s: float = 10.0) -> None: ...

class UnavailableReadinessGate:
    """Returned when FLEET_BIN is unset or not executable. ALWAYS returns UNAVAILABLE — never READY."""
```

**The exit-code contract, read out of `main.rs` (not guessed):**

| Exit | keel's meaning | `ReadinessOutcome` |
|---|---|---|
| `0` | `READY (<path>) -- P/T checks passed` | `READY` |
| `6` (`EXIT_INVARIANT`) | **`NOT_READY`** *or* **`MEASURED_NOTHING`** — the two share one code | `NOT_READY` |
| `8` (`EXIT_MISMATCH`) | `SHAPE INVALID … the gate was not reached` | `NOT_READY` |
| `3` (`EXIT_ENV`) | unreadable file / bad JSON / missing `owners.v1.json` | `UNAVAILABLE` |
| `7` (`EXIT_REFUSAL`) | usage error | `UNAVAILABLE` |
| timeout / `FileNotFoundError` | — | `UNAVAILABLE` |

**§4.2a — two layers, and the bad fixture never reaches the gate.** `gate lld-ready <file>` calls
`check_and_evaluate`, which runs a **shape** layer first and only then the gate; `--selftest` calls
`evaluate` **directly**, deliberately bypassing shape (F06 §5.4). Consequence, confirmed by F06's
verifier on the live binary — *"the bad fixture run directly (not via `evaluate`) → caught by the shape
layer with exit 8 before the gate runs at all"*:

> **`one_line_freeze.json` through the CLI yields exit 8 (`SHAPE INVALID`), not exit 6.** So does
> `one_alternative.json` (`alternatives` has `min_length=2` in the schema itself). **No checked-in
> fixture yields exit 6.** To exercise `NOT_READY` you must derive a shape-valid, gate-invalid brief
> in-test — take `complete_module.json` and break exactly one thing the *schema* does not check but the
> *gate* does. Three that I verified are available, against the real reference files:
> **(a)** `owner` — `owners.v1.json` holds exactly `["rachit@devxlabs.ai"]`, which is precisely
> `complete_module.json`'s `owner`, so any other string fails `C2-OWNER`. **(b)** `registry.searched`
> — the fixture's verdict is `{"kind":"build_new","searched":[…]}` and those two paths are exactly
> `gate-refs.v1.json:registry_paths`, so replacing one fails `REG-VERDICT`. **(c)** a `number`
> derivation whose `calc` carries no arithmetic operator fails `R17-DERIV`.
> **Mutate a parsed copy in memory. Never edit a fixture file on disk** (F06 §6's hard rule).

This is why §4.2's rule 1 matters more than it looks: had the seam tried to distinguish 6 from 8, the
only fixture available would have taught it the wrong mapping.

**§4.2b — the pydantic round-trip is unverified; determine it empirically in §6 Step 2.**
`ModuleBrief.model_dump(mode="json")` may emit explicit `null`s for optional fields the fixture omits
(e.g. `NumberDerivation.source`). Whether keel's shape layer accepts an explicit `"source": null` is
**not known** and must not be assumed: run `fleet gate lld-ready` against a dumped-and-rewritten copy of
`complete_module.json` and compare to the original before writing `readiness.py`. If it differs, dump
with `exclude_none=True` and say so in the evidence file. A silent shape rejection here would make
**every** freeze attempt return `NOT_READY` while every unit test stayed green.

Three rules on this seam, each load-bearing:

1. **Only exit `0` is a pass.** `NOT_READY` and `MEASURED_NOTHING` are indistinguishable by exit code, so
   do **not** try to tell them apart; both are "may not freeze," which is the correct fail-closed reading
   and the one that cannot rot when keel's stderr wording changes.
2. **`stdout`/`stderr` are captured verbatim into the verdict and never parsed.** The `P/T checks passed`
   count is deliberately *not* extracted — a regex over another repo's human-readable output is a
   drift surface with no test on the other side. It is carried as evidence for a human, not as data.
3. **`UNAVAILABLE` refuses the freeze.** It is never treated as "assume ready," never skipped, and never
   silently downgraded to a dialogue-only eligibility check. T5.4 pins this; blueprint §9's
   *"every ambiguous state resolves toward keep clarifying / do not freeze."*

### 4.3 EDIT (additive only) — `orb/backend/relay-py/src/orb_relay/proxy/schemas.py`

New models. **No existing model may lose or change a field.** All fields are string/list/bool —
**no numeric field anywhere in `ProposalTurn`**, because §4.4's pushback guard canonicalises it and
`canonical_json` raises on numeric leaves (§8.3 L4).

```python
class AskKindWire(str, Enum):   CONFIRM="confirm"; CHOOSE="choose"; ANSWER="answer"

class Ask(BaseModel):                       # blueprint §5.1
    kind: AskKindWire
    question: Annotated[str, Field(min_length=1)]
    options: list[str] = []                 # non-empty iff kind is CHOOSE (validator)
    slot: str | None = None                 # the CoverageSlot this targets, or None for a depth ask

class ProposalTurn(BaseModel):              # blueprint §5.1 — the no-naked-proposal type
    kind: Literal["PROPOSE"]
    target: Annotated[str, Field(min_length=1)]          # node_id
    proposal: Annotated[str, Field(min_length=1)]
    why:      Annotated[str, Field(min_length=1)]
    achieves: list[str]
    pros:     list[str]
    cons:     Annotated[list[str], Field(min_length=1)]  # §5.1: MUST be non-empty
    killed:   Annotated[list[KilledAlt], Field(min_length=2)]  # §5.1: ≥ 2 — reuses F02's KilledAlt
    ask:      Ask

class ReadinessVerdictWire(BaseModel):
    outcome: Literal["ready","not_ready","unavailable"]
    exit_code: int | None
    detail: str                              # stdout+stderr, verbatim, for a human

class FreezeProposal(BaseModel):
    """The ❄ event. NOT an lld.v1 Freeze — see §2.1. `proposed_by` is deliberately a DIFFERENT
    const from Freeze.stamped_by so no coercion between the two types can typecheck."""
    proposed_by: Literal["orb:dialogue"]
    node_id:      Annotated[str, Field(min_length=1)]
    content_hash: Annotated[str, Field(pattern=r"^sha256:[0-9a-f]{64}$")]  # of the ModuleBrief only
    brief:        ModuleBrief
    readiness:    ReadinessVerdictWire
    clarify_turns_used: int
    residual_ambiguity: float
    coverage: list[RegisterReading]          # reuses F04's wire shape

class FreezeProtocolState(BaseModel):        # additive on BuildTurnState
    move: Literal["ask","freeze","split"]
    residual_ambiguity: float
    best_question_slot: str | None
    best_question_kind: AskKindWire | None
    best_question_eig: float
    escalated: bool
    clarify_turns: int
    blocking: list[str]                      # every unmet FREEZE_ELIGIBLE conjunct, named
```

`BuildTurnState` gains **one** optional field, `protocol: FreezeProtocolState | None = None`.
`ConversationResponse` gains **two**: `proposal: ProposalTurn | None = None` and
`freeze: FreezeProposal | None = None`. Both default `None`, so every non-build caller's body is
byte-identical to today's (A7.3 asserts it).

### 4.4 EDIT (additive only) — `orb/backend/relay-py/src/orb_relay/app.py`

**One block, inside the existing `if body.mode is ResponseMode.BUILD:` at line 845, after the existing
`registers = _build_session_store.registers(...)` at line 922.** Nothing above it moves.

```
a. Tier-0 contradiction rule (§4.5) over the accumulated user text → if it fires, append
   Evidence(register=Register.CONTRADICTION, slot=None, weight=+2.0, reliability=1.0, tier=TIER0)
   AND a PremiseRevised for the conflicting slot's dependents (build_slot_dependencies()).
   Re-read `registers` after appending.
b. clarify_turns := count of this session's CLARIFY turns, derived from the persisted evidence log
   (never a new counter, never in-process state — F04 §2.2 killed in-process session state).
c. depth_pass := False when there is no validated brief this turn; otherwise
   readiness_gate.evaluate(decompose_outcome.brief).is_pass().
   ** The gate is only ever consulted when a brief exists. No brief ⇒ no gate call ⇒ no subprocess. **
d. move := freeze_protocol.next_move(registers, depth_pass=..., clarify_turns=...,
                                     ambiguity_history=...)
e. move is Ask   → build FreezeProtocolState; leave `freeze` None.
   move is Split → FreezeProtocolState(move="split"); leave `freeze` None.
   move is Freeze→ FreezeProposal(..., content_hash=content_hash(brief.model_dump(mode="json")), ...)
f. The spoken reply (`text`, `beats`) is COMPLETELY UNCHANGED by all of the above. The gate
   subprocess runs after the guarded completion has already produced `text` — it can add latency to
   the JSON body, never to the spoken path (blueprint §4.3's off-the-hot-path rule).
```

Module-level, alongside the other singletons: `_readiness_gate` resolved once at import from
`FLEET_BIN` (falling back to `UnavailableReadinessGate`), and overridable via
`app.dependency_overrides` for tests.

### 4.5 The contradiction rule — minimal, Tier-0, and *reachable*

`FREEZE_ELIGIBLE` has `Contradiction = 0` as a **hard** conjunct. F04 shipped
`is_contradiction_blocking()` reading the real register, and documented honestly that *"nothing in F04
emits `Contradiction` evidence yet… the register never leaves its zero-confidence prior."* A hard
conjunct that no input can ever falsify is the same defect class F06 §4.2 had to fix
(`checked == 0 ⇒ MeasuredNothing` was unreachable).

F05 ships the blueprint's **own named example** as a deterministic Tier-0 rule, and nothing more:
mutually-exclusive keyword pairs over the session's accumulated user text — `{stateless, no storage,
in-memory only}` × `{persist, database, survives restart, across sessions}` (blueprint §8's exact
trace). Same class of stand-in as F04's `_SLOT_KEYWORDS`, disclosed the same way: **no model call, no
NLU claim, and explicitly not a real contradiction detector.** Its job is to make the conjunct
falsifiable. `is_contradiction_blocking()` is reused unchanged as the reader.

---

## 5. Acceptance suite — write these FIRST; they are the spec

**Three new files. All red until the implementation exists — that is correct and expected (T1/T2).**
Run: `cd orb/backend/relay-py && .venv/bin/pytest -q`.

> **The builder may not edit these files, or any file under `tests/`.** If a test looks wrong, **stop and
> escalate to the lead** — F03's and F08's builders both did exactly that and were both right.

**Provenance of every number below.** All expected values were computed **by the lead, from §4.1's
formulas and F04's published update law, before any F05 implementation existed** — using an
independent scratch calculator that re-derives `sigmoid`/`logit`/`κ`/`clamp` from `belief.py`'s
*docstring* rather than importing it. They are therefore an oracle, not a transcript of behaviour.
Assert with `pytest.approx(..., abs=1e-12)`; values marked **exact** must also satisfy `== `.

### T1 — `tests/test_f05_freeze_protocol.py` — the stopping rule at the pristine state

State: `initial_registers()` — every slot `(value=0.5, confidence=0.0)` ⇒ `u_s = 1.0`, `(1−c_s) = 1.0`.

| # | Assertion | Expected |
|---|---|---|
| **T1.1** | `residual_ambiguity(initial_registers())` | **exactly `1.0`** |
| **T1.2** | `sum(SLOT_WEIGHT.values())` and `SUM_SLOT_WEIGHT` | **exactly `4.0`** (both; pins §4.1b) |
| **T1.3** | `expected_information_gain` per slot, non-escalated | `interface` **`0.125`** · `acceptance` **`0.125`** · `registry_verdict` **`0.135`** · `deps` **`0.12`** · `data_owned` **`0.075`** · `non_goals` **`0.04`** |
| **T1.4** | `best_question(pristine, escalated=False)` | `.slot is CoverageSlot.REGISTRY_VERDICT`, `.kind is AskKind.CHOOSE`, `.eig == 0.135`. **The C1-first property of §3.2** — a weight change that loses it must fail here. |
| **T1.5** | Tie-break determinism: `interface` and `acceptance` both score `0.125`; with `registry_verdict` driven to `confidence = 1.0`, `best_question(...).slot is CoverageSlot.INTERFACE` | declaration order wins, no dict-ordering luck |
| **T1.6** | `COMPOSITIONAL_SLOTS` matches the schema partition: for each slot, introspect the matching `ModuleBrief` field annotation — `list[<BaseModel>]` or a `BaseModel` ⇒ compositional; `list[str]` ⇒ atomic | pins §4.1c against `lld_schemas`, so a schema change cannot leave it stale |
| **T1.7** | `keep_asking(pristine)` | `True` (max EIG `0.135` ≥ `0.05`) |
| **T1.8** | `slot_uncertainty(0.5) == 1.0`; `slot_uncertainty(1.0) == 0.0`; `slot_uncertainty(0.0) == 0.0`; `slot_uncertainty(0.973403006423134)` | `0.17702772704896477`; no `ValueError`, no `nan` at the endpoints |

### T2 — convergence: the freeze branch (same file)

Register trajectory used throughout, from `initial_registers()` with F04's own
`_TURN_EVIDENCE_WEIGHT = 1.2`, `r = 1.0`, Tier-0 (**these four rows are the oracle for every state
below; T2.1 asserts them directly so a wrong `belief.py` cannot silently shift the rest**):

| after | `logit` | `value` | `confidence` | `u = H₂(value)` |
|---|---|---|---|---|
| 1 × | `1.2` | `0.7685247834990175` | `0.4` | `0.780574086303188` |
| 2 × | `2.4` | `0.9168273035060777` | `0.8` | `0.4132608943283298` |
| 3 × | `3.6` | `0.973403006423134` | `1.0` | `0.17702772704896477` |
| 4 × | `4.8` | `0.9918374288468401` | `1.0` | `0.06834971013683284` |

**T2.2 — the exact freeze point.** All five `REQUIRED_SLOTS` at **2 ×**; `non_goals` untouched:

- `residual_ambiguity(...)` == `0.12851956992238264` — and `≤ THETA_AMB` ✓
- `coverage_ok(...)` is `True` (every required `c_s == 0.8`, `≥ 0.80` **on the boundary** — an
  implementation using `>` instead of `>=` fails exactly here, and nowhere else)
- per-slot EIG: `interface` `0.010331522358208242` · `acceptance` `0.010331522358208242` ·
  `registry_verdict` `0.011158044146864903` · `deps` `0.009918261463879913` ·
  `data_owned` `0.0061989134149249454` · `non_goals` `0.04000000000000001`
- `keep_asking(...)` is `False` (max is `non_goals` at `0.04` < `0.05`)
- `freeze_eligible(..., depth_pass=True).eligible` is **`True`**, `.blocking == ()`
- `freeze_eligible(..., depth_pass=False).eligible` is **`False`**, and `.blocking` contains a
  `"DEPTH"`-prefixed entry — **coverage without depth does not freeze** (blueprint §3.2's load-bearing
  sentence)

**T2.3 — three evidences.** All five required at **3 ×**: `residual_ambiguity` == **exactly `0.05`**
(the pure `non_goals` floor, `0.2/4.0`), max EIG `0.04000000000000001`.

**T2.4 — the `non_goals` floor is structural.** `residual_ambiguity` over *any* register state with
`non_goals` at its prior is `≥ 0.05`; and `best_question(...).slot` is **never** `NON_GOALS` when any
required slot is uncovered. Derive the `0.05` in the test from `SLOT_WEIGHT[NON_GOALS] / SUM_SLOT_WEIGHT`,
not as a literal, so a weight change moves the assertion with it.

**T2.5 — `non_goals` never blocks.** With all five required at 3 × and `non_goals` at its prior,
`freeze_eligible(..., depth_pass=True).eligible` is `True`. Pins §3.2.

### T3 — the stall, and the escape (same file) — **the regression pin for §3.1**

Construct §3.1's exact state: `data_owned` at **1 ×**, `interface`/`acceptance`/`deps`/`registry_verdict`
at **3 ×**, `non_goals` at its prior.

| # | Assertion | Expected |
|---|---|---|
| **T3.1** | `residual_ambiguity(...)` | `0.12025166776728692` — and `≤ THETA_AMB` |
| **T3.2** | `coverage_ok(...)` | `False` (`data_owned` at `0.4`) |
| **T3.3** | `keep_asking(...)` | **`False`** — `max EIG` is `non_goals` at `0.04000000000000001`; `EIG[data_owned, ANSWER] == 0.035125833883643459` |
| **T3.4** | `escalation_armed(..., clarify_turns=5, ambiguity_history=[])` | **`True`** — armed by reason (i), the stall, with **no** history of stalled turns needed |
| **T3.5** | `best_question(..., escalated=True)` | `.slot is DATA_OWNED`, `.kind is AskKind.CONFIRM`, `.eig == 0.07025166776728692` (`≥ ε_gain`) |
| **T3.6** | `next_move(..., depth_pass=True, clarify_turns=5, ambiguity_history=[])` | `Move.Ask` with the T3.5 candidate — **not** `Freeze`, **not** `Split`, and **not** a no-op |
| **T3.7** | **The negative control this suite would be worthless without.** A build of `next_move` whose continue-condition is `keep_asking(...)` returns no ask at this state. Assert the shipped `next_move` returns an `Ask`, then assert `keep_asking(...) is False` **in the same test** — so the test body itself exhibits that the two predicates disagree here. | both |

**T3.8 — the second escape trigger, independently.** From a state where `keep_asking(...)` is `True`,
`escalation_armed(..., ambiguity_history=[0.40, 0.38, 0.37, 0.36])` (three consecutive deltas of
`0.02, 0.01, 0.01`, each `< 0.05`) is `True`; with `[0.40, 0.30, 0.20, 0.10]` (deltas `0.10`) it is
`False`. Blueprint §3.4's `N = 3`.

### T4 — termination: the never-answered branch (same file)

State: `acceptance` at its prior; `interface`/`data_owned`/`deps`/`registry_verdict` at **3 ×**;
`non_goals` at its prior. This is FEATURES.md's *"a slot deliberately never answered."*

| # | Assertion | Expected |
|---|---|---|
| **T4.1** | `residual_ambiguity(...)` | **exactly `0.3`** (`= (1.0 + 0.2)/4.0`) — `> THETA_AMB`, so ambiguity blocks too |
| **T4.2** | `best_question(..., escalated=False)` | `.slot is ACCEPTANCE`, `.eig == 0.125`; escalated: `.eig == 0.25` |
| **T4.3** | `freeze_eligible(..., depth_pass=True).eligible` is `False`; `.blocking` contains **both** a coverage entry naming `acceptance` **and** an ambiguity entry | a refusal always names every reason, not the first |
| **T4.4** | **Termination, driven.** Loop `next_move(...)` with `clarify_turns` incrementing 0,1,2,… and the registers **frozen** (the worst case: the user answers nothing that moves any register). Assert: every call for `clarify_turns` in `0..11` returns `Move.Ask`; the call at `clarify_turns == 12` returns `Move.Split`; and the loop exits in **exactly 13 iterations**. Assert `SPLIT_TRIGGER_TURNS == 12` separately so the 13 is derived, not magic. |
| **T4.5** | `next_move` is **total**: over the cross-product of `{pristine, 1×, 2×, 3× on each of the 6 slots}` × `depth_pass ∈ {T,F}` × `clarify_turns ∈ {0, 11, 12, 13}`, every call returns exactly one `Move` variant and never raises. Catches a half-finished dispatch. |
| **T4.6** | Coverage is a **hard** conjunct: with `depth_pass=True`, ambiguity `≤ θ_amb`, contradiction clear, and `keep_asking` `False`, but one required slot at `c_s = 0.79`, `freeze_eligible(...).eligible` is `False`. The `0.79`/`0.80` pair is the boundary test. |

### T5 — `tests/test_f05_readiness_gate.py` — the keel seam, against the REAL binary

Not a mock. Build once: `cargo build --manifest-path fleet/keel/Cargo.toml`; skip the file with an
explicit `pytest.skip("fleet binary not built")` **only** if the binary is genuinely absent — a skip is
recorded as a skip, never counted as a pass.

| # | Assertion | Expected |
|---|---|---|
| **T5.1** | `SubprocessReadinessGate(...).evaluate(<complete_module.json parsed to ModuleBrief>)` | `.outcome is READY`, `.exit_code == 0`, `"READY" in .stdout`, and `"14/14"` in `.stdout` (F06's verifier confirmed `14/14 checks passed`, twice, byte-identical). Uses F06's own control fixture — **do not hand-author a brief.** |
| **T5.2** | **Exit 6 (`NOT_READY`) requires a shape-VALID, gate-invalid brief — see §4.2a.** Load `complete_module.json`, change **`owner`** to a string absent from `fleet/contracts/owners.v1.json`, in memory only. | `.outcome is NOT_READY`, `.exit_code == 6`, `"NOT_READY" in .stdout`, `"C2-OWNER" in .stderr`, `.is_pass() is False` |
| **T5.3** | Exit 8 (`SHAPE INVALID`) — `one_line_freeze.json`'s brief. **This does NOT reach the gate** (§4.2a). | `.outcome is NOT_READY`, `.exit_code == 8`, `"SHAPE INVALID" in .stdout` — 8 must not be mistaken for a pass and must not raise |
| **T5.4** | `UnavailableReadinessGate().evaluate(<the valid complete_module brief>)` | `.outcome is UNAVAILABLE`, `.is_pass() is False`. **A gate that cannot run must refuse, not assume.** |
| **T5.5** | `SubprocessReadinessGate("/nonexistent/fleet")` | `.outcome is UNAVAILABLE`; **no exception escapes**; a build turn using it still returns HTTP 200 |
| **T5.6** | `ReadinessVerdict` carries `stdout`/`stderr` verbatim and the module contains **no** regex/`split()` over them (AST scan for `re.` and `.split(` in `readiness.py`) | pins §4.2 rule 2 |
| **T5.7** | `readiness.py` contains **none** of the 14 check-id strings (`C1-OPEN`, `R17-DERIV`, …) — grep its source | the orb holds **no** copy of the rubric (§2.2) |

### T6 — the guards (in `test_f05_freeze_protocol.py`)

| # | Assertion | Expected |
|---|---|---|
| **T6.1** | **`MIN_SLOT_VALUE` is reachable.** Fold `[Evidence(COVERAGE, interface, w=-1.2, r=1.0, TIER0)] × 2` → `value == 0.08317269649392238`, `confidence == 0.8`. Then `coverage_ok` is `False` **even though `c_s ≥ θ_cov`**, and `freeze_eligible(...).blocking` names `interface`. Constructing the negative evidence in the test *is* the point (§4.1a). |
| **T6.2** | **Contradiction is hard and reachable.** Append `Evidence(CONTRADICTION, None, w=+2.0, r=1.0, TIER0)`; `is_contradiction_blocking(registers)` is `True`; `freeze_eligible(..., depth_pass=True).eligible` is `False` with `"CONTRADICTION"` in `.blocking` — **from a state that was eligible before the append** (T2.2's). Without that before/after pair the test proves nothing. |
| **T6.3** | **`ProposalTurn` refuses a naked proposal.** `cons=[]` → `ValidationError`; `killed=[<one>]` → `ValidationError`; `killed=[<two valid KilledAlt>]`, `cons=["…"]` → constructs. Blueprint §5.1. |
| **T6.4** | **The pushback no-repeat guard.** `canonical_json(turn_a.model_dump(mode="json")) == canonical_json(turn_a2.model_dump(...))` for two turns differing only in key order; `!=` for two differing in `proposal`. Reuses F02's `canonical_json` and asserts it does **not** raise — i.e. `ProposalTurn` has no numeric leaf (§8.3 L4). |
| **T6.5** | **The firewall.** AST-parse `freeze_protocol.py`: imports **none** of `fastapi`, `subprocess`, `store.*`, `gateway_client`, `readiness`, `time`, `datetime`. The stopping rule cannot reach a clock, a process, or a socket. |
| **T6.6** | **`belief.py` is untouched.** `git diff master -- src/orb_relay/cognitive/belief.py` is empty. Also assert F04's own `tests/test_f04_belief_registers.py` and `test_f04_build_session_store.py` show an empty diff. |

### T7 — `tests/test_f05_convergence.py` — end to end, over the relay, from a raw client

A real `uvicorn` subprocess on an ephemeral port + a scripted stub gateway, driven by `httpx` — the same
shape as `test_f04_build_mode_wire.py`'s A1, for the same reason (`TestClient` cannot catch an
import-time failure). Reuse `fleet/contracts/fixtures/lld/complete_module.json` as the scripted brief
(the lane contract's fixture-reuse rule; **do not hand-author one**).

| # | Assertion | Expected |
|---|---|---|
| **T7.1** | **The freeze branch.** Script the gateway so every BUILD turn's decompose returns `complete_module.json`. Drive turns until `body["freeze"]` is non-null. Assert it happens in **≤ 6 turns**, and that the turn count is `< SPLIT_TRIGGER_TURNS` — "bounded" measured, not claimed. |
| **T7.2** | The `freeze` body is a **`FreezeProposal`, not a `Freeze`**: `proposed_by == "orb:dialogue"`; **`"stamped_by"` is not a key anywhere in the response JSON** (recursive key scan). §2.1's structural assertion. |
| **T7.3** | `freeze.content_hash` equals `lld_schemas.content_hash(brief.model_dump(mode="json"))` recomputed in the test — the hash is of the **brief**, not of the proposal. |
| **T7.4** | **The never-answered branch.** Script decompose to return a `clarify_request` every turn (no brief ⇒ `depth_pass=False`). Drive **14** turns. Assert: `body["freeze"] is None` on all 14; `body["build"]["protocol"]["move"] == "ask"` for the first 12; `== "split"` from turn 13 on; and every response is **HTTP 200**. This is FEATURES.md's second half. |
| **T7.5** | **The stall does not hang, over the wire.** Script decompose to return a brief with an empty `data_owned` for turns 1–3 (so `data_owned` gets one evidence and stops), then nothing further. Assert `protocol.escalated` becomes `True` and `protocol.best_question_kind == "confirm"` on `data_owned` — §3.1's escape observed in a real response body, not just in a unit test. |
| **T7.6** | **Focus mode is untouched.** The same running server: `mode="focus"` → `build is None`, `proposal is None`, `freeze is None`, and the response body's key set is **byte-identical** to the pre-F05 body for the same input (capture it before the change). |
| **T7.7** | **The gate is not on the spoken path.** With `FLEET_BIN` pointed at a wrapper script that `sleep 3`s then exits 0, a BUILD turn still returns HTTP 200 and `text` is unchanged from the same turn run with the real binary. Pins §4.4(f). |
| **T7.8** | **No brief ⇒ no subprocess.** With `FLEET_BIN` pointed at a script that appends a line to a temp file on every invocation, run 5 turns whose decompose returns `clarify_request`. The file has **0 lines**. Pins §4.4(c). |

### T8 — the focus/regression witnesses (no new file)

`git diff master` must show **zero** changes to: `cognitive/belief.py`, `store/build_session_store.py`,
`store/freeze_store.py`, `proxy/lld_schemas.py`, `proxy/conversation_guard.py`, `proxy/atomizer.py`,
`proxy/lld_decomposer.py`, anything under `fleet/`, anything under `orb/apps/mobile/`, and any existing
file under `tests/`. Named check, run before commit:

```bash
cd "<worktree>" && git diff --name-only master -- \
  orb/backend/relay-py/src/orb_relay/cognitive/ \
  orb/backend/relay-py/src/orb_relay/store/ \
  orb/backend/relay-py/src/orb_relay/proxy/lld_schemas.py \
  orb/backend/relay-py/src/orb_relay/proxy/conversation_guard.py \
  orb/backend/relay-py/src/orb_relay/proxy/atomizer.py \
  orb/backend/relay-py/src/orb_relay/proxy/lld_decomposer.py \
  fleet/ orb/apps/ orb/backend/relay-py/tests/test_f0[1-4]*.py
# MUST print nothing.
```

### T9 — mutation adequacy (required before claiming green)

Green tests prove nothing until they can go red. Apply each, record the **real** failure text, revert via
`git checkout --` against a checkpoint commit (**never `git stash`** — §8.3 L3), confirm green again.
Paste the actual output in the evidence file.

| # | Mutation | Must break | Why it earns its place |
|---|---|---|---|
| M1 | `EPSILON_GAIN` `0.05 → 0.03` | T2.2 (`keep_asking` becomes `True` at the freeze point) | pins the floor, not just its existence |
| M2 | `THETA_COV` `0.80 → 0.81` | T2.2's boundary | catches `>` vs `>=` |
| M3 | `SLOT_WEIGHT[NON_GOALS]` `0.2 → 0.6` | T1.3, T2.3, **T2.4** | proves the `non_goals` floor is derived |
| M4 | `RHO_CONFIRM` `1.0 → 0.5` | **T3.5** only (escape EIG falls `0.0702… → 0.0351…`, below ε) | the escape loses its resolving power. **Stated precisely: T3.6 still returns an `Ask`**, because `continue_dialogue` does not consult EIG — so M4 degrades question quality and does *not* re-open the §3.1 hang. If it breaks anything beyond T3.5, that is a finding worth reporting, not a bonus. |
| M5 | `continue_dialogue` returns `keep_asking(...)` instead of `¬eligible ∧ turns < 12` | **T3.6, T3.7, T4.4, T4.5** — all four, traced: at T3's state `next_move` has no valid variant left (not Ask, not Freeze, not Split) so it raises or returns wrong; at T4's state `keep_asking` stays `True` past turn 12 so `Split` never fires | the exact misreading §2.3 kills |
| M6 | `SPLIT_TRIGGER_TURNS` `12 → 1000` | **T4.4** and **T7.4** (the wire test's `ask → split` flip at turn 13 never happens) | termination is asserted, not assumed |
| M7 | `add REQUIRED_SLOTS += (NON_GOALS,)` | T2.5 (nothing ever freezes) | proves §3.2 is load-bearing |
| M8 | `ReadinessVerdict.is_pass()` returns `outcome is not NOT_READY` | **T5.4** | `UNAVAILABLE` silently becomes a pass — the single most dangerous mutation here |
| M9 | `depth_pass` hardcoded `True` in `app.py`'s BUILD block | T7.4 | the gate stops being consulted; the wire test is the only witness |
| M10 | `coverage_ok` drops the `value_s ≥ MIN_SLOT_VALUE` term | **T6.1** | proves §4.1a's guard is reachable, not decoration |
| M11 | `FreezeProposal.proposed_by` → `"keel:lld-ready"` | **T7.2** | the forgery, caught at the wire |

M4, M5 and M8 are the three that matter most: each turns a *refusal* into a *silent pass*, which is this
estate's most-repeated failure shape.

---

## 6. How to drive this end to end manually (NOT optional)

Every command literal, from the worktree root
`/Users/rachitsrivastava/youtube/Principal Engineering/Light/.worktrees/F05-freeze-protocol`.
**Foreground, or `timeout N <cmd>`; never background-and-wait** (a subagent cannot receive its own
background children's notifications).

```bash
# Step 0 — GET ON MASTER FIRST. F06's gate does not exist in this worktree (§0.1a).
git merge --no-edit master && test -f fleet/keel/fleet/src/lld_ready.rs && echo "F06 present"

# Step 1 — the venv (absent in this worktree, §0.1h)
cd orb/backend/relay-py && python3 -m venv .venv \
  && .venv/bin/pip install --upgrade pip && .venv/bin/pip install -e ".[dev]"
.venv/bin/pytest -q          # RECORD THIS NUMBER. It is your zero-regression baseline.

# Step 2 — the keel binary the readiness gate shells out to
cargo build --manifest-path ../../../fleet/keel/Cargo.toml
export FLEET_BIN="$(cd ../../../fleet/keel/target/debug && pwd)/fleet"
"$FLEET_BIN" gate lld-ready ../../../fleet/contracts/fixtures/lld/complete_module.json; echo "exit=$?"
# EXPECT: "gate lld-ready: READY (...) -- N/14 checks passed", exit=0
"$FLEET_BIN" gate lld-ready ../../../fleet/contracts/fixtures/lld/one_line_freeze.json; echo "exit=$?"
# EXPECT: "SHAPE INVALID ... the gate was not reached", exit=8  (NOT 6 — see §4.2a)
# Capture `$?` DIRECTLY — never after a pipe (verify.sh:20).

# Step 2b — the exit-6 path, which no checked-in fixture reaches (§4.2a). Break `owner` only.
python3 - <<'EOF' > /tmp/f05-bad-owner.json
import json,pathlib
b=json.loads(pathlib.Path("../../../fleet/contracts/fixtures/lld/complete_module.json").read_text())
b["owner"]="nobody-at-all"; print(json.dumps(b))
EOF
"$FLEET_BIN" gate lld-ready /tmp/f05-bad-owner.json; echo "exit=$?"
# EXPECT: NOT_READY, "C2-OWNER" on stderr, exit=6

# Step 2c — THE ROUND-TRIP CHECK (§4.2b). Does a pydantic re-dump still pass keel's shape layer?
.venv/bin/python -c "
import json,pathlib
from orb_relay.proxy.lld_schemas import validate_module_brief
raw=json.loads(pathlib.Path('../../../fleet/contracts/fixtures/lld/complete_module.json').read_text())
b=validate_module_brief(raw); assert b is not None
pathlib.Path('/tmp/f05-roundtrip.json').write_text(json.dumps(b.model_dump(mode='json')))"
"$FLEET_BIN" gate lld-ready /tmp/f05-roundtrip.json; echo "exit=$?"
# EXPECT exit=0. If exit=8, re-dump with exclude_none=True and record it. DO NOT proceed on a guess:
# a silent shape rejection here makes every freeze attempt NOT_READY with every unit test green.

# Step 3 — terminal A: the scripted stub gateway (a REAL process)
#   Do NOT reuse tests/manual/stub_gateway.py: it returns one fixed non-JSON string (F03 hit this).
#   Write a disposable scripted-reply stub, as F03's builder and verifier both did.

# Step 4 — terminal B: the real relay, with the real FLEET_BIN in its env
ORB_GATEWAY_URL=http://127.0.0.1:<stub> FLEET_BIN="$FLEET_BIN" .venv/bin/uvicorn orb_relay.app:app --port 8099

# Step 5 — terminal C: drive it as a raw client. No RN, no Swift.
# 5a  warmup mode=build           -> opening == "Build mode. What are we making?"   (F04, unchanged)
# 5b  turn 1..N with mode=build   -> watch build.protocol.residual_ambiguity FALL turn over turn.
#     A static number here is the whole feature not working. Print it every turn.
# 5c  the turn that freezes       -> freeze != null, freeze.proposed_by == "orb:dialogue",
#                                    and `grep -c stamped_by` over the body == 0
# 5d  THE NEGATIVE DRIVE          -> restart with a stub that always clarifies; run 14 turns;
#     assert freeze stays null and protocol.move flips ask -> split at turn 13. Watch it, don't infer.
# 5e  kill the fleet binary (chmod -x) mid-session -> next freeze attempt returns
#     readiness.outcome == "unavailable" and freeze == null. It must REFUSE, not assume.
# 5f  mode=focus on the SAME server -> build/proposal/freeze all null, reply unchanged

# Step 6 — the gate
cd ../../.. && bash fleet/verify.sh    # foreground; read the REAL exit, not tee's
```

**Step 5e is the one to actually perform.** It is the difference between a wired gate and a decorative
one, and `UNAVAILABLE → refuse` is M8's property observed in a live process rather than a unit test.

---

## 7. Done-definition

A tick means it was **observed**, not intended.

1. `git merge master` done; `fleet/keel/fleet/src/lld_ready.rs` present in the working tree.
2. All of §5 written **before** implementation, and **proved red at the lane base** — in a disposable
   `git worktree add --detach <tmp> <base-ref>`, never `git stash` (§8.3 L3). Record which tests were
   red and which were legitimately green-at-base; **do not miscount a green-at-base test as a new pass**
   (F03's builder disclosed this correctly — match that standard).
3. `.venv/bin/pytest -q` green, with **zero regressions** against Step 1's recorded baseline. Name the
   number in both directions (`<baseline> → <baseline + new>`).
4. All eleven §5 T9 mutations applied by hand, **real failure text captured**, reverted, re-confirmed
   green. A mutation that fails to break its named victim is a **finding**, not a footnote.
5. §6 driven for real, including **5d and 5e**, with the actual response bodies pasted into
   `docs/evidence/F05-freeze-protocol.md`.
6. T8's untouched-file check prints nothing, run against the **committed** hash.
7. `bash fleet/verify.sh` run in the foreground; its **real** exit recorded, including any pre-existing
   failures, which must be traced to a file and line and shown to be **disjoint from F05's files**
   (F06's verifier set this standard: name `f08_pr_emit.rs:22`, not "some flaky test").
8. `docs/evidence/F05-freeze-protocol.md` written: red-on-arrival proof, mutation table with real
   output, the E2E transcript, and an explicit list of what is **not** proved.
9. `status/F05-Freeze-protocol--propose--pushback--free.status` → `STATUS=verifying`, `SINCE=` updated;
   `bash status/render.sh` re-run. **Note the real filename** — it ends `--free.status`, not
   `--freeze-.status`.
10. A dated `FLEET-LEARNINGS.md` entry appended at the **repo-root absolute path**
    `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md` — **never**
    `Light/FLEET-LEARNINGS.md`.
11. **Committed to `lane/F05-freeze-protocol`, NOT merged.** This lane edits `proxy/schemas.py`, a wire
    contract → **human-merge only (A15/D4)**. Ship to a PR and stop.

---

## 8. Assumptions, out-of-scope, and landmines already paid for

### 8.1 Assumptions — falsify before relying on them

| # | Assumption | How to falsify cheaply |
|---|---|---|
| A1 | `ε_gain = 0.05`, `θ_cov = 0.80`, `θ_amb = 0.15`, `N = 3`, split = 12 are the blueprint's `[A]` values and are **untuned against real dialogue**. The pristine-state EIGs (`0.04`–`0.135`) sit close enough to `0.05` that a weight change flips behaviour. | §5's tables make every threshold's effect visible; blueprint §6.4's trigger-rate-vs-defect-rate pairing is the real governor and is **not** available until F10 produces built modules. Do not claim these are tuned. |
| A2 | `w_s` (§4.1b) is a reasoned estimate of lane cost, not a measurement. | Re-derive once F10 has ≥ 10 real lanes: measure which slot's error actually caused rework. |
| A3 | `ρ_CHOOSE = 0.9` is F05's own `[A]` — the blueprint gives only `0.8` (self-report) and `→1` (binary). | It only affects question *ordering*, never a freeze verdict; a wrong value costs one mis-ordered question. |
| A4 | F04's keyword slot-extractor is the only thing moving `Coverage`, so convergence speed in T7.1 reflects **that stand-in**, not real NLU. | Stated in the evidence file. Do not report "converges in ≤ 6 turns" as a product claim. |
| A5 | keel's exit codes are stable (`0/3/6/7/8`, read from `main.rs`). | T5.1–T5.3 run the real binary; a code change breaks them loudly. |

### 8.2 Explicitly OUT of scope — do not build these

- **F09, the fleet handoff.** F05 produces a `FreezeProposal` and stops. It does **not** call
  `fleet sow`, does not touch `fleet/`, and does not know F07's intake exists. If you find yourself
  editing anything under `fleet/`, you have left this lane.
- **Emitting a stamped `Freeze` / an `lld.v1`.** §2.1. The missing keel producer is flagged in §9.
- **Persisting the freeze ledger.** F12's row (§2.4).
- **The depth rubric itself** — F06 owns it, in Rust and TS. F05 adds no rule, no check id, no threshold
  of keel's (T5.7 asserts it).
- **Real slot extraction / real NLU.** F04's keyword stand-in is reused unchanged and its honest cap
  carried forward verbatim.
- **A real contradiction detector.** §4.5 ships the one keyword-pair rule needed to make the hard
  conjunct reachable, and says so.
- **Register adaptation (blueprint §7), the full 9-state FSM, `blake3` freeze-hash parity with 03 §4.1**
  (F02 shipped `sha256:`; that divergence is F02's, already merged, and not F05's to re-litigate).
- **Voice/latency work.** Nothing here is on the audio path.

### 8.3 Landmines already paid for — do not rediscover

- **L1 — this worktree is 4 commits behind `master` and F06's gate is absent.** `git merge master`
  before anything. Writing `readiness.py` against a tree with no `lld_ready.rs` will produce a mock and
  call it done.
- **L2 — no `.venv` in this worktree.** Both F06's and F03's verifiers lost time here. The documented
  steps are `orb/scripts/setup.sh:145-149`; do not run the full interactive `setup.sh` (it prompts for
  API keys this lane does not need).
- **L3 — `git stash` is a shared stack across every worktree of one repo.** Three separate incidents.
  Use `git worktree add --detach <tmp> <ref>` for baseline isolation, and `git checkout --` against a
  checkpoint commit for mutation reverts.
- **L4 — `canonical_json` / `content_hash` raise `TypeError` on ANY numeric leaf, at any depth**
  (`lld_schemas.py`, F02 §6.3; `numeric_trap.json` is the fixture). `FreezeProposal` carries floats.
  **Hash the `ModuleBrief` only.** `ProposalTurn` is deliberately number-free so T6.4's guard can
  canonicalise it.
- **L5 — `ScriptedGateway.calls` increments *before* its reply queue is checked.** Every BUILD turn
  already makes ≥ 2 gateway calls (conversational + F03's mandatory decompose). A test that scripts too
  few replies fails with `IndexError: pop from empty list` in a place that looks unrelated. This cost
  F03 a cross-lane conflict and the verifier a full re-derivation.
- **L6 — carry-over is scoped to `(tenant_id, user_id)`, not `session_id`.** A test file driving several
  scenarios through one process must derive `user_id` from `session_id` (`f"u-{session_id}"`), or
  `conversation_guard.py`'s verbatim-repeat control fires, spends an extra gateway call, and produces a
  confusing failure unrelated to the property under test.
- **L7 — do not touch `conversation_guard.py`.** It is a safety surface with its own distress corpus and
  a red-team history. F04's A7 already established that BUILD mode correctly falls through to the
  unmoded path — the `shifts_mental_load` veto must **not** fire on build turns, because handing exactly
  one decision back to the human is what `Ask` exists for. If a §5 test cannot pass without editing it,
  **stop and escalate**.
- **L8 — capture `$?` directly, never after a pipe.** F06's verifier read `tee`'s status and briefly
  recorded a wrong exit code, self-caught. `verify.sh:20` warns about this in the file itself.
- **L9 — the Bash tool wraps `grep` with `ugrep`.** A script sees `/usr/bin/grep` and different regex
  semantics. Probe the way the script will actually run.

---

## 9. Routing (A17), sizing, and what I am flagging forward

**Routing: `mid-engineer` (Sonnet).** This is integration + a non-obvious termination argument across
four existing modules and a cross-language process seam — logic and integration work, not mechanical
scaffolding. Not `junior-engineer`: §3.1's stall/escape and §4.2's fail-closed exit-code mapping are
exactly the places a fully-specified-but-unreasoned build produces a plausible wrong answer.
**Verification: `verifier` (Sonnet, never Haiku), independently, in its own worktree**, per §7.

**Sizing:** one focused session. Two pure modules (~250 lines together), one additive schema block, one
additive `app.py` block (~40 lines), three test files. The long pole is §6's manual drive and §5's
eleven mutations, not the code.

**Flagged forward — not fixed here, do not silently absorb later:**

1. **No producer of a stamped `Freeze` exists anywhere in the tree.** `Freeze` (F02) and
   `STAMPED_BY = "keel:lld-ready"` (F06) both exist; nothing writes one. `fleet gate lld-ready` prints a
   verdict and exits. **F07/F09 must add a keel path that emits `freeze.v1` JSON** (a `--json` flag on
   the gate is the cheap form), or the S1 hand-off has no payload. This is the single biggest gap
   between here and F10's capstone.
2. **`FreezeStore` is live, tested, and has zero callers.** Not F05's to wire (§2.4). F12 should either
   adopt it or delete it; a tested store nothing calls is the shape of a fake.
3. **The blueprint's `ε_gain`/`θ_cov`/`θ_amb` are `[A]`, and §3.2's pristine EIGs land within `0.01` of
   `ε_gain` for two slots.** Blueprint §6.4's own governor (trigger-rate paired with post-build
   defect-rate) cannot run until F10. Until then these thresholds are *assumed*, and any claim that the
   loop is "tuned" is unearned.
4. **`is_contradiction_blocking`'s real detector is still missing.** §4.5 makes the conjunct reachable
   with one keyword-pair rule. Blueprint §4.3's Tier-2 schema-locked semantic check is a later lane's,
   and until it lands the orb detects only the one structural pair.
5. **The 14 checks exist twice (Rust + TS) and are cross-tested; the orb-side Python has zero copies by
   design.** If a later lane wants an in-process Python pre-check, it must land **with** a
   `lld-ready-crosslang.sh` arm covering it — otherwise it is a third drifting copy (§2.2).

---

*Contract written by the lead before any implementation existed. §5's expected values are the oracle:
they were derived from §4.1's formulas and `belief.py`'s published update law, not read off a run. A
builder who finds an implementation that disagrees with them has found a bug in the implementation, or
a bug in this contract — in either case, **stop and escalate**, do not adjust the test.*
