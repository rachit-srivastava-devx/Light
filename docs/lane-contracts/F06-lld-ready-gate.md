# F06 — the `lld-ready` gate (lane contract)

> **Lead-authored, contract only.** No implementation ships with this document. The acceptance
> suite in §7 is written **before** the gate exists and is **expected to be red** until a builder
> makes it green (T1/T2). **The builder may not edit `fleet/keel/fleet/tests/f06_lld_ready.rs` or
> `fleet/tests/acceptance/lld-ready-*.sh` beyond *adding* cases.** Any diff that weakens, deletes,
> or re-states an assertion in those files is a contract breach — flag it, do not merge it.
>
> Blueprint: [`blueprints/Speed-of-Thought-L8-Deep-Dive/03-SPEC-COMPLETE-GATE-AND-DESIGN-GRAPH.md`](../../../blueprints/Speed-of-Thought-L8-Deep-Dive/03-SPEC-COMPLETE-GATE-AND-DESIGN-GRAPH.md) §2 (§2.1 signature, §2.2 depth conjuncts, §2.3 `fake_cost`, §2.4 fixtures, §2.5 killed alternatives, §2.6 the advisory judge) + that file's Operator's-scars.
> Upstream contract: [`F02-lld-v1-contract.md`](F02-lld-v1-contract.md) — **merged and independently verified** (commit `6fd66f3`, F02 status `verified`).
> Depends on: F02 only. Blocks: F07 (SOW intake), F09 (seam), F17 (rubric tightening), F27, F33.

---

## 1. Restatement — what is actually being asked, in this tree's own terms

FEATURES.md F06 asks for *"a deterministic (0 LLM), Rust `lld-ready` gate that refuses a freeze with
any open question, no owner, no acceptance line, or a `depth_score` below a threshold."*

In this tree, after F02 merged, that decomposes into something narrower and more specific than the
blueprint's sketch, because **three of the four things `03` §2.1 assumes already exist do not, and
one thing it does not mention already does**:

| `03` §2.1 assumes | This tree, today | Consequence for F06 |
|---|---|---|
| a `Gate` trait with `GateId` / `Evidence` / `Verdict` / `Outcome::MeasuredNothing` | **absent.** Swept `fleet/keel/fleet/src/`: the only `Verdict` is `agent.rs:383` `enum Verdict { Credit, Fault }` (agent scorecards) and the only outcome enums are `agent.rs:221 ScorecardOutcome` and `swarm.rs:93 EvidenceOutcome`. No `trait Gate`, no `GateId`, no `bad_fixture`/`control_fixture`/`fake_cost`, no gate `selftest` harness anywhere | §3(a): the gate ships as a **pure free function with the same invariants**, not a trait impl. Building the kernel is a different feature. |
| `OwnerRegistry::resolves()` and `metric_registry::resolves()` | **absent as code.** `fleet/contracts/owners.v1.json` exists (`{"owners": ["rachit@devxlabs.ai"]}`); `agent.rs:65-125`'s `Registry` is an `agents.toml` map, a different domain; there is no metric registry at all | refs are **caller-supplied data** (§5.3), and the metric-registry conjunct degrades to the denylist the TS gate already ships (§4, `R21-ALTS`) |
| `GraphIndex::impact()` for the blast-radius conjunct | **absent as a pure fn.** `graph.rs:292-329` `impact_command()` is a CLI entry that opens SQLite and queries dependents. No public `impact()`. And `Light/registry/{features,services,modules}` are **all empty directories** | §3(b): the blast-radius conjunct is **killed for F06** and disclosed, not silently dropped |
| — (not mentioned) | **`orb/apps/mobile/src/build/LldReadyGate.ts` already implements 14 deterministic checks**, F02-adapted, 18/18 green under the F02 verifier's independent re-run | §2: this is an **extract (C2)**, not a build-new. The Rust gate is the second mirror of an existing predicate set, kept honest by a comparator — exactly F02's shape. |

So, stated in this tree's terms:

> **F06 ships `fleet::lld_ready::evaluate(&Value, &GateRefs) -> Verdict`** — a total, pure Rust
> function, 0 I/O, 0 LLM, 0 clock, 0 randomness — that runs the **same 14 check ids** the merged TS
> gate runs, **reuses `lld.rs`'s already-comparator-verified field predicates instead of
> re-deriving them**, emits a `depth_evidence` block that validates against the merged
> `freeze.v1.json`, is the **only** writer of `stamped_by: "keel:lld-ready"`, and makes
> `checked == 0 ⇒ MeasuredNothing` a **reachable, tested refusal** rather than the dead branch it
> is in TS today (§4.2 — this is the single most important thing in this lane).

### 1.1 One named divergence from FEATURES.md, decided at the lead level

FEATURES.md F06 says the gate *"refuses … a `depth_score` below a threshold calibrated against a
labeled corpus."* **The shipped design has no threshold, and must not acquire one.**

The reason is in the merged code, not in taste. `LldReadyGate.ts:5-7` states it and `U3-T4` enforces
it: a brief scoring **13/14** — every structural check but one — is `NOT_READY`. Any threshold
`t < 1.0` is strictly weaker than "every check passes", and admits a brief with a real failed check;
`t = 1.0` is just the conjunction with extra arithmetic in front of it. Worse, a ratio threshold
restores the trade the conjunction forbids: a proposer could drop a real acceptance predicate
(`C3-ACC-GROUND`) and buy the ratio back with two padded alternatives. The cheapest green would stop
being the correct fix — which is the one property `03` §2.3 exists to preserve.

**The conjunction is the bar. `depth_evidence.score` is a report, never a term in the decision.**
The "calibrated against a labeled corpus" half of FEATURES.md's sentence is real work, but it is
**F17**'s (rubric tightening), and `03` §8 already marks it `[TBM]` — *"the tuning signal is
trigger-rate vs post-build defect-rate on a labeled set … not feel"*, and no such labeled set is
scored in this tree yet. F06 must not invent one to satisfy a sentence.

FEATURES.md's F06 row is updated as part of this lane's done-definition (§9 item 10) — it also
currently carries a **column-shift corruption** (F02's status text sits in F06's Branch/Acceptance
cells) and is wrongly marked **☑ done** for a gate that does not exist in Rust. Both are fixed here.

---

## 2. C1 / L2 registry verdict, said out loud

> ### **EXTRACT (C2) + a bounded build-new.**
> **Extract:** the 14-check predicate set and the `DepthScore`/`MEASURED_NOTHING` verdict shape from
> `orb/apps/mobile/src/build/LldReadyGate.ts` (2nd consumer: the fleet keel runtime).
> **Reuse in place, do not copy:** three field predicates already living in `fleet/keel/fleet/src/lld.rs`.
> **Build-new, and only this:** the reachable-`MeasuredNothing` seam, the checked-in `GateRefs`
> source, the freeze-stamping path, the CLI dispatch, and the TS↔Rust comparator.

### 2.1 What already exists — reuse it, do not rewrite it

| Asset | Location | How F06 uses it |
|---|---|---|
| The 14 check ids + their semantics | `orb/apps/mobile/src/build/LldReadyGate.ts:19-34`, `:80-216` | **ported id-for-id.** `GATE_CHECK_IDS` is a shared vocabulary; the comparator (§7, F06-T10) is a set-equality over it, so a renamed or added id breaks the build loudly |
| `DepthScore` / `GateVerdict` shape | `LldReadyGate.ts:49-60` | mirrored in Rust. It is already the exact shape `freeze.v1.json`'s `depth_evidence` requires (`{checks_total, checks_passed, ratio, failed_check_ids}` + `checked_check_ids`) — F02 §5.2 built it that way on purpose (*"F06 fills it"*) |
| `valid_node_id` — `^[a-z0-9][a-z0-9-]{2,63}$` | `lld.rs:64-80` (**private today**) | **make `pub(crate)` and call it** from the `SHAPE` check. This function carries a paid-for bug fix: an early draft accepted `"x"` because `{2,63}` was read as `len >= 1`; the F02 comparator caught it (`lld.rs:66-71` documents it). Re-deriving this in `lld_ready.rs` re-opens that exact bug |
| `number_calc_ok` — a digit **and** an operator from `+-*/÷×=≈%` | `lld.rs:100-104` (private) | **make `pub(crate)` and call it** from `R17-DERIV`. Mirrors TS `CALC_ARITH` at `LldReadyGate.ts:74,114` |
| `structural_enforced_by_ok` — `file.{ts,tsx,py,rs,json}[:line]` \| `type:` \| `gate:` | `lld.rs:106-127` (private) | **make `pub(crate)` and call it** from `R17-DERIV`. Mirrors TS `ENFORCED_BY_PATTERN` at `LldReadyGate.ts:72,119-121` |
| Shape validation of the brief | `lld::validate_module_brief` (`lld.rs:182`), wired at `main.rs:3448` as `fleet contract lld validate` | the **contract layer**, run by the CLI *before* the gate (§5.4). The gate does not redefine or re-run it |
| `stamped_by` refusal | `lld.rs:653-666` — `check_freeze` accepts **only** the literal `"keel:lld-ready"` | already enforces "only the gate may stamp". F06 is the code that earns the right to write it (§5.5) |
| The fixture corpus | `fleet/contracts/fixtures/lld/` (6 fixtures + `AUTHORSHIP.md`) | **read-only to this lane.** See §6 — this is a hard rule, not a preference |
| Test conventions | `fleet/keel/fleet/tests/f02_lld_crosslang.rs` — `f{feature}_t{n}_{description}`, fixtures via `concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/fixtures/lld")` | followed exactly |
| Comparator convention | `fleet/tests/acceptance/lld-crosslang.sh` — statuses captured directly, never `$?` after a pipe (E1/S11), exit **6** on divergence, *"measuring nothing is a failure"* | followed exactly |

**Three of the six regexes the TS gate uses already exist, hand-ported and comparator-verified, in
`lld.rs`.** That is the whole C1 argument in one line: this workspace has **no `regex` crate**
(`lld.rs:59-63` says so and explains why), so every predicate is hand-written, and hand-written
predicates are precisely where F02's `{2,63}` bug lived. Reuse is not tidiness here; it is the
difference between inheriting a fixed bug and re-introducing it.

### 2.2 What genuinely does not exist — build-new, and only this

1. `fleet/keel/fleet/src/lld_ready.rs` — the gate.
2. The **reachable** `MeasuredNothing` seam (§4.2). The TS gate's `checked === 0` branch
   (`LldReadyGate.ts:220-222`) is unreachable dead code and its test does not drive it. This is
   net-new correctness, not a port.
3. `fleet/contracts/gate-refs.v1.json` — a checked-in source for `registry_paths` /
   `known_node_ids`, which today exist **only as literals inside a test file**
   (`LldReadyGate.test.ts:9-11`).
4. The freeze-stamping path: turning a `Ready` verdict into a `depth_evidence` block +
   `stamped_by`, validated by `lld::validate_lld_v1`.
5. `fleet gate lld-ready` CLI dispatch + `--selftest`. (**No collision**: `grep '"gate"' main.rs`
   returns zero hits today; `gate` is a free top-level verb. Dispatch follows
   `contract_lld_validate`'s structure at `main.rs:3448`/`:3465` — including the usage line at
   `:3454`, which is what keeps `graph.rs`'s reachability test green. See §11.1 item 6.)
6. `fleet/tests/acceptance/lld-ready-crosslang.sh` — the TS↔Rust comparator.

### 2.3 The precedent this lane must match

F02 is the template and it is two weeks fresh: one contract, three hand-written mirrors, one shared
fixture corpus, one comparator that exits 6 on divergence, and **the comparator is the thing that
found the real bugs** — not the unit tests. F06 is the same shape with two mirrors instead of three.
A Rust gate with green Rust tests and no comparator would be a *different gate* that happens to
share a name, and the failure mode is concrete: the orb would freeze a spec that fleet then refuses,
opaquely, at the seam F09 builds.

---

## 3. Killed alternatives

Each is grounded in code in this tree, with the file:line that kills it.

### 3.1 KILLED — implement `03` §2.1 literally: `impl Gate for LldReady` with `bad_fixture()` / `control_fixture()` / `fake_cost()`

- **Why killed.** The trait does not exist. A full sweep of `fleet/keel/fleet/src/` finds no
  `trait Gate`, no `GateId`, no `Evidence`, no `Outcome::MeasuredNothing`, no `bad_fixture`, no
  `control_fixture`, no `fake_cost`, and no gate-`selftest` harness — the nearest neighbours are
  `agent.rs:383 enum Verdict { Credit, Fault }` and `swarm.rs:93 EvidenceOutcome`, both agent-
  scorecard types with no relation to spec readiness. Implementing the trait means **first building
  the keel gate kernel** (`05-GATE-KERNEL-AND-UNFAKEABILITY` §1) — a substantially larger feature
  with its own registry decision, its own startup-disable banner behaviour, and its own contract.
  Folding it into F06 makes this lane roughly 3× its brief and couples the *first* gate to the
  kernel's design before there is a second gate to generalise from.
- **What ships instead.** The free function carries every invariant the trait exists to enforce, and
  each one is a named test rather than a trait obligation: fixtures are pinned in
  `fleet/tests/acceptance/lld-ready-selftest.sh` (F06-T1/T2 + the shell selftest), `fake_cost` ships
  as the §8 ADR paragraph, and `MeasuredNothing` is structural (§4.2).
- **Revisit trigger.** The **second** keel gate lands (F20 `stream-live-gate`, F25 registry gate, or
  F33's trigger gate — whichever is first). Two gates is when the trait stops being speculative
  generality and starts being deduplication; the abstraction is then extracted *from* two working
  implementations rather than guessed ahead of one.
- **Failure story.** `route.rs`'s 6-stage router in this same crate was built ahead of its consumer
  and, per the fleet rules skill, is **still not wired into `run_with_evidence`/`swarm dispatch`** —
  a designed-ahead abstraction that has never run. Building a gate kernel to host exactly one gate
  is the same bet, and this tree has already lost it once.

### 3.2 KILLED — implement `03` §2.2's blast-radius conjunct (`g.impact(&n.interface) != d.blast_radius`)

- **Why killed.** Three independent blockers, any one sufficient. **(i) It is not pure.**
  `graph.rs:292-329`'s `impact_command()` opens a SQLite database and queries dependents; there is
  no public pure `impact()`. A gate that opens a database has I/O on the control path, and `03`
  §2.1's whole invariant is *"no io, no async, no network."* **(ii) There is nothing to resolve
  against.** `Light/registry/features/`, `registry/services/` and `registry/modules/` are all
  **empty directories** — the code graph has no nodes for a module brief's interface to impact.
  **(iii) The field does not mean that.** In the merged schema, `blast_radius` is a **prose string**
  in two places (`module-brief.v1.json`'s `failure_story.blast_radius`, min 10 chars —
  `lld.rs:405-410`; and `lld.v1.json`'s `sow_seed.blast_radius` — `lld.rs:689-694`). It is not a
  symbol set the gate could recompute and compare. Implementing the blueprint's conjunct would mean
  re-shaping a merged, verified, human-merged contract — which is A15/D4 work, not this lane's.
- **What ships instead.** `R21-FAIL` (the ported check) enforces that `failure_story.blast_radius`
  is present and ≥10 non-blank characters. That is a *presence* check, not a *derivation* check.
  **Say so honestly**: this is the one conjunct where the proposer's own number would be trusted, so
  F06 does not pretend to check it. `03` §8 already names this — *"the blast-radius conjunct is only
  as good as the code graph … an un-indexed greenfield repo cannot pass C4 until the first index is
  built."* This tree is that greenfield repo.
- **Revisit trigger.** `graph.rs` exposes a pure `impact(&self, symbol) -> BTreeSet<SymbolId>` over
  an in-memory index **and** `registry/` contains ≥1 real module. Both, not either.
- **Failure story.** The estate's standing scar is *"a proxy is not the property."* Shipping a
  `blast_radius` conjunct that compares two prose strings, or that silently passes because the index
  is empty, would be a gate reporting a green it never measured — the same class as `fleet-rs` `C4`
  (a gate that reported `lessons[0]` and PASSED) that `03` §2.1 cites as its own founding scar.

### 3.3 KILLED — re-derive the 14 checks in Rust from the blueprint, independently of `LldReadyGate.ts`

- **Why killed.** It sounds more rigorous and is strictly worse. Two independently-derived predicate
  sets over one contract is exactly the state F02 §4.5 diagnosed as *"live drift, no binding test"*
  and spent a whole lane fixing. The concrete failure: the orb reaches `❄ freeze` on a brief its own
  gate accepted, hands it across the F09 seam, and fleet's gate refuses it — with a different
  vocabulary of reasons, so the operator cannot even tell which gate is right. And the divergence
  would be invisible: both test suites would be green.
- **What ships instead.** A port, plus the comparator that makes the port *checkable* — the same
  mechanism, over the same corpus, as `lld-crosslang.sh`.
- **Revisit trigger.** The TS gate is retired (F06's successor re-homes readiness entirely
  fleet-side and the orb calls `fleet gate lld-ready` over the seam). At that point there is one
  mirror and the comparator becomes dead weight. **Not today** — `LldReadyGate.ts` is a live,
  merged, 18/18-green module F02 explicitly kept compiling (F02 §10.1 item 4).
- **Failure story.** F02's own: the Rust `valid_node_id` accepted `node_id: "x"` while TS and Python
  (real regex engines) refused it. **Seven Rust unit tests were green.** The comparator caught it,
  because the comparator was the only thing comparing. That bug is documented in
  `lld.rs:66-71` and it is one hand-written predicate away from happening again in `lld_ready.rs`.

### 3.4 KILLED — a `depth_score` ratio threshold as the pass criterion (FEATURES.md's literal wording)

- **Why killed.** See §1.1. In code: `LldReadyGate.ts:5-7` and `U3-T4` already forbid it, and
  `freeze.v1.json`'s `depth_evidence.score` is annotated as report data — `lld.rs:598-627` validates
  it as *shape*, never as a decision input. Any `t < 1.0` admits a brief with a genuinely failed
  check; `t = 1.0` is the conjunction wearing arithmetic. And a ratio makes the checks **fungible**:
  fourteen checks of visibly unequal weight (a real acceptance predicate vs. a `purpose` length
  bound) become interchangeable at 1/14 each, so the cheapest green stops being the correct fix.
- **What ships instead.** The conjunction. `ratio` is computed and reported into
  `depth_evidence.score.ratio` for the human and for F17's calibration corpus, and F06-T4 asserts
  a 13/14 brief is `NotReady`.
- **Revisit trigger.** F17, **and** only with a scored labeled corpus showing trigger-rate against
  post-build defect-rate (`03` §8's `[TBM]`). A number, not a feel.
- **Failure story.** `03` §Operator's-scars: *"a generic depth rubric refused a correct one-file
  utility for 'too few killed alternatives' … it refused correct work and taught the operator to
  resent the gate."* A mis-tuned threshold fails in both directions and only measurement says which
  — and this tree's standing scar (`quality-gate-precision-first`) is that a new gate's first run is
  mostly false positives, so an un-calibrated threshold would be muted within a day.

### 3.5 KILLED — make the gate load `owners.v1.json` / `gate-refs.v1.json` itself

- **Why killed.** `03` §2.1's invariant is `0` I/O — *"it cannot call a model and cannot read the
  tree it is judging."* A gate that reads its own references is a gate whose verdict depends on the
  working directory, and its determinism claim becomes untestable (the same node yields different
  verdicts on different checkouts). It also makes F06-T5's purity assertion unwritable.
- **What ships instead.** `GateRefs` is a plain data struct passed in by the caller, exactly as
  `LldReadyGate.ts:38-42` does. The **CLI** (`main.rs`) does the file reading, in the layer where I/O
  belongs, matching `contract_lld_validate`'s existing structure at `main.rs:3465`.
- **Revisit trigger.** None foreseen. If a caller ever needs defaults, they belong in a
  `lld_ready::refs_from_path()` helper *outside* `evaluate`, and F06-T5 must still pass.
- **Failure story.** The estate's `gate-must-assert-rendered-state` / `check-for-live-writers`
  scars are both "the check read state that something else was concurrently changing." A gate that
  re-reads `registry/` mid-lane while a sibling worktree writes it is that failure with a spec
  attached.

---

## 4. The two things this lane exists to get right

Everything else is a port. These two are not.

### 4.1 The check vocabulary is fixed, and the denominator is the vocabulary

`GATE_CHECK_IDS` has exactly **14** entries and they are byte-identical to
`LldReadyGate.ts:19-34`:

```
C1-OPEN  C2-OWNER  C3-ACC-PARSE  C3-ACC-GROUND  C3-ACC-NONTAUT
R17-DERIV  R19-ABSOLUTE  R21-ALTS  R21-FAIL
C12-STORE  C12-DEPS  REG-VERDICT  IFACE  SHAPE
```

`checks_total` is `GATE_CHECK_IDS.len()`, **derived, never a literal** — the estate's
`gate-census-not-sample` scar is that a hard-coded denominator drifts silently below the real
coverage. F06-T3 is a **census**: it asserts every one of the 14 can be failed in isolation, and its
loop bound is `GATE_CHECK_IDS.len()`, so adding a 15th id without a matching isolation case fails
the test rather than quietly reducing coverage.

Each check is **total over `serde_json::Value`**: a missing, null, or mistyped field makes that check
*fail*, never panic. This mirrors `LldReadyGate.ts:227-231`'s `try { … } catch { passed = false }`
and is what lets the selftest run `evaluate` directly against the malformed bad fixture (§5.4).

### 4.2 `checked == 0 ⇒ MeasuredNothing` must be **reachable**, and it currently is not

This is the lane's headline finding and its highest-value line of code.

`03`'s first Operator's scar is *"the gate that passed while measuring nothing … `checked` came out
`0` and the `reasons` vector was empty, so it read `Ready`. It certified an empty node."* The fix is
supposed to be structural.

**In the shipped TS gate it is not.** `LldReadyGate.ts:219-222`:

```ts
const checked = CHECKS.length;          // CHECKS is a module-level const of 14 entries
if (checked === 0) {                    // ← unreachable. Always 14.
  return { outcome: 'MEASURED_NOTHING', checked: 0 };
}
```

and the test that claims to cover it, `LldReadyGate.test.ts:69-71`, is named
*"U3-T6 an empty check set is MEASURED_NOTHING, never READY"* but passes `{owners: [], registry_paths: [], known_node_ids: []}`
and asserts only `expect(v.outcome).not.toBe('READY')`. Its own inline comment concedes the
mechanism: *"owners is empty ⇒ C2 cannot pass."* That verdict is `NOT_READY` with 3 failed checks.
**The `MEASURED_NOTHING` path is never executed by any test in this tree.** A branch that cannot run
is not a safeguard; it is a comment that compiles — and it is the exact shape of the scar it was
written to prevent (`a check cheaper to fake than to satisfy will be faked`).

**F06 closes it with a seam, not a comment:**

```rust
/// The public entry point. Always measures the full vocabulary.
pub fn evaluate(brief: &Value, refs: &GateRefs) -> Verdict {
    evaluate_with(brief, refs, CHECKS)
}

/// The same evaluator over an explicit check slice. `pub` **so that an empty slice is reachable
/// from a test** — the `MeasuredNothing` refusal is the one branch that must be proven to execute,
/// and a branch no test can reach is indistinguishable from a branch that does not exist
/// (03 Operator's-scar #1; LldReadyGate.ts:220 is that branch today).
/// Passing an empty slice is safe by construction: it yields a refusal, never a pass.
pub fn evaluate_with(brief: &Value, refs: &GateRefs, checks: &[Check]) -> Verdict { … }
```

F06-T6 drives `evaluate_with(control_fixture, full_refs, &[])` — a brief that is otherwise
**`Ready` 14/14** — and asserts the outcome is `MeasuredNothing`, that `checked == 0`, and that
`score` is `None`. A green with a zero denominator is the worst kind of green, so the type must not
be able to carry one: `Verdict::score` is `Option<DepthScore>` and is `None` **only** for
`MeasuredNothing`, asserted in the same test.

**Mutation proof is mandatory** (§7.6): deleting the `checked == 0` guard must turn F06-T6 red.
Without that, the test is asserting a tautology.

---

## 5. The interface — exactly what ships

### 5.1 `fleet/keel/fleet/src/lld_ready.rs`

Style follows `lld.rs`: `&serde_json::Value` in, a structured result out, **all** violations
collected (never first-error-only — the comparator diffs failed-id *sets*), no `regex` crate,
`pub(crate)` reuse of `lld.rs`'s predicates.

```rust
//! F06: the `lld-ready` gate. Deterministic, pure, 0 LLM, 0 I/O (03 §2.1).
//! Second mirror of orb/apps/mobile/src/build/LldReadyGate.ts; kept honest by
//! fleet/tests/acceptance/lld-ready-crosslang.sh, not by these tests alone.

pub const GATE_CHECK_IDS: [&str; 14] = [ /* §4.1, in TS order */ ];

/// Caller-supplied reference sets. NOT read from disk here (§3.5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateRefs {
    pub owners: Vec<String>,
    pub registry_paths: Vec<String>,
    pub known_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReason { pub check_id: &'static str, pub detail: String }

#[derive(Debug, Clone, PartialEq)]
pub struct DepthScore {
    pub checks_total: u32,
    pub checks_passed: u32,
    pub ratio: f64,                 // round(passed/total, 3) — matches TS's *1000/1000
    pub failed_check_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome { Ready, NotReady, MeasuredNothing }

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub outcome: Outcome,
    pub checked: usize,
    /// `None` **iff** `outcome == MeasuredNothing` (§4.2). A verdict cannot report a score
    /// without a denominator.
    pub score: Option<DepthScore>,
    pub reasons: Vec<GateReason>,
}

pub type Check = (&'static str, fn(&Value, &GateRefs) -> bool);
pub const CHECKS: &[Check; 14] = &[ /* … */ ];

pub fn evaluate(brief: &Value, refs: &GateRefs) -> Verdict;
pub fn evaluate_with(brief: &Value, refs: &GateRefs, checks: &[Check]) -> Verdict;

/// The `depth_evidence` block for freeze.v1 (F02 §5.2: "F06 fills it").
/// `checked_check_ids` is the full measured vocabulary; `failed_check_ids ⊆ checked_check_ids`.
pub fn depth_evidence(v: &Verdict) -> Option<Value>;

/// The gate's identity. THE ONLY place in the fleet tree that constructs this literal for
/// writing (lld.rs:658 only *compares* it). F06-T9 greps for that invariant.
pub const STAMPED_BY: &str = "keel:lld-ready";
```

### 5.2 The 14 checks — port table, with the reuse target for each

| id | TS source | Rust implementation | Reuse |
|---|---|---|---|
| `C1-OPEN` | `:80-82` | `open_questions` array is empty | — |
| `C2-OWNER` | `:84-86` | `refs.owners.contains(owner)` | — |
| `C3-ACC-PARSE` | `:88-96` | given/when/then trim-len ≥3; `oracle_kind ∈ {test,property,metric}` | mirror `lld.rs:277-303` semantics; **do not call** `validate_module_brief` (§5.4) |
| `C3-ACC-GROUND` | `:98-101` | `artifact.starts_with(owner_path)` **and** `then.contains(artifact)` | — |
| `C3-ACC-NONTAUT` | `:103-107` | `then != given`; `then` not in the 5-pattern tautology denylist | hand-port, anchored + case-insensitive |
| `R17-DERIV` | `:109-124` | ≥1 guarantee; each `number` ⇒ `calc` ok, each `structural` ⇒ `enforced_by` ok | **`lld::number_calc_ok`**, **`lld::structural_enforced_by_ok`** |
| `R19-ABSOLUTE` | `:126-131` | a claim containing an absolute word must be `kills_structural` + `structural` | hand-port `\b(zero\|never\|impossible\|cannot\|no way)\b` — see §11.1 |
| `R21-ALTS` | `:133-151` | ≥2 (**flat floor, no `grain` exemption**); all 3 fields non-blank; `revive_trigger` not in the non-trigger denylist (`never` exempted); options case-insensitively distinct | — |
| `R21-FAIL` | `:153-156` | trigger / blast_radius / fail_safe each ≥10 trim-chars | — |
| `C12-STORE` | `:158-162` | every `owned_by_node == node_id`; stores distinct | — |
| `C12-DEPS` | `:164-169` | no self-dep; every dep ∈ `refs.known_node_ids`; no dep collides with an owned store | — |
| `REG-VERDICT` | `:171-178` | `install`/`extract` ⇒ `matched_path ∈ refs.registry_paths`; `build_new` ⇒ `searched` non-empty and all ∈ `refs.registry_paths` | — |
| `IFACE` | `:180-186` | ≥1 entry; each signature has `(` and `)`, or starts `type ` / `interface ` | — |
| `SHAPE` | `:188-196` | node_id pattern; `purpose` 1..=200 **chars** (not bytes); `owner_path` non-empty, no `..` | **`lld::valid_node_id`** |

`purpose` is counted in **`chars()`**, matching `lld.rs:214`'s `p.chars().count() <= 200` and JS's
UTF-16-ish `.length` closely enough for the ASCII corpus; **`.len()` on a `&str` is bytes and is
wrong.** Named because it is a one-character diff that the comparator would catch only if a fixture
carried a non-ASCII `purpose`, and none does.

`R21-ALTS` carries **no `grain: 'leaf'` exemption.** F02 closed that loophole in the schema, in all
three mirrors, *and* in `LldReadyGate.ts:134-138` (which documents the closure inline). Re-opening
it in Rust — including "for parity with the blueprint's Operator's-scar #5" — is a contract breach.
`03`'s scar #5 argues *for* a grain-scaled floor; **F02 §3.4 overruled it and the human merged that
call.** The precedence order is `merged contract > blueprint prose`; flag the tension, do not
re-litigate it in code.

### 5.3 `fleet/contracts/gate-refs.v1.json` — new, and why it is not `owners.v1.json`

Two of the three reference sets have **no checked-in source anywhere in this tree**. They exist only
as literals at `LldReadyGate.test.ts:9-11`:

```ts
registry_paths: ['registry/services/llm-gateway', 'registry/features/cost-control-plane'],
known_node_ids: ['orb-freeze-ledger'],
```

and neither resolves against reality: `Light/registry/{features,services,modules}` are **empty
directories**, and no node id is emitted by anything yet. A gate whose only reference data lives in
its own test file is a gate that cannot run in production, and the control fixture
(`complete_module.json`) needs *both* sets to reach `Ready` — its `deps: ["orb-freeze-ledger"]`
drives `C12-DEPS` and its `registry.searched` drives `REG-VERDICT`.

Ships as a sibling of `owners.v1.json`, same style, **with the limitation in the file itself**:

```json
{
  "schema_version": "1.0",
  "description": "Reference sets for the lld-ready gate (F06). HONEST LIMIT: registry_paths is a checked-in list, NOT a scan of Light/registry/ -- those three directories are empty as of F06. known_node_ids likewise anticipates the freeze ledger (03 §4.3) that F12 builds. Both degrade this gate's C12-DEPS and REG-VERDICT checks from 'resolves in a live registry' to 'appears in a maintained list': mitigates, not kills. Revisit when registry/ holds >=1 real module.",
  "registry_paths": ["registry/services/llm-gateway", "registry/features/cost-control-plane"],
  "known_node_ids": ["orb-freeze-ledger"]
}
```

`owners` stays in `owners.v1.json` — it is already referenced by `module-brief.v1.json`'s
`description` and by F02 §5.2's `owner` row; moving it would edit a merged contract for cosmetics.

### 5.4 Shape is the contract layer's job; readiness is the gate's

The gate does **not** call `lld::validate_module_brief`, and this is deliberate.

`fleet contract lld validate` (`main.rs:3448`, `contract_lld_validate` at `:3465`) already owns shape
validation and already has typed exit codes the F02 verifier adversarially tested (0/3/7/8 across
nonexistent / empty / malformed / empty-object / bare-array inputs). Duplicating it inside
`evaluate` would (a) put a second, weaker copy of a verified validator in the tree — the exact drift
F02 §4.5 exists to prevent, and (b) **break the comparator**: `one_alternative.json` fails
`lld::validate_module_brief` on path `alternatives` *and* fails `R21-ALTS`, so a Rust gate that
folded shape in would report `{R21-ALTS, SHAPE}` where TS reports `{R21-ALTS}` — a spurious
divergence caused by the gate, not by a real disagreement.

So the layering is:

| layer | owner | runs | on failure |
|---|---|---|---|
| shape | F02, `lld::validate_module_brief` | `fleet gate lld-ready <file>` runs it **first** | exits with the contract-validation code; `evaluate` is never reached |
| readiness | F06, `lld_ready::evaluate` | after shape passes | `Ready` / `NotReady` / `MeasuredNothing` |

**and `evaluate` remains total anyway**, so `--selftest` can call it *directly* on the malformed
`one_line_freeze.json` and prove the gate itself refuses (blueprint §2.4's requirement). The CLI
precondition is operator ergonomics; it is not what makes the gate safe.

### 5.5 The stamp

`Ready` ⇒ the caller may write `stamped_by: "keel:lld-ready"` and the `depth_evidence` block into a
`freeze`. `NotReady` / `MeasuredNothing` ⇒ **no stamp is produced at all** (`depth_evidence()`
returns `None` for `MeasuredNothing`; the CLI refuses to stamp a `NotReady`). `lld.rs:653-666`
already refuses every other value of the field, so this closes the loop F02 opened: the schema says
only the gate may stamp, and F06 is the gate.

`STAMPED_BY` appears **exactly once** in `lld_ready.rs`. F06-T9 greps the source for that, the same
way `f02_t14_no_source_path_constructs_the_stamper_literal` greps `lld.rs` — a fact a reviewer can
check in one command, not a convention a reader must trust.

### 5.6 Files owned by this lane

**New:**
```
fleet/keel/fleet/src/lld_ready.rs
fleet/keel/fleet/tests/f06_lld_ready.rs              ← contract suite; builder MUST NOT edit (§7)
fleet/contracts/gate-refs.v1.json
fleet/tests/acceptance/lld-ready-selftest.sh         ← contract suite; builder MUST NOT edit
fleet/tests/acceptance/lld-ready-crosslang.sh        ← contract suite; builder MUST NOT edit
```
**Edited (all one- or few-line):**
```
fleet/keel/fleet/src/lib.rs        +pub mod lld_ready;
fleet/keel/fleet/src/lld.rs        visibility ONLY: valid_node_id / number_calc_ok /
                                   structural_enforced_by_ok -> pub(crate). Zero logic change;
                                   lld-crosslang.sh must still exit 0 (§9 item 4).
fleet/keel/fleet/src/main.rs       +`fleet gate lld-ready <file>` and `--selftest` dispatch,
                                   in contract_lld_validate's style (:3448/:3465), + usage line
fleet/verify.sh                    +2 stage lines (selftest, crosslang)
fleet/contracts/fixtures/lld/AUTHORSHIP.md   append ONE row (§6). Append-only.
FEATURES.md                        F06 row: fix the column-shift corruption, un-tick ☑, record
                                   the §1.1 divergence and the C1 verdict
```
**Read-only to this lane — touching any of these is a breach:** every `*.json` under
`fleet/contracts/fixtures/lld/`, all three `fleet/contracts/{module-brief,freeze,lld}.v1.json`,
`fleet/contracts/owners.v1.json`, `fleet/tests/acceptance/lld-crosslang.sh`,
`fleet/keel/fleet/tests/f02_lld_crosslang.rs`, and every file under `orb/`.

**Overlap check.** F02 (merged) owned `lld.rs`, `f02_lld_crosslang.rs`, the schemas and fixtures —
F06 touches `lld.rs` for **visibility only** and nothing else on that list. F08 (merged) owned
`lifecycle.rs`, `main.rs`, `tests/f08_pr_emit.rs`, `tests/compile_fail/`. **`main.rs` and
`verify.sh` are shared with F08 and F02** — both are append-at-the-end edits; re-run the full
`verify.sh` after any rebase rather than trusting a clean merge (F02 §9.4 landmine).

---

## 6. Fixture independence — a hard rule, because it is currently earned

`AUTHORSHIP.md:12-13` records `complete_module.json` and `one_line_freeze.json` as
**"independent of the gate (F06 not yet built)"** — authored by U1, before any Rust gate existed,
by someone who is not this gate's author. That is the strongest independence position any fixture in
this tree has, and it is exactly what `03` §2.4's second designed-in scar demands
(*"the bad fixture is authored by someone other than the gate's author … a self-authored
`lld-ready` fixture is unverified, and unverified is reported, not assumed"*).

**The moment F06's builder edits either file, that claim becomes false.**

Therefore:

1. **No file under `fleet/contracts/fixtures/lld/` may be modified or added by this lane**, with the
   single exception of appending one row to `AUTHORSHIP.md`.
2. Per-check isolation (F06-T3) is done by **mutating a deserialized copy of the control fixture in
   memory**, never by adding fixture files — the pattern `LldReadyGate.test.ts:28-49` (U3-T3)
   already uses.
3. The `AUTHORSHIP.md` row F06 appends must say, in the file, that F06 **inherited** these fixtures
   unmodified and that the U1-authored pair is now doing the job it was written for:

   > | (no new fixtures) | — | F06 added none. `complete_module.json` and `one_line_freeze.json` were authored by U1 before any gate existed and are used **unmodified** as this gate's control and bad fixtures. F06's builder verified byte-identity against the merge base rather than asserting it. This is the full independence bar, not the one-step-short disclosure the four F02-era fixtures carry. |

4. **The builder must prove non-modification, not claim it**: `git diff <merge-base> -- fleet/contracts/fixtures/lld/` must show only the `AUTHORSHIP.md` hunk. That command goes in the evidence bundle.

---

## 7. Acceptance suite — write these first; they are the spec

**T1/T2.** All of §7 is written before the implementation and is **expected to be red**. The builder
may **add** cases; any diff that changes or removes an assertion below is a breach.

`fleet/keel/fleet/tests/f06_lld_ready.rs`, conventions per `f02_lld_crosslang.rs`:

```rust
const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/fixtures/lld");
fn load(name: &str) -> Value { /* read FIX/{name}.json */ }
fn brief_of(v: &Value) -> Value { /* v["module_brief"] if wrapper, else v */ }
fn refs() -> GateRefs { /* owners.v1.json + gate-refs.v1.json, read by the TEST, not the gate */ }
```

| id | name | asserts |
|---|---|---|
| **F06-T1** | `f06_t1_refuses_the_one_line_freeze_naming_at_least_five_check_ids` | `evaluate(load("one_line_freeze"), refs())` ⇒ `Outcome::NotReady`; `reasons` contains **≥5 distinct** `check_id`s; the set includes at least `C1-OPEN`, `C2-OWNER`, `R21-ALTS`, `R21-FAIL`, `SHAPE`. Mirrors U3-T1 and `03` §2.4's bad-fixture line. |
| **F06-T2** | `f06_t2_accepts_the_control_fixture_with_checked_driven_to_full_count` | `evaluate(load("complete_module"), refs())` ⇒ `Outcome::Ready`; `checked == 14`; `score.checks_total == 14`; `score.checks_passed == 14`; `score.ratio == 1.0`; `score.failed_check_ids.is_empty()`. **This is the S5 outage guard** — it proves a reachable accepting input exists (`03` §2.4). |
| **F06-T3** | `f06_t3_every_check_id_can_be_failed_in_isolation_from_the_control_fixture` | A **census**: for each of `GATE_CHECK_IDS` (loop bound is `GATE_CHECK_IDS.len()`, not `14`), apply a targeted mutation to a clone of the control fixture and assert the verdict is `NotReady` **and** `failed_check_ids == [that id]` — **exactly one, not "contains"**. Asserts the case table's length equals `GATE_CHECK_IDS.len()`, so a 15th id without a case is a **failure, not silent under-coverage**. **See §7.5 — the TS test of the same name is a sample, not a census, and F06 must not inherit that.** |
| **F06-T4** | `f06_t4_a_thirteen_of_fourteen_brief_is_not_ready_the_ratio_is_never_a_threshold` | Mutate the control fixture to fail exactly one check ⇒ `NotReady`, `score.ratio ≈ 0.929`, and `score.ratio > 0.9`. **The FEATURES.md divergence (§1.1) made executable**: a high ratio is not a pass. |
| **F06-T5** | `f06_t5_the_gate_is_pure_no_io_no_clock_no_randomness` | (a) **source grep** of `src/lld_ready.rs` (the `f02_t14` pattern): zero occurrences of `std::fs`, `File::`, `std::net`, `SystemTime`, `Instant`, `rand`, `reqwest`, `tokio`, `env::`, `include_str!`. (b) **behavioural**: `evaluate` on the control fixture 100× returns byte-identical `Verdict`s; and `evaluate` over a **key-order-shuffled** clone of the fixture returns an identical verdict (the F02 `lld-v1.test.ts` shuffle property, reused). |
| **F06-T6** | `f06_t6_an_empty_check_set_is_measured_nothing_never_ready` | **The one the TS test fakes (§4.2).** `evaluate_with(load("complete_module"), refs(), &[])` — a brief that is otherwise `Ready` 14/14 — ⇒ `Outcome::MeasuredNothing`, `checked == 0`, `score.is_none()`, `reasons.is_empty()`. Separately asserts `evaluate(...)` on that same input is `Ready`, so the test cannot pass by the fixture merely being bad. |
| **F06-T7** | `f06_t7_the_cli_refuses_a_shape_invalid_brief_before_the_gate_runs` | The `fleet gate lld-ready` entry function, given `one_line_freeze.json`, returns the **contract-validation** failure (not a gate verdict), and `evaluate` is not reached. Proves the §5.4 layering. Paired with a positive control: the same entry on `complete_module.json` reaches the gate and returns `Ready`. |
| **F06-T8** | `f06_t8_the_emitted_depth_evidence_validates_against_the_merged_freeze_schema` | `depth_evidence(&ready_verdict)` produces a block that, grafted into a `freeze` object, yields **zero** `freeze.depth_evidence.*` violations from `lld::validate_lld_v1`. Asserts `checked_check_ids` == all 14 ids in `GATE_CHECK_IDS` order and `failed_check_ids ⊆ checked_check_ids`. Also: `depth_evidence(&measured_nothing_verdict)` is `None`. **The gate consumes F02's validator; it does not re-implement the shape.** |
| **F06-T9** | `f06_t9_the_gate_is_the_only_writer_of_the_stamper_literal` | Source-greps `src/lld_ready.rs` for `"keel:lld-ready"`: **exactly one** occurrence, on the `STAMPED_BY` const line. Greps `src/lld.rs` and asserts its single occurrence is still the *comparison* at `check_freeze` (the F02 invariant must survive F06's visibility edit). Then: a stamp built from a `Ready` verdict is accepted by `lld::validate_lld_v1`; the `"orb:lld-ready"` variant is refused on path `freeze.stamped_by`. |
| **F06-T10** | `f06_t10_rust_and_ts_gates_agree_on_every_fixture` | The Rust half of the comparator: emits `{"gate":"lld-ready","mirror":"rs","fixtures":{<name>:{"outcome":…,"failed_check_ids":[sorted]}}}` over all 6 fixtures. The shell comparator below diffs it against the TS mirror's report. |

### 7.1 `fleet/tests/acceptance/lld-ready-selftest.sh` — the gate's own guard

`03` §2.4: *"a gate that passes its bad fixture is disabled at startup with a banner, not silently
trusted."* Runs on **every** `verify.sh`:

```
bad fixture   one_line_freeze.json  -> MUST be refused   (exit non-zero from `fleet gate lld-ready --selftest`)
control       complete_module.json  -> MUST be accepted  (the S5 outage guard)
```

Both directions, in one script, exit non-zero if **either** fails — including if it accepts the bad
fixture. Statuses captured directly, **never `$?` after a pipe** (`verify.sh:20`, E1/S11). An empty
or missing report reads as a **failure**, not as agreement (`lld-crosslang.sh`'s own rule).

### 7.2 `fleet/tests/acceptance/lld-ready-crosslang.sh` — the port's only real proof

Modeled line-for-line on `fleet/tests/acceptance/lld-crosslang.sh`. Two mirrors, one corpus, one
comparison, **exit 6** on any divergence (fleet's invariant code).

- Rust report ← `cargo test --test f06_lld_ready -- --ignored f06_t10_emit_report` (or a
  `fleet gate lld-ready --report <dir>` subcommand; the builder picks, the *output shape* is fixed).
- TS report ← a small `vitest`/`tsx` runner that calls `lldReady()` over the same 6 fixtures with the
  same refs, emitting the same JSON with `"mirror":"ts"`.
- Reports must be **byte-identical except the `mirror` key**.
- Success line: `ok 2 mirrors agree on 6 fixtures`. **Measuring nothing is a failure**: a report with
  `fixtures: {}` fails.
- Refs for both sides come from `owners.v1.json` + `gate-refs.v1.json`, so a refs change cannot make
  the two mirrors disagree for a reason that is not a real disagreement.

### 7.3 Prove the suite red before trusting it (T2)

Before any implementation, capture and keep:
1. `cargo test --test f06_lld_ready` — **fails to compile** (`lld_ready` does not exist). Paste it.
2. `bash fleet/tests/acceptance/lld-ready-selftest.sh` — fails, `fleet gate lld-ready` unknown. Paste it.
3. `bash fleet/tests/acceptance/lld-ready-crosslang.sh` — exits 6, `MIRROR FAILED: rs`. Paste it.

### 7.4 Mutation adequacy — three mandatory mutations, each with a named victim test

A green suite proves nothing until each of these has been made and reverted, with output captured:

| # | Mutation | Must turn red | Why this one |
|---|---|---|---|
| M1 | Delete the `checked == 0 ⇒ MeasuredNothing` guard in `evaluate_with` | **F06-T6** | §4.2 — if F06-T6 stays green, the branch is dead code again and the lane's headline fix is theatre |
| M2 | Change `Ready` to be `score.ratio >= 0.9` instead of `reasons.is_empty()` | **F06-T4** | §1.1/§3.4 — proves the threshold really is refused, not merely argued against in prose |
| M3 | In `R21-ALTS`, restore `let floor = if grain == "leaf" { 1 } else { 2 }` | **F06-T3**'s `R21-ALTS` case **and** F06-T10 | F02 §3.4/§10.1#4 — the loophole was closed in 5 places; prove Rust is the 6th and that the comparator catches its reopening |

If M3 turns F06-T3 red but leaves F06-T10 green, the comparator is not actually comparing —
**that is a finding, report it.**

### 7.5 The TS mutation table is a **sample**, not the census its name claims — do not copy it

The builder will reach for `LldReadyGate.test.ts:28-49` (U3-T3) as F06-T3's mutation table. **Read it
first.** It is named *"every check id can be failed in isolation"* and its inline comment says
*"Each mutation below must trip exactly its own check and no other."* Neither is true of what it
does:

- **It covers 8 of 14 ids.** The `mutations` record has keys `C1-OPEN`, `C2-OWNER`,
  `C3-ACC-NONTAUT`, `R17-DERIV`, `R19-ABSOLUTE`, `R21-ALTS`, `R21-FAIL`, `C12-STORE`.
  **`C3-ACC-PARSE`, `C3-ACC-GROUND`, `C12-DEPS`, `REG-VERDICT`, `IFACE` and `SHAPE` have no
  isolation coverage at all** — including `SHAPE`, the check that carries the reused, previously
  buggy `valid_node_id` (§2.1).
- **It asserts `.toContain(id)`, not equality**, so even for the 8 it does cover it never proves the
  mutation trips *only* that check. A mutation that fails five checks passes this test.
- The denominator is `Object.entries(mutations).length` — **the sample's own size** — so the test
  cannot notice that six ids are missing, and adding a 15th check would leave it green.

This is `gate-census-not-sample` exactly: a coverage claim whose denominator is derived from what was
measured rather than from what exists. **F06-T3's loop bound is `GATE_CHECK_IDS.len()` and its
assertion is set-equality**, and the builder must author the six missing mutations. Each is a real
(small) design question, not a transcription — e.g. `SHAPE` must be tripped via `purpose: ""`,
**not** via `owner_path` or `node_id`, which cascade into `C3-ACC-GROUND` and `C12-STORE`
respectively and would break the exactly-one assertion. The eight TS cases are a starting point for
eight of fourteen, no more.

Flag it forward (§13 item 6); fixing the TS test is not F06's lane.

---

## 8. `fake_cost()` — the incentive paragraph, shipped with the gate (`03` §2.3)

Ships as a doc-comment on `evaluate`, adapted from `03` §2.3 to what this tree actually enforces:

> **Cheapest green = the correct fix, for 13 of 14 checks.** To go green a brief must close its open
> questions (`C1-OPEN` — the real act); name an owner that appears in `owners.v1.json`
> (`C2-OWNER` — a reference, not a string you can invent); express acceptance as a structured
> predicate whose `artifact` lies under the node's own `owner_path` and is named in its own `then`
> (`C3-ACC-*` — you cannot hand-wave that into existence); carry guarantees whose numeric
> derivations contain real arithmetic and whose structural ones name a resolvable file/type/gate
> (`R17-DERIV`); back any absolute claim with a structural kill (`R19-ABSOLUTE`); carry ≥2 distinct
> killed alternatives with non-placeholder revive triggers (`R21-ALTS`); and own its data and deps
> consistently (`C12-*`, `REG-VERDICT`). **The two illegitimate-cheap paths are named, not hidden:**
> (1) *plausible-but-shallow field content* — real metric names, thin thinking — which no
> deterministic check reaches; the advisory judge (`03` §2.6, not built) and the human are the only
> mitigations, so depth-*content* is `mitigates`, never `kills`. (2) **`R21-FAIL`'s `blast_radius`
> is a presence check, not a derivation** — the gate cannot recompute it in this tree (§3.2), so a
> proposer's prose is taken at face value there. Everything else the proposer writes is compared
> against something it does not control. There is no field whose green state a proposer can set
> directly; `stamped_by` is refused by the schema for every value but the gate's own const
> (`lld.rs:653-666`). Forging readiness requires being the gate, which requires being `keel`.

Stating (2) out loud is the point. `03` §8 already concedes it; a gate that claimed a derived
blast radius here would be a proxy sold as the property.

---

## 9. Done-definition

A checkbox is not done until its evidence line is in the bundle. **Evidence, not assertion.**

1. `fleet/keel/fleet/src/lld_ready.rs` exists; `GATE_CHECK_IDS` is 14 ids byte-identical to
   `LldReadyGate.ts:19-34`; `checks_total` is derived from `GATE_CHECK_IDS.len()`, not a literal.
2. `cargo test --test f06_lld_ready` — **10/10 green**, output pasted.
3. **The §7.3 red-first capture and all three §7.4 mutation results are in the bundle** — the actual
   captured output, including the red ones. "Tests pass" is not evidence; the red-then-green pair is.
4. `bash fleet/tests/acceptance/lld-crosslang.sh` still exits 0 with
   `3 mirrors agree on 6 fixtures` — **proving the `lld.rs` visibility edit changed no logic.**
5. `bash fleet/tests/acceptance/lld-ready-selftest.sh` exits 0, both directions proven (the bad
   fixture refused **and** the control accepted).
6. `bash fleet/tests/acceptance/lld-ready-crosslang.sh` exits 0 with `2 mirrors agree on 6 fixtures`.
7. `git diff <merge-base> -- fleet/contracts/fixtures/lld/` shows **only** the `AUTHORSHIP.md`
   hunk (§6 item 4). Command and output in the bundle.
8. `bash fleet/verify.sh` — full result pasted, red included. Any pre-existing failure isolated with
   a **disposable detached worktree** (`git worktree add --detach`), never `git stash` (§11.2).
   Known baseline as of `6fd66f3`: ~14 passed / 7 failed of 22, with `f08_pr_emit.rs`'s test-dir race
   intermittently tripping `unit tests`/`coverage` — diff the failure **set** against the baseline,
   not the exit code.
9. `cd orb && npx vitest run apps/mobile/src/build/` still green (18/18) — F06 does not touch orb,
   and this proves it.
10. `FEATURES.md`'s F06 row is repaired: column-shift corruption fixed, `☑` corrected, `Branch (C1)`
    = `extract (C2) — port LldReadyGate.ts's 14 checks + reuse lld.rs's predicates`, and the §1.1
    threshold divergence recorded in one line.
11. A dated `FLEET-LEARNINGS.md` entry appended at the **absolute** path
    `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md` — never
    `Light/FLEET-LEARNINGS.md`, never a relative guess from inside a worktree (§11.2).
12. `status/F06-lld-ready-gate--depth-bar-enforcement-.status` moved through
    `building → verifying → verified` with `SINCE` updated **each** time, then `bash status/render.sh`.
13. **PR opened, not merged.** This lane adds a gate to `verify.sh` — it changes what CI accepts, so
    A15/D4 human-merge applies. No self-approve.

---

## 10. Routing (A17), and sizing

| Phase | Model | Why |
|---|---|---|
| **Build** | **mid-engineer (Sonnet)** | Six hand-ported predicates with no `regex` crate, a purity invariant, a reachability seam, and a two-language comparator. The `{2,63}` bug (§3.3) is exactly the failure mode a mechanical transcription produces. **Do not route to junior-engineer** — it reads as a transcription and is not one. |
| **Verify** | **verifier (Sonnet, never Haiku)** | Must independently re-derive §7.3's red-first and **all three** §7.4 mutations in a clean worktree, and must personally check §6 item 4 and done-item 4. |

**Size: one focused session.** Roughly 400 lines of Rust (gate + tests), one JSON reference file,
two shell scripts modeled on an existing one, and four small edits. Fleet-only — **no `orb/` edit**,
which is what keeps it to one session (§5.4's layering decision is what bought that).

---

## 11. Landmines already paid for — do not rediscover

### 11.1 Specific to this lane

1. **No `regex` crate in this workspace** (`lld.rs:59-63`). All six TS regexes are hand-ported. The
   two hardest: `ABSOLUTE_CLAIM`'s `\b…\b` **word boundaries** (a naive `contains("never")` matches
   `"whenever"` and would refuse a correct brief — a false positive, which this estate rates worse
   than a miss), and the five **anchored, case-insensitive** tautology patterns (`/^(it )?works?$/i`
   must not match `"it works when the cache is warm"`). Write the boundary/anchor cases as unit
   tests inside `lld_ready.rs`, not only in the contract suite.
2. **`purpose` is `.chars().count()`, not `.len()`** (§5.2). Bytes ≠ characters; `lld.rs:214` gets
   this right and is the model.
3. **`ratio` must round the way TS rounds** — `Math.round(x*1000)/1000` (`LldReadyGate.ts:239`). A
   different rounding makes the comparator diverge on a fixture that genuinely agrees. `f64` equality
   in F06-T2/T4 compares against the rounded value, not a raw quotient.
4. **The `grain: 'leaf'` floor exemption is closed in 5 places and must stay closed** (§5.2). It is
   the one place where the blueprint's prose (`03` Operator's-scar #5) and the merged contract
   (F02 §3.4) disagree, and the merged contract wins.
5. **`main.rs` and `verify.sh` are shared append targets** with F02 and F08. Re-run the whole
   `verify.sh` after any rebase; do not read a clean merge as a clean build (F02 §9.4).
6. **`graph.rs`'s `every_shipped_module_is_reachable_from_dispatch`** parses every file under `src/`
   with tree-sitter and fails the module on any parse error. Two consequences: `lld_ready.rs` needs a
   **real dispatched caller in `main.rs`** (a `pub mod` with no CLI path fails that test — the reason
   `contract lld validate` exists at all, per `main.rs:3462-3463`), and a local named `raw`
   immediately followed by `&raw` trips a grammar ambiguity (`lld.rs:866-869`).

### 11.2 Estate-wide, all confirmed applicable to this tree

7. **Never `git stash` in any worktree here.** `refs/stash` is one shared stack across every worktree
   of a repo; a sibling lane's `pop` takes yours. This has already cost two sessions — one of them
   literally named `sot-lld-ready-gate`. Use `git worktree add --detach /tmp/f06-baseline <ref>`.
8. **Never background a slow command and wait.** A subagent gets no notification for its own
   background children and will sit forever. Foreground, or `timeout N <cmd>`.
9. **`verify.sh` never uses `$?` after a pipe** (its own comment, `verify.sh:20`, D22/D23/E1). Both
   new scripts follow it — captured statuses, no pipes into the check.
10. **Write `FLEET-LEARNINGS.md` to its absolute path.** Two prior units created a stray in-repo copy
    instead, and one was then deleted with a false "already folded in" claim; the content was lost
    until a verifier caught it.
11. **Check for live writers before attributing any regression.** Twice, a background agent faked a
    test regression in this tree (one deleted `bin/route.sh`; another `git init`-ed `fleet/` and
    deleted `bin/fanout.sh`). `ps` alone misses the second kind.
12. **The Bash tool wraps `grep` with `ugrep`;** a shell script sees `/usr/bin/grep` with different
    regex semantics. F06-T5 and F06-T9 are source-greps — probe them the way the *script* runs, not
    the way the agent's shell runs.

---

## 12. Explicitly out of scope

- **F07 — fleet SOW intake.** `sow.rs` (`EXIT_READY_AWAITING_REVIEW = 9` at `sow.rs:13`,
  `review_status()` at `:109`), `fleet sow`, and the 4th mirror `fleet/crew/crew/lld.py` that F02
  §10.1 item 3 makes a hard requirement on F07. **F07 consumes this gate's `Verdict` and
  `depth_evidence`; F06 does not call `sow.rs` and does not add a `crew` mirror.**
- **F09 — the seam.** F06 defines and stamps; nobody sends an `lld.v1` across the boundary yet.
- **The freeze ledger.** `03` §4.2/§4.3's append-only supersede/reopen mechanics. F06 emits a
  `depth_evidence` block and a stamp; the ledger that stores and versions freezes is F12's.
- **The keel gate kernel / `trait Gate`.** §3.1. Revisits when a second gate lands.
- **The blast-radius conjunct.** §3.2. Revisits when `graph.rs` has a pure `impact()` **and**
  `registry/` is non-empty.
- **The advisory depth-content judge** (`03` §2.6) and the `BeliefModel` belief signal — that is F04's
  server-side register work, and it must never become a term in `evaluate`.
- **A calibrated depth threshold and a labeled corpus.** §1.1/§3.4 — **F17**.
- **Retiring or rewriting `orb/apps/mobile/src/build/LldReadyGate.ts`.** It stays as the TS mirror.
  F06 adds the comparator, not a replacement. Re-homing is a later lane (F02 §10.1 item 4's
  *"eventually"*).
- **`design-graph.v1`, the node lifecycle, `lane-status.v1` push** (`03` §3, F21).
- **Any codegen** (F35).

---

## 13. Flagged, not fixed — hand these forward

| # | Finding | Owner |
|---|---|---|
| 1 | **`LldReadyGate.ts:220-222`'s `MEASURED_NOTHING` branch is unreachable dead code, and `LldReadyGate.test.ts:69-71` (U3-T6) does not test it** — it asserts `not.toBe('READY')` on a brief that fails `C2-OWNER`. The blueprint's #1 operator scar is unenforced TS-side. F06 fixes the **Rust** side (§4.2). | The orb-side fix (export a `lldReadyWith(brief, refs, checks)` seam and make U3-T6 drive it) is **out of F06's scope** but should be spawned. Until then the TS mirror carries the hole. |
| 2 | `registry_paths` / `known_node_ids` resolve against a **checked-in list**, not a live registry — `Light/registry/{features,services,modules}` are empty. `C12-DEPS` and `REG-VERDICT` are therefore `mitigates`, not `kills` (§5.3). | **F25** (registry install/extract/build-new gate) / whichever lane first puts a real module in `registry/`. |
| 3 | `FEATURES.md`'s F06 row is **column-shift corrupted** (F02's status text occupies F06's Branch and Acceptance cells) **and wrongly marked ☑** for a gate with no Rust implementation. The F03 row carries the same corruption. | F06 repairs its own row (§9 item 10). **The F03 row is someone else's** — flag it, do not edit another lane's row. |
| 4 | `fleet/contracts/attestation.v1.json`'s `subject.digest.required: ["blake3"]` cannot accept a SHA-256 `content_hash` (F02 §10.1 item 1, still open). | **F07 / F09.** Editing another contract is human-merge. |
| 5 | `lld.rs`'s module doc (`:14-17`) claims *"No production caller exists yet in this crate"* — stale since `main.rs:3448` wired `fleet contract lld validate`. One-line doc fix, not F06's file to churn. | Whoever next edits `lld.rs` substantively. |
| 6 | **`LldReadyGate.test.ts:28-49` (U3-T3) is a sample sold as a census** (§7.5): 8 of 14 ids, `.toContain` instead of equality, and a denominator taken from the sample itself. Six checks — `C3-ACC-PARSE`, `C3-ACC-GROUND`, `C12-DEPS`, `REG-VERDICT`, `IFACE`, `SHAPE` — have **zero** isolation coverage TS-side. | Orb-side fix is **out of F06's scope**; spawn it. F06's own T3 is a true census, and F06-T10's comparator will surface any Rust/TS behavioural gap in those six regardless. |
