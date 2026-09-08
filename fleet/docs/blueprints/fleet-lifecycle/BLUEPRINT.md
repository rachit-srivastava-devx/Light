# BLUEPRINT — `fleet-lifecycle`

## 1. Header

- **Crate:** `fleet-lifecycle`
- **One-line purpose:** The compile-time type-state machine for a fleet task — 16 marker states, a
  sealed `Task<S>`, and pure transition methods that each consume the old task and return
  `(Task<NewState>, TransitionReceipt)` as a value, making an illegal lifecycle edge a compile error
  rather than a runtime check.
- **Build branch:** `refactor` (MIGRATION-PLAN §3 row 15) — lift the pure ~40% of
  `fleet/keel/fleet/src/lifecycle.rs` (1478 lines, read in full 2026-09-08) near-as-is; the IO-bearing
  ~60% (persistence, CLI, the two orchestration entry points) does **not** come with it — see §5 and
  the divergence note at the end of this file.
- **Imports:** `fleet-types` — `LifecycleState` (the runtime-vocabulary enum, used only by the
  divergence-note discussion below; this crate's own compiled edges are the source of truth for what
  is legal, see §4) and `GateRefusal` (this crate's sole error type — the `Refusal` struct in
  `lifecycle.rs:134-163` is **not** re-declared here; see §5).
- **Imported by:** `src/` (composition root — owns the persisted runtime bridge: loading a state
  string from disk, calling this crate's `resume`/`advance_any`, persisting the result, and the CLI's
  `fleet lifecycle <states|advance|show>` surface); `fleet-store` (implements `ReceiptLedger` against
  its own append-only ledger and constructs `ChangeEmitter` around a real `git`/`gh` push, both
  supplied as trait objects at the caller's boundary — this crate never depends back on `fleet-store`);
  `fleet-worker` (drives a task's own `Task<S>` through `Leased → Building → Built` while supervising
  a dispatch, using the typed edges directly, not the string-driven `resume` bridge).

## 2. Responsibility & non-goals

**Owns:** the 16 lifecycle marker states (`Intake` … `Refused`) and the sealed `State` trait that
makes them a closed set; the generic `Task<S: State>` struct, whose private fields (`id`,
`retry_depth`, `_s: PhantomData<S>`) only code inside this crate may construct or read; every
transition method (`specify`, `review`, `decompose`, …, `propose`, `observe`, `reopen`, `refuse` per
state) as an inherent impl on `Task<$FromState>`, each returning `Result<Task<$ToState>, GateRefusal>`
(or, for `propose`, `Result<(Task<Proposed>, ProposedChange), GateRefusal>`); the `HumanApproval`
token type (proof a human, not an agent, approved a gated edge — mintable only inside this crate);
the `AttestationBundle`/`ChangeEmitter`/`ProposalRequest`/`ProposedChange` types the `propose` edge
needs to decide whether a pull request may be opened, and the pure completeness checks over an
attestation's `predicate.elements`; and — because only this crate may touch `Task<S>`'s private
fields (see the divergence note) — the sanctioned re-entry API (`resume`, `AnyTask`, `advance_any`)
that lets a caller who only has a persisted state *name* (a `&str` loaded from disk) get back into the
typed world without this crate exposing its fields publicly.

**Non-goals (the seam):**
- Does **not** persist anything. `ReceiptLedger` is a trait the caller supplies; this crate never
  opens a file, reads an environment variable, or touches the clock. The concrete `FileLedger`
  (`lifecycle.rs:595-618`), `load_state`/`persist_state` (`lifecycle.rs:645-668`), and `state_dir`
  (`lifecycle.rs:551-565`) all move to the caller — `fleet-store` owns the ledger-append
  implementation, `src/` owns where the "current state" file lives.
- Does **not** push a branch or call `gh`/`git` — `ChangeEmitter` is a trait the caller supplies; the
  real implementation (worktree push + `gh pr create`) stays in `src/`/`main.rs`, exactly as
  documented in `lifecycle.rs:296-302`'s own comment (a lib crate cannot depend on the bin that
  drives it).
- Does **not** parse CLI arguments, print to stdout/stderr, or decide process exit codes — `command`,
  `parse_task`, `advance_to`, `refuse_runtime`, `report_environment`, `drive_run` (all of
  `lifecycle.rs:945-1149`, `997-1072`) are CLI/IO glue and belong in `src/`.
- Does **not** decide *when* a real dispatch calls which edge, or supervise a subprocess — that's
  `fleet-worker`'s job; this crate only exposes the edges as typed methods.
- Does **not** re-derive `attest_verify_inner`'s full attestation validation (the deep cross-check of
  `blast_radius` against real diff bytes, or of `cost` against a real token meter) — `AttestationBundle::missing_element` checks structural completeness only, exactly as `lifecycle.rs:368-393`'s own
  doc comment states; the one real validator stays in `src/` (`main.rs`'s `attest_verify_inner`), and
  `fleet pr emit` must still run it before this crate's `propose` edge is ever reached.
- Does **not** duplicate `fleet-types::LifecycleState`'s runtime vocabulary as a second enum. Where
  this crate needs "the name of a state as data" (the `resume`/`advance_any` re-entry bridge), it
  matches on `&str` against its own 16 states directly (mirroring `lifecycle.rs`'s `STATES`/
  `canonical_next` tables) rather than importing `LifecycleState` and converting both ways — see the
  divergence note for why a two-way sync between this crate's compiled edges and `fleet-types`'s data
  table is a real maintenance seam, not a false one.

## 3. Public API contract

```rust
//! Compile-time lifecycle for a fleet task. A state is represented by the type parameter of
//! `Task<S>`. Every transition consumes the old task, calls the caller-supplied `ReceiptLedger`,
//! and returns a fresh `Task` typed to the new state -- or a `GateRefusal`, with the old task
//! (and the ledger) left untouched. This crate performs no IO: `ReceiptLedger` and `ChangeEmitter`
//! are ports the caller implements; this module only decides what transitions are legal.

use fleet_types::GateRefusal;
use std::marker::PhantomData;
use serde_json::Value;
use std::path::PathBuf;

// ---- states -----------------------------------------------------------------------------------

mod sealed { pub trait Sealed {} }

/// Marker implemented only by the 16 lifecycle states declared in this crate. Sealed: no
/// downstream crate can invent a 17th state and have it accepted by `Task<S>`.
pub trait State: sealed::Sealed {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Intake;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Specified;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Reviewed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Decomposed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Contracted;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Briefed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Leased;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Building;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Built;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Verifying;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Verified;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Attested;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Accepted;
/// The change has been pushed and a real pull request opened. Not `Merged`/`Landed` -- the change
/// is proposed for human integration, never self-merged (author != integrator). Terminal for an
/// agent: there is no `merge` edge and never will be (`merge_is_not_a_lifecycle_edge` compile-fail
/// test enforces this).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Proposed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Observed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct Refused;

// seal_states!(...) implements `sealed::Sealed` + `State` for all 16 structs above (lifecycle.rs:81-93, verbatim).

// ---- identity, receipts, approval --------------------------------------------------------------

/// Stable identifier carried through every transition. Never empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// `Err(GateRefusal{code: "EMPTY_TASK_ID", ..})` iff `value.trim()` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, GateRefusal> { unimplemented!() }
    pub fn as_str(&self) -> &str { &self.0 }
}
// impl fmt::Display for TaskId -- writes the raw id (lifecycle.rs:113-117, verbatim).

/// A transition receipt awaiting the caller's own timestamp/actor stamping. `from`/`to` are the
/// `'static` type names of the marker structs (`std::any::type_name::<S>()`), never a
/// caller-suppliable string -- the receipt can only name a real compiled state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionReceipt {
    pub task_id: TaskId,
    pub from: &'static str,
    pub to: &'static str,
    pub evidence: String,
}

/// The only capability a transition needs from the caller's evidence ledger. The caller (today
/// `fleet-store`) implements this against a real hash-chained append-only file; this crate never
/// constructs a `ReceiptLedger` itself.
pub trait ReceiptLedger {
    fn append(&self, receipt: TransitionReceipt) -> Result<(), GateRefusal>;
}

/// Proof that a human, rather than an agent, approved a gated edge (`review`, `accept`). Only
/// code in this crate can mint one -- the daemon's human-approval boundary in `src/` calls this
/// constructor; nothing downstream can forge approval by constructing the token some other way.
#[derive(Debug)]
pub struct HumanApproval { evidence: String }

impl HumanApproval {
    /// `Err(GateRefusal{code: "EMPTY_HUMAN_APPROVAL", ..})` iff `evidence.trim()` is empty.
    pub fn recorded(evidence: impl Into<String>) -> Result<Self, GateRefusal> { unimplemented!() }
}

// ---- the task and its edges ---------------------------------------------------------------------

/// A task whose legal operations are determined entirely by `S`. Fields are private to this
/// crate: no caller, including `src/`, can construct or inspect a `Task<S>` except through the
/// typed edges below or the sanctioned `resume` bridge (§ "resume"). This is what
/// `construct_verified` (compile-fail test) enforces.
#[derive(Debug)]
pub struct Task<S: State> { id: TaskId, retry_depth: u32, _s: PhantomData<S> }

impl<S: State> Task<S> {
    pub fn id(&self) -> &TaskId { &self.id }
    pub fn retry_depth(&self) -> u32 { self.retry_depth }
    // fn transition<N: State>(self, evidence, ledger) -> Result<Task<N>, GateRefusal> is private;
    // every public edge below is a thin wrapper over it (lifecycle.rs:525-548, verbatim except
    // `Refusal` -> `GateRefusal`).
}

impl Task<Intake> {
    pub fn new(id: TaskId) -> Self { unimplemented!() }
    pub fn specify(self, evidence: impl Into<String>, ledger: &impl ReceiptLedger) -> Result<Task<Specified>, GateRefusal> { unimplemented!() }
    pub fn refuse(self, reason: impl Into<String>, ledger: &impl ReceiptLedger) -> Result<Task<Refused>, GateRefusal> { unimplemented!() }
}

impl Task<Specified> {
    /// Requires a `HumanApproval` -- an agent cannot self-review (`non_human_review` compile-fail test).
    pub fn review(self, approval: HumanApproval, ledger: &impl ReceiptLedger) -> Result<Task<Reviewed>, GateRefusal> { unimplemented!() }
    pub fn refuse(self, reason: impl Into<String>, ledger: &impl ReceiptLedger) -> Result<Task<Refused>, GateRefusal> { unimplemented!() }
}

// The straight-through edges (one `impl Task<$From> { pub fn $method(...) -> Result<Task<$To>, GateRefusal> }`
// per row, generated by the `edge!` macro, lifecycle.rs:240-264 verbatim):
//   Reviewed::decompose -> Decomposed   Decomposed::contract -> Contracted
//   Contracted::brief -> Briefed        Briefed::lease -> Leased
//   Leased::build -> Building           Building::finish_build -> Built
//   Built::begin_verification -> Verifying   Verifying::verify -> Verified
//   Verified::attest -> Attested        Proposed::observe -> Observed
//   Observed::reopen -> Intake

// The refusal edges (one `impl Task<$From> { pub fn refuse(...) -> Result<Task<Refused>, GateRefusal> }`
// per row, generated by the `refusal_edge!` macro, lifecycle.rs:266-284 verbatim):
//   Building, Built, Verifying, Attested, Accepted

impl Task<Attested> {
    /// Requires a `HumanApproval` (`accept` is a human-only gate, symmetric with `review`).
    pub fn accept(self, approval: HumanApproval, ledger: &impl ReceiptLedger) -> Result<Task<Accepted>, GateRefusal> { unimplemented!() }
}

// ---- the propose edge and its evidence types -----------------------------------------------------

/// Push `request.head` and open a pull request. MUST return the real PR URL on success --
/// returning `Ok` without one (e.g. `gh` exiting 0 with no URL parsed) is itself a bug in the
/// implementation, not something this trait can prevent structurally; the implementation is the
/// caller's responsibility (`src/`, today `main.rs`'s real worktree-push + `gh pr create`).
pub trait ChangeEmitter {
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, GateRefusal>;
}

/// Everything `propose` needs to ask a `ChangeEmitter` to open a pull request. `diff` carries the
/// actual attested bytes so `propose` verifies the digest itself rather than trusting the caller
/// already checked it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalRequest {
    pub repo: PathBuf, pub base: String, pub head: String,
    pub artifact_id: String, pub diff: Vec<u8>, pub title: String, pub body: String,
}

/// What a successful `propose` actually did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedChange { pub url: String, pub head: String, pub commit: String, pub changed_files: u64 }

/// The elements of a delivery attestation `propose` must find structurally complete before it
/// asks a `ChangeEmitter` to open a pull request. Built from the attestation's
/// `predicate.elements` object; does not re-derive the caller's own deep validation of it.
#[derive(Clone, Debug)]
pub struct AttestationBundle { elements: Value }

impl AttestationBundle {
    pub fn new(elements: Value) -> Self { Self { elements } }

    /// The pinned set of 8 elements `attest_verify_inner` (the caller's validator) enforces today
    /// -- not the aspirational 9th. A test asserts this array matches that enforced set so a 9th
    /// element lands consciously, never silently.
    pub const REQUIRED: [&'static str; 8] = [
        "sow", "blind_suite", "independent_verification", "adequacy",
        "blast_radius", "rollback", "cost", "oracle_independence",
    ];

    /// The first required element missing or structurally incomplete, or `None` if all 8 are
    /// present -- `independent_verification`/`oracle_independence`/`blind_suite` are checked to
    /// their full documented shape; the other five need only be present and non-null.
    pub fn missing_element(&self) -> Option<&'static str> { unimplemented!() }
}

impl Task<Accepted> {
    /// `Accepted -> Proposed`: emit a real pull request carrying the attested diff. The agent's
    /// terminal act -- there is no `merge` edge. Gate order is deliberate and must stay
    /// side-effect-free until the last line: (1) attestation completeness
    /// (`missing_element`), (2) `blake3::hash(&request.diff)` must equal `request.artifact_id`
    /// (`ARTIFACT_DIGEST_MISMATCH`), (3) `request.diff` must be non-empty (`EMPTY_DIFF`) --
    /// ALL three before `emitter.emit` runs; a refused proposal must push nothing and must never
    /// call the emitter. Port verbatim from `lifecycle.rs:474-513`, s/Refusal/GateRefusal/.
    pub fn propose(
        self,
        bundle: &AttestationBundle,
        request: &ProposalRequest,
        emitter: &impl ChangeEmitter,
        ledger: &impl ReceiptLedger,
    ) -> Result<(Task<Proposed>, ProposedChange), GateRefusal> { unimplemented!() }
}

// ---- resume: the sanctioned re-entry point for a caller that only has a state NAME -------------

/// Every state, erased to a single enum so a caller that only knows a task's state as a persisted
/// `&str` (loaded from disk) can get back into the typed world. Exists ONLY because `Task<S>`'s
/// fields are private to this crate (§ "Task<S>" above) -- no code outside `fleet-lifecycle` can
/// construct e.g. `Task<Built>` from a bare `TaskId` any other way, by design (`construct_verified`
/// compile-fail test). This is the crate's answer to the divergence note below.
pub enum AnyTask {
    Intake(Task<Intake>), Specified(Task<Specified>), Reviewed(Task<Reviewed>),
    Decomposed(Task<Decomposed>), Contracted(Task<Contracted>), Briefed(Task<Briefed>),
    Leased(Task<Leased>), Building(Task<Building>), Built(Task<Built>),
    Verifying(Task<Verifying>), Verified(Task<Verified>), Attested(Task<Attested>),
    Accepted(Task<Accepted>), Proposed(Task<Proposed>), Observed(Task<Observed>),
    Refused(Task<Refused>),
}

/// Reconstruct a task in the state named by `state` (one of the 16 wire names, e.g. `"Built"`).
/// Pure: no IO, just a match + the private constructor. `Err(GateRefusal{code: "UNKNOWN_LIFECYCLE_STATE", ..})`
/// for any other string -- the caller is responsible for having read `state` from a place that
/// only ever writes one of the 16 names (this crate does not validate that its OWN past writes are
/// trustworthy; it validates the string in front of it right now).
pub fn resume(state: &str, id: TaskId, retry_depth: u32) -> Result<AnyTask, GateRefusal> { unimplemented!() }

/// Advance `any` exactly one step along its single canonical forward edge (mirrors
/// `lifecycle.rs`'s `canonical_next`/`typed_advance`, `Refusal` -> `GateRefusal`), using
/// `HumanApproval::recorded` internally for the two human-gated edges (`review`, `accept`) --
/// this makes `advance_any` a "just keep going" driver for automated re-drive, NOT a way to
/// bypass the human-approval requirement (the evidence string IS the approval evidence).
/// `Accepted` has no canonical next: `propose` needs a `bundle`/`request`/`emitter` this fn does
/// not carry, so it returns `Err(GateRefusal{code: "PR_EMIT_REQUIRES_EVIDENCE", ..})` naming the
/// caller's real entry point instead (mirrors `lifecycle.rs:897-909`). `Refused` has no forward
/// edge at all: `Err(GateRefusal{code: "ILLEGAL_LIFECYCLE_TRANSITION", ..})`.
pub fn advance_any(any: AnyTask, evidence: impl Into<String>, ledger: &impl ReceiptLedger) -> Result<AnyTask, GateRefusal> { unimplemented!() }
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `Task<S: State>` | Every legal transition is a `pub fn` on `impl Task<$From>` returning `Task<$To>`; the compiled set of `impl` blocks IS the edge set. Fields private to this crate. | `task.verify()` when `task: Task<Intake>` — no such method exists (`E0599`, `intake_to_verified` compile-fail test); `Task::<Verified> { id, retry_depth, _s }` built directly — private-field error (`E0451`, `construct_verified` test); calling any edge method twice on the same binding — moved-value error (`E0382`, `reuse_consumed` test, since `transition` takes `self` by value and no state implements `Copy`). |
| `State` (sealed) | Exactly 16 implementors, all declared in this crate; `sealed::Sealed` is not `pub`, so no downstream crate can add a 17th and have `Task<TheirState>` accepted anywhere. | A caller inventing a state outside the reviewed 16 and having any `impl Task<$From>` accept it as `$To`. |
| `Proposed` / absence of `Merged` | There is no `Task<Proposed>::merge` method anywhere in this crate, and never will be — `merge_is_not_a_lifecycle_edge` is a standing compile-fail regression test, not a one-time check. | An agent self-merging its own proposed change (`author != integrator`, FLEET-LEARNINGS.md SDLC gate model) — made structurally impossible, not policy-forbidden. |
| `HumanApproval` | Constructible only via `HumanApproval::recorded`, which itself requires non-empty evidence; `review`/`accept` take `HumanApproval` by value (not `&str`), so a caller cannot substitute a bare string for a real approval. | `specified.review("agent says yes", &ledger)` — type mismatch, `E0308` (`non_human_review` compile-fail test: `&str` where `HumanApproval` is expected). |
| `TransitionReceipt.{from,to}` | Always `std::any::type_name::<S>()` for a real, compiled marker struct — never a caller-suppliable string. | A receipt claiming a transition between two states that were never actually adjacent types at the call site (the receipt is derived from the type parameters the compiler already checked, not re-typed by hand). |
| `AttestationBundle::REQUIRED` | Exactly the 8 elements `attest_verify_inner` (the caller's validator) enforces today, asserted by a pinned unit test (§9) so a 9th element lands consciously. | `propose` silently accepting an attestation missing an element the caller's real validator requires, because this crate's list quietly drifted from it. |
| `Task<Accepted>::propose` | Gate order fixed: completeness → digest match → non-empty diff, all before `emitter.emit`; a `GateRefusal` from any of the three means the emitter never ran (verified by `RecordingEmitter` unit tests, §9). | A refused proposal that still pushed a branch or opened a PR — the gate must run before the side effect, not clean up after it. |
| `AnyTask` / `resume` | `resume` is the *only* way outside this crate to obtain a `Task<S>` for an `S` other than `Intake` (via `Task::new`) without walking every intermediate edge; it goes through the same private constructor every internal edge uses, so it can never produce a `Task<S>` inconsistent with `S` actually being state `S`. | A caller fabricating a `Task<Built>` for a task that was actually last recorded `Refused` by hand-rolling the private struct literal — impossible (private fields); `resume` is the one sanctioned, auditable path, and it is a pure function of the string handed to it, nothing more. |

**Money/precision:** no money type in this crate. `retry_depth: u32` is a count, not a precision-
sensitive value; no float appears anywhere.

**Clock/RNG/IO injection points:** none inside `Task<S>`'s edges or `resume`/`advance_any` — every
fact a transition needs (evidence string, `HumanApproval`, the ledger, the emitter) is a parameter.
`TransitionReceipt` carries no timestamp; the caller stamps time and actor when it persists the
receipt (this is explicit in the header's one-line purpose and repeated here because it is the
single most load-bearing constraint of this crate: no `SystemTime::now()`, no `env::var`, no `fs::`,
no `std::io` call anywhere in `src/`).

## 5. Reuse map

Source read in full: `fleet/keel/fleet/src/lifecycle.rs` (1478 lines, 2026-09-08) and its compile-fail
suite `fleet/keel/fleet/tests/compile_fail/*.rs` (6 cases + `.stderr` goldens) plus the harness
`fleet/keel/fleet/tests/compile_fail.rs` (`trybuild::TestCases::new().compile_fail(...)`).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `lifecycle.rs:16-33` (`STATES`) | String-keyed edge table used only by the runtime (`advance_to`/`command`) to validate a requested edge before dispatching. | **no** | Runtime-only concern; the caller validates against `fleet_types::LifecycleState::allowed_next` instead of a second copy of this table (see §2's non-goal and the divergence note on keeping the two in lockstep). |
| `lifecycle.rs:35-93` (`sealed`, `State`, 16 marker structs, `seal_states!`) | The closed state set. | yes, verbatim | None — this is the crate's core. |
| `lifecycle.rs:96-117` (`TaskId`, `Display`) | Non-empty stable id. | yes | `Refusal` → `GateRefusal` (from `fleet-types`). |
| `lifecycle.rs:119-131` (`TransitionReceipt`, `ReceiptLedger`) | Receipt shape + injected ledger port. | yes | `Refusal` → `GateRefusal` in the trait's `Result`. |
| `lifecycle.rs:134-163` (`Refusal`) | Local `{code, message}` error type. | **no — replaced** | This crate uses `fleet_types::GateRefusal` instead of re-declaring an identical shape; see MIGRATION-PLAN §3 row 1 and the divergence note. |
| `lifecycle.rs:169-186` (`HumanApproval`) | Human-approval token, `pub(crate)` constructor. | yes | Constructor becomes `pub` (this crate's own boundary, not `main.rs`'s); still the only mint site. |
| `lifecycle.rs:189-220` (`Task<S>`, `Task<Intake>` impls) | The struct + `new`/`specify`/`refuse`. | yes | `Refusal` → `GateRefusal`. |
| `lifecycle.rs:222-238` (`Task<Specified>` impls) | `review`/`refuse`. | yes | Same. |
| `lifecycle.rs:240-264` (`edge!` macro + 11 invocations) | The straight-through edges. | yes, verbatim | Same. |
| `lifecycle.rs:266-284` (`refusal_edge!` macro + 5 invocations) | The `refuse` edges for mid-pipeline states. | yes, verbatim | Same. |
| `lifecycle.rs:286-294` (`Task<Attested>::accept`) | Human-gated acceptance. | yes | Same. |
| `lifecycle.rs:296-514` (`ChangeEmitter`, `ProposalRequest`, `ProposedChange`, `AttestationBundle` + 3 completeness fns + `is_valid_artifact_id`, `Task<Accepted>::propose`) | The propose edge and everything it needs to decide, structurally, whether a PR may open. | yes | `Refusal` → `GateRefusal` throughout; nothing else changes — every function here is already pure (reads only its own arguments, does `blake3::hash` on in-memory bytes, no IO). |
| `lifecycle.rs:516-549` (`impl<S: State> Task<S>` — `id`, `retry_depth`, private `transition`) | Shared accessors + the one real state-mutation. | yes, verbatim | `Refusal` → `GateRefusal`. |
| `lifecycle.rs:551-668` (`state_dir`, `safe_task_name`, `task_path`, `FileLedger`, `json_escape`, `short_state`, `io_refusal`, `load_state`, `persist_state`) | Filesystem persistence: env var read, path construction, atomic write-then-rename, a hand-rolled JSON-lines append. | **no** | All IO — moves to the caller. `fleet-store` is the natural owner of a `ReceiptLedger` impl; `src/` or `fleet-store` owns `load_state`/`persist_state` (the "what state is this task in, right now" store) since neither is specific to the typed machine, only to one way of remembering its last-known state string. |
| `lifecycle.rs:670-797` (`AgentMilestone`, `project_agent_task`, `synthetic_proposal`, `NoopEmitter`, `refuse_agent_task`) | Drives a task through the *typed* edges to a milestone AND persists state at each stopping point via `FileLedger`. | **no** | Mixed pure-drive + IO-persist in one function; the pure "walk these edges in order" shape is exactly what a caller can now do directly with this crate's public edges — `project_agent_task`'s *body* (the sequence of `.specify()/.review()/...` calls) is a worked usage example for `src/`'s own swarm-dispatch code, not something this crate re-exports, since the `persist_state` calls interleaved through it are IO. |
| `lifecycle.rs:813-923` (`canonical_next`, `typed_advance`) | String-state → single canonical next edge, dispatching into `Task::<$state> { id, retry_depth: 0, _s: PhantomData }` built via the private-field literal (legal only because this code lives in the same module as `Task`). | **partially — becomes `resume`/`advance_any`** | The private-field construction trick cannot move to the caller (fields are private to this crate by design). This crate instead exposes the sanctioned `resume`/`advance_any` pair (§3) that do the same job through a public, crate-internal-only constructor path. This is a REAL shape change, not a verbatim lift — flagged in the divergence note. |
| `lifecycle.rs:925-1149` (`append_runtime_receipt`, `refuse_runtime`, `advance_to`, `parse_task`, `command`, `report_environment`, `drive_run`) | CLI parsing, stdout/stderr printing, exit-code mapping, `FileLedger` construction from `$FLEET_STATE`. | **no** | Entirely IO/CLI — `src/`'s composition root owns the `fleet lifecycle` subcommand and `drive_run`'s equivalent (a real run wrapped start-to-`Accepted`). |
| `lifecycle.rs:1151-1183` (`propose_change`) | Loads persisted state, refuses unless `"Accepted"`, builds `Task::<Accepted>` via the private-field trick, calls `propose`, persists `"Proposed"`. | **no — replaced by `resume` + direct `propose` call** | `src/`/`fleet-store` loads the string, calls this crate's `resume(state, id, retry_depth)?`, pattern-matches `AnyTask::Accepted(task)` (refusing with a caller-defined "not accepted" error otherwise — this crate's `resume` only validates that the STRING is one of the 16 names, not that it's the specific one the caller wanted), then calls `task.propose(...)` directly and persists the result itself. |
| `lifecycle.rs:1185-1478` (`#[cfg(test)] mod tests`) | 8 unit tests: attestation completeness, pinned element set, propose pass/refuse, full legal path (15 receipts), empty-evidence refusal, persisted-across-invocations, illegal-runtime-refusal. | as inspiration, split | The 6 tests that exercise only typed edges/`AttestationBundle`/`propose` (not `FileLedger`/`advance_to`) port directly into this crate's test plan (§9); the 2 that touch the filesystem (`persisted_task_advances_across_invocations`, `illegal_runtime_steps_refuse_in_both_directions`) are the caller's tests once `load_state`/`persist_state`/`advance_to` move there. |
| `tests/compile_fail/*.rs` + `.stderr` (6 cases) | The compile-time guarantees: no forged construction, no skipped gates, no non-human review, no reuse-after-move, no `Accepted → Observed` skip-PR edge, no `merge` edge. | yes, verbatim | Move into `crates/fleet-lifecycle/tests/compile_fail/`; update every `use fleet::lifecycle::{...}` import to `use fleet_lifecycle::{...}`; `.stderr` goldens need their `--> tests/compile_fail/...rs` paths and (for `non_human_review.stderr`) the `--> src/lifecycle.rs` note-location updated to this crate's actual file (`src/edges_specified.rs` per §8) — trybuild re-generates these on first `TRYBUILD=overwrite cargo test` run; do not hand-edit blindly, regenerate and diff-review. |

## 6. Behavior spec

### `fn Task::<Intake>::specify(self, evidence, ledger) -> Result<Task<Specified>, GateRefusal>`
*(representative of every straight-through edge — `decompose`, `contract`, `brief`, `lease`, `build`, `finish_build`, `begin_verification`, `verify`, `attest`, `observe`, `reopen` all share this exact shape)*

| Input dimension | Behavior |
|---|---|
| empty | `evidence` is `""` or all-whitespace → `Err(GateRefusal{code: "EMPTY_TRANSITION_EVIDENCE", ..})`; `self` is NOT consumed into a new state — the caller still owns the old `Task<Intake>`... actually it IS consumed (moved into `transition`), but no `Task<Specified>` is produced and no receipt is appended (verified by `empty_evidence_refuses_without_advancing_or_writing`). |
| null / `None` | n/a — `evidence: impl Into<String>` has no null; a caller passing `String::new()` is the empty case above. |
| wrong-type | n/a — the type parameter fixes `self`'s state at compile time; there is no runtime type to get wrong. |
| huge | `evidence` at 100x–1000x expected length (a full SOW, hundreds of KB): no length cap in this crate — `transition` stores/forwards the string as-is; the caller's `ReceiptLedger` impl decides whether to cap it (this crate does not silently truncate evidence, which would make a receipt lie about what happened). |
| negative | n/a — no numeric input. |
| duplicate | Calling `.specify(...)` twice on the same binding does not compile (`E0382`, moved value) — this is the `reuse_consumed` compile-fail guarantee, not a runtime "duplicate" case. |
| concurrent | `Task<S>` is not `Sync`-shared across threads by any API here (transitions consume `self` by value) — no data race is representable; see §11. |
| unicode / non-ASCII | `evidence` is stored as an owned `String`; any valid UTF-8 (including non-ASCII) passes the non-empty check and is forwarded to the ledger verbatim — no normalization, no panic. |
| already-exists | n/a — a transition does not "create" an id; the same `TaskId` can legally pass through `Intake` again after `Observed::reopen` (the one designed cycle), and nothing here rejects a reused id. |
| partial-failure | `ledger.append(...)` returns `Err` → `transition` returns that `Err` immediately; the caller's `Task<Specified>` is never constructed, so there is no half-transitioned task to leak — either the receipt is appended AND the new task is returned, or neither happens (verified by `incomplete_bundle_refuses_before_the_emitter_runs`'s sibling assertion style, applied here as `ledger_append_failure_yields_no_new_task`, §9). |

### `fn Task::<Specified>::review(self, approval: HumanApproval, ledger) -> Result<Task<Reviewed>, GateRefusal>`
*(and `Task<Attested>::accept`, the other human-gated edge, shares this shape)*

| Input dimension | Behavior |
|---|---|
| empty | `approval`'s inner evidence being empty is unrepresentable here — `HumanApproval::recorded` already refused it at construction; `review` itself has no separate emptiness case beyond the shared `EMPTY_TRANSITION_EVIDENCE` check on `approval.evidence` inside `transition`. |
| null / `None` | n/a — `approval: HumanApproval` is not optional; a caller with no approval cannot call this method at all (that's the whole point — see `non_human_review`). |
| wrong-type | Passing a bare `&str`/`String` where `HumanApproval` is expected is `E0308` at compile time (`non_human_review` compile-fail test) — not a runtime case. |
| huge | Same as `specify` — no cap on the approval's internal evidence length. |
| negative | n/a. |
| duplicate | Same move-semantics as `specify`. |
| concurrent | Same as `specify`. |
| unicode / non-ASCII | Same as `specify`. |
| already-exists | n/a. |
| partial-failure | Same as `specify` — ledger failure yields no `Task<Reviewed>`. |

### `fn AttestationBundle::missing_element(&self) -> Option<&'static str>`

| Input dimension | Behavior |
|---|---|
| empty | `elements: json!({})` (empty object) → `object.get("sow")` is `None` for every key → returns `Some("sow")` (the first `REQUIRED` entry), never panics on a missing key. |
| null / `None` | `elements: Value::Null` → `.as_object()` is `None` → every key is `complete = false` → `Some("sow")`, same as empty. |
| wrong-type | `elements: json!([1,2,3])` (an array, not an object) → same as null: `.as_object()` is `None` → `Some("sow")`. `elements.oracle_independence: json!("not an object")` → `oracle_independence_is_complete` returns `false` via its own `.as_object()` guard → reported as missing, not a panic. |
| huge | `elements` with hundreds of extra unknown keys alongside the 8 required ones: extra keys are ignored (`REQUIRED.iter()` only ever looks up the 8 known names); no unbounded scan, cost is `O(8)` object lookups regardless of how large `elements` is. |
| negative | n/a — no numeric input; `oracle_independence_is_complete`'s `quadrant` check is a fixed 4-string set membership test, not numeric. |
| duplicate | n/a — `elements` is a single JSON object; JSON objects cannot have duplicate keys once parsed (the last one wins per `serde_json` parsing rules, which happens upstream of this crate). |
| concurrent | `&self`, no interior mutability — safe to call from multiple threads on a shared `&AttestationBundle`. |
| unicode / non-ASCII | String fields (`builder`, `verifier`, `o1_author`, …) compared/read via `Value::as_str`; any valid UTF-8 string is accepted as-is — no normalization performed, matching `decide`'s documented limit in the router crate. |
| already-exists | n/a — pure read, no state created. |
| partial-failure | n/a — no IO, cannot fail partially; always returns synchronously. |

### `fn Task::<Accepted>::propose(self, bundle, request, emitter, ledger) -> Result<(Task<Proposed>, ProposedChange), GateRefusal>`

| Input dimension | Behavior |
|---|---|
| empty | `request.diff` empty (`vec![]`) → `Err(GateRefusal{code: "EMPTY_DIFF", ..})`, checked AFTER the digest check (an empty diff's blake3 hash is a real, matchable value, so digest-match can pass on an empty diff — the emptiness check exists precisely to catch that case) and BEFORE `emitter.emit` runs. |
| null / `None` | n/a — no `Option` params; `bundle`/`request`/`emitter`/`ledger` are all required references. |
| wrong-type | n/a — see `AttestationBundle::missing_element`'s row above for how a malformed `elements` value inside `bundle` is handled (it surfaces as a named-missing-element refusal, never a panic, before `propose` even reaches the digest check). |
| huge | `request.diff` at MB-scale: `blake3::hash` is streaming/O(n) with no crate-imposed cap; no truncation. |
| negative | n/a. |
| duplicate | Calling `propose` twice on the same `Task<Accepted>` binding is a compile error (moved value), same as every other edge. Two DIFFERENT `Task<Accepted>` values (different `TaskId`s) both proposing with the same `request.head` branch name is not this crate's concern — `emitter.emit`'s real implementation (in `src/`) is where a duplicate-branch conflict would surface, as a `GateRefusal` the emitter itself constructs. |
| concurrent | `self` consumed by value; `bundle`/`request` are shared `&`-refs with no interior mutability — safe to call concurrently with different `Task<Accepted>` values sharing the same `bundle`/`request` data if a caller wanted to (unusual, but not unsound). |
| unicode / non-ASCII | `request.title`/`.body` pass through to `emitter.emit` untouched; no normalization or length limit imposed here (a real `ChangeEmitter` talking to a real forge API decides its own limits). |
| already-exists | n/a — "already proposed" is not a state this fn can observe; it consumes `Task<Accepted>`, which by construction has not yet proposed. |
| partial-failure | The 3-stage gate (completeness → digest → emptiness) runs entirely before `emitter.emit`; if `emitter.emit` itself fails (`Err`), `propose` returns that `Err` and does NOT append a receipt (verified by `incomplete_bundle_refuses_before_the_emitter_runs`'s pattern, extended to an `emitter_failure_leaves_no_receipt` test, §9) — there is no path where a receipt is appended but the emitter never ran, or the emitter ran but `propose` reports failure. |

### `fn resume(state: &str, id: TaskId, retry_depth: u32) -> Result<AnyTask, GateRefusal>`

| Input dimension | Behavior |
|---|---|
| empty | `state = ""` → `Err(GateRefusal{code: "UNKNOWN_LIFECYCLE_STATE", ..})` — not one of the 16 wire names. |
| null / `None` | n/a — `state: &str` is not optional. |
| wrong-type | `state = "Verifiedd"` (typo) or any string not exactly matching one of the 16 names (case-sensitive, matching `lifecycle.rs`'s `std::any::type_name`-derived short names) → same `UNKNOWN_LIFECYCLE_STATE` refusal, naming the string it was given, so the caller can tell a typo from a genuinely corrupted store. |
| huge | n/a — `state` is compared against 16 fixed literals; cost is O(16) regardless of `state`'s length (a pathologically long garbage string just fails every comparison and falls through to the refusal). |
| negative | `retry_depth` is `u32` — cannot be negative; a caller cannot construct a negative retry count. |
| duplicate | Calling `resume` twice with the same `(state, id.clone(), retry_depth)` is not an error — it is required to be idempotent (pure function, no shared state mutated); each call returns an independent `AnyTask` value. |
| concurrent | Pure function of its arguments — trivially safe from any number of threads. |
| unicode / non-ASCII | `state` is compared with byte/UTF-8 `==` against the 16 ASCII literal names; any non-ASCII string simply never matches and falls to the refusal — no panic. |
| already-exists | n/a — `resume` does not check whether this `id` "already exists" anywhere; it has no store to check against (that's exactly why it's pure). |
| partial-failure | n/a — no IO, cannot fail partially. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path (`{ path = "../fleet-types" }`) | Supplies `GateRefusal` (this crate's sole error type) and, for the divergence-note discussion only, `LifecycleState`. |
| `serde_json` | `1.0.151` (matches `fleet/keel/fleet/Cargo.toml`'s existing pin) | `AttestationBundle.elements: Value` and the 3 completeness-check helpers read/compare JSON values exactly as `lifecycle.rs` does today. |
| `blake3` | `1` (matches fleet's existing pin) | `propose`'s digest check (`blake3::hash(&request.diff)`); the same crate `safe_task_name` used, but that function itself does not move here (§5). |

`[dev-dependencies]`

| Crate | Version | Why |
|---|---|---|
| `trybuild` | `1` (matches fleet's existing pin) | Runs the 6 compile-fail cases moved from `fleet/keel/fleet/tests/compile_fail/`. |

No `serde` derive is needed on any type in this crate's public surface (unlike `fleet-router`'s
`Decision`/`Stage`) — nothing here is serialized directly; the caller wraps `TransitionReceipt`/
`ProposedChange` fields into its own wire types (`fleet-types::Receipt`) when it persists them.

## 8. Crate file layout

```
crates/fleet-lifecycle/
  Cargo.toml
  src/
    lib.rs                 # ~36 — module decls + re-exports only
    states.rs              # ~61 — sealed::Sealed, State, 16 marker structs, seal_states! invocation
    task_id.rs             # ~29 — TaskId, Display
    receipt.rs             # ~23 — TransitionReceipt, ReceiptLedger
    human_approval.rs      # ~26 — HumanApproval
    task.rs                # ~49 — Task<S> struct + impl<S: State> (id, retry_depth, private transition)
    edges_intake.rs        # ~31 — Task<Intake>: new, specify, refuse
    edges_specified.rs     # ~28 — Task<Specified>: review, refuse
    edges_straight.rs      # ~36 — edge! macro + its 11 invocations
    edges_refusal.rs       # ~27 — refusal_edge! macro + its 5 invocations
    edges_attested.rs      # ~18 — Task<Attested>::accept
    proposal_types.rs      # ~36 — ChangeEmitter, ProposalRequest, ProposedChange
    attestation.rs         # ~58 — AttestationBundle, REQUIRED, missing_element
    attestation_checks.rs  # ~51 — independent_verification/oracle_independence/blind_suite _is_complete, is_valid_artifact_id
    edges_propose.rs       # ~51 — Task<Accepted>::propose
    resume.rs              # ~68 — AnyTask, resume()
    advance.rs             # ~54 — advance_any() (mirrors canonical_next/typed_advance, pure)
  tests/
    compile_fail.rs                # ~5  — trybuild harness (mirrors lifecycle.rs's own tests/compile_fail.rs)
    compile_fail/
      accepted_skips_pr.rs / .stderr
      construct_verified.rs / .stderr
      intake_to_verified.rs / .stderr
      merge_is_not_a_lifecycle_edge.rs / .stderr
      non_human_review.rs / .stderr
      reuse_consumed.rs / .stderr
    common/
      mod.rs                # ~11 — re-exports of doubles/fixtures/variant for `mod common;` in each test binary
      doubles.rs             # ~49 — MemoryLedger, FailingLedger, RecordingEmitter, FailingEmitter
      fixtures.rs             # ~66 — complete_elements(), sample_request(), accepted_task()
      variant.rs               # ~25 — variant_name(&AnyTask) -> &'static str, for resume/advance assertions
    happy_path.rs            # ~55 — full Intake..Observed walk, receipt count, plus the empty-evidence and ledger-failure cases
    propose_gates.rs          # ~41 — attestation-completeness gate: happy path + incomplete-bundle refusal
    propose_gates_digest.rs    # ~55 — digest-mismatch / empty-diff gates, plus a failed emitter leaving no receipt
    attestation_pinning.rs      # ~32 — pinned REQUIRED set + missing_element's own basic behavior
    attestation_deep_checks.rs   # ~46 — independent_verification/blind_suite/oracle_independence deep-shape + artifact-hash checks
    resume_bridge.rs              # ~29 — resume() over all 16 states, plus the unknown-name refusal
    advance_bridge.rs              # ~43 — advance_any() over all 14 non-terminal edges, Accepted's PR_EMIT_REQUIRES_EVIDENCE, Refused's dead end
    resume_retry_depth.rs           # ~12 — resume() carries retry_depth through untouched
    task_id_display.rs               # ~11 — TaskId's Display writes the raw id verbatim
```
Every `src/` file above is a straight split of `lifecycle.rs`'s already-short blocks (§5's line-range
column); none requires new logic to fit under 80 lines except `resume.rs`/`advance.rs`, which are new
(§ divergence note) and sized to match. Unlike the original plan, no test lives as a co-located
`#[cfg(test)]` module inside `src/` — every test, including what §9 calls "unit tests", is an
integration test under `tests/` driving the crate through its public API only, sharing one `tests/
common/` fixture module (see §9).

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-lifecycle"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
serde_json = "1.0.151"
blake3 = "1"

[dev-dependencies]
trybuild = "1"
```

## 9. Test plan

There is no co-located `#[cfg(test)]` module anywhere in `src/` — every test, including the cases
below that started life as `lifecycle.rs`'s unit tests, is an integration test under `tests/` that
drives the crate through its public API only, via a shared `tests/common/` fixture module
(`doubles.rs`: `MemoryLedger`, `FailingLedger`, `RecordingEmitter`, `FailingEmitter`; `fixtures.rs`:
`complete_elements()`, `sample_request()`, `accepted_task()`; `variant.rs`: `variant_name(&AnyTask)`).

**Ported-as-integration tests** (public-API-only, ported from `lifecycle.rs:1185-1478` where noted):
- `happy_path.rs::empty_evidence_refuses_without_advancing_or_writing` — ported: `Task::new(...).specify("  ", &ledger)` is `Err(EMPTY_TRANSITION_EVIDENCE)` and the `MemoryLedger` stays empty.
- `happy_path.rs::ledger_append_failure_yields_no_new_task` — a `FailingLedger` whose `append` always returns `Err` makes `specify` return that `Err` (`LEDGER_DOWN`); no `Task<Specified>` is ever produced.
- `attestation_pinning.rs::missing_element_names_the_first_incomplete_or_absent_element` — ported (complete/pending-oracle-independence/missing-adequacy cases).
- `attestation_pinning.rs::pinned_element_names_match_the_enforced_set` — ported, asserts `AttestationBundle::REQUIRED`'s exact 8-element array and order.
- `propose_gates_digest.rs::emitter_failure_leaves_no_receipt` — a `FailingEmitter` whose `emit` returns `Err(EMIT_DOWN)`; asserts `propose` returns that `Err` and the ledger's receipt count is unchanged from before the call.

**Other integration tests** (`tests/`, calling only the public API):
- `happy_path.rs::legal_path_consumes_each_state_and_records_every_edge` — full `Intake..Observed` walk (15 receipts, final `task.id()` check) plus asserting `retry_depth()` is unchanged across every edge (no edge silently bumps it).
- `propose_gates.rs::complete_bundle_advances_to_proposed_and_records_one_receipt` — happy path through `propose`, asserts exactly one emitter call and one appended receipt.
- `propose_gates.rs::incomplete_bundle_refuses_before_the_emitter_runs` — removing `adequacy` from the elements refuses with `INCOMPLETE_ATTESTATION` before the emitter runs.
- `propose_gates_digest.rs::digest_mismatch_refuses_before_the_emitter_runs` — same shape as the incomplete-bundle test but with `request.artifact_id` set to a hash that does not match `request.diff`; asserts `ARTIFACT_DIGEST_MISMATCH` and an empty `RecordingEmitter`.
- `propose_gates_digest.rs::empty_diff_refuses_after_digest_check_passes` — `request.diff = vec![]` with `artifact_id` correctly set to `blake3::hash(&[]).to_hex()` (so the digest check passes), asserts `EMPTY_DIFF` and an empty emitter — this is the one case that specifically proves emptiness is checked, not just digest.
- `attestation_deep_checks.rs` — 5 cases: a bad field inside `independent_verification`/`blind_suite`/`oracle_independence` each names that element missing (not accepted just because the key is non-null), plus a non-hex and an uppercase `o1_hash` are both rejected by `is_valid_artifact_id`.
- `resume_bridge.rs::resume_reconstructs_every_one_of_the_16_states` — for each of the 16 wire names, `resume(name, id.clone(), 0)` returns the matching `AnyTask` variant (checked via `variant_name`, not just `is_ok()`).
- `resume_bridge.rs::resume_refuses_an_unknown_state_name` — `resume("Bogus", id, 0)` is `Err(UNKNOWN_LIFECYCLE_STATE)`.
- `resume_retry_depth.rs::resumed_task_reports_the_supplied_retry_depth` — `resume("Built", id, 7)` round-trips `retry_depth() == 7` (guards against a stubbed getter that always reports `0`).
- `task_id_display.rs::display_writes_the_raw_id` — `TaskId`'s `Display` writes the id verbatim.
- `advance_bridge.rs::advance_any_walks_the_canonical_edge_from_every_resumable_state` — for each of the 14 non-`Accepted`/non-`Refused` states, `advance_any` on the `resume`d task lands on the documented next state (mirrors `lifecycle.rs`'s `canonical_next` table).
- `advance_bridge.rs::advance_any_refuses_accepted_naming_the_real_entry_point` — `Accepted` has no canonical next: `Err(PR_EMIT_REQUIRES_EVIDENCE)`.
- `advance_bridge.rs::advance_any_refuses_from_refused` — `Refused` has no forward edge: `Err(ILLEGAL_LIFECYCLE_TRANSITION)`.

**Compile-fail tests** (`tests/compile_fail.rs` + `tests/compile_fail/*.rs`, via `trybuild`):
- All 6 cases moved verbatim from `fleet/keel/fleet/tests/compile_fail/`, imports updated to
  `fleet_lifecycle::{...}`. `.stderr` goldens regenerated with `TRYBUILD=overwrite cargo test
  --test compile_fail -p fleet-lifecycle` and then diffed by hand against the originals — only the
  file path and (for `non_human_review`) the `note: method defined here --> src/...` location are
  expected to differ; any other diff is a real regression in the ported code.

**Mutation-testing targets** (`cargo mutants -p fleet-lifecycle`):
- Deleting the `if evidence.trim().is_empty() { return Err(...) }` guard inside `transition` must be
  killed by `empty_evidence_refuses_without_advancing_or_writing`.
- Reordering `propose`'s 3 gates (e.g. checking `diff.is_empty()` before the digest) must be killed
  by `empty_diff_refuses_after_digest_check_passes` (which specifically constructs a digest-correct
  empty diff — a reordering that checked emptiness first would still refuse, but for the wrong
  reason at the wrong gate; assert the refusal `code` is exactly `EMPTY_DIFF`, not just "some Err").
- Moving `emitter.emit(request)?` to before the 3 gates must be killed by
  `incomplete_bundle_refuses_before_the_emitter_runs`'s `emitter.0.borrow().is_empty()` assertion.
- Flipping `digest != request.artifact_id` to `==` must be killed by
  `digest_mismatch_refuses_before_the_emitter_runs`.
- Changing any `edge!`/`refusal_edge!` macro invocation's `$to` (e.g. wiring `Building::finish_build`
  to `Verified` instead of `Built`) must be killed by `legal_path_consumes_each_state_and_records_every_edge`'s exact state sequence.

**Property tests:** not applicable — the state graph is a small, fixed, exhaustively-enumerable set
(16 states, ≤2 outgoing edges each); exhaustive unit/integration coverage (above) is a better fit than
a generator, and `resume_reconstructs_every_one_of_the_16_states` already IS the exhaustive case a
property test would otherwise generate.

## 10. Verification recipe

```bash
cd crates/fleet-lifecycle
cargo test -p fleet-lifecycle --all-targets
cargo clippy -p fleet-lifecycle --all-targets -- -D warnings
cargo mutants -p fleet-lifecycle
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
grep -rn 'std::fs\|std::env::var\|SystemTime::now\|Command::new' src/ | grep -v '^src/edges_propose.rs:.*blake3' # must print nothing (no IO leaked back into this crate)
```
Expected: all integration tests (22 `#[test]` functions across the 9 files listed in §9, none
co-located in `src/`) plus the compile-fail harness pass, 0 skipped — publish `<passed>/<total>`
(the compile-fail harness counts as 1 test regardless of its 6 cases; publish those 6 as a named
sub-count too, e.g. "integration 22/22, compile-fail 6/6 cases, trybuild harness 1/1").
Clippy: 0 warnings. Mutants: every target named in §9 caught — floor is **100% of viable mutants
caught** (this crate is small and pure); publish `<caught>/<total>`, never a percentage alone. The
final `grep` must return nothing — this crate must remain provably IO-free, not just IO-free by
convention.

## 11. L8 checklist

- [ ] Every fallible path returns `fleet_types::GateRefusal` — none swallowed into `bool`/`Option`/
      `String`/`.unwrap()` in non-test code.
- [ ] Clock/RNG/IO are absent, not merely injected — this crate has none of the three; verified by
      §10's final `grep` gate.
- [ ] Thread-safety documented: `Task<S>` has no interior mutability and every transition consumes
      `self` by value, returning an owned `Task<N>` — `Send`/`Sync` for free (no `Rc`/`RefCell`
      anywhere in the public types); safe to hold different `Task<S>` values on different threads
      simultaneously, and trivially safe to share `&AttestationBundle`/`&ProposalRequest` (both
      read-only) across threads.
- [ ] No float used anywhere in this crate (`retry_depth: u32`, all else strings/enums/`Value`).
- [ ] No self-grading: verification runs `cargo mutants`, not just the crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator (`x/y` per suite) is stated in this file and
      restated in the PR — not just "green".
- [ ] No test touches the filesystem — this crate has no filesystem access to test against; if a
      future test needs one, it must use `tempdir()`, never the repo tree (this crate should never
      need this rule in practice, since it is pure by design).
- [ ] Every non-goal in §2 is actually absent from the code: no `fs::`/`std::io`/`env::var`/
      `Command::new`/`SystemTime::now` anywhere in `crates/fleet-lifecycle/src/` — enforced by §10's
      grep gate, not just this checkbox.
- [ ] **No source file exceeds 80 lines** (verified: `find src tests -name '*.rs' | xargs wc -l` —
      every file ≤ 80, per §8's table). `lib.rs` is a thin hub, not a dumping ground.

## 12. Definition of Done

`fleet-lifecycle` is DONE when: §10's five commands all pass with a published denominator (unit +
integration + compile-fail tests green with the sub-counts stated above, clippy clean, mutants
`M/M` caught, the two grep gates print nothing) run from `crates/fleet-lifecycle/`; every unchecked
box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` lists the crate; the 6
compile-fail `.stderr` goldens are reviewed (not just regenerated and trusted) against the originals
in `fleet/keel/fleet/tests/compile_fail/`; and Opus has independently re-derived the 16-state edge
graph from this blueprint alone (without re-reading `lifecycle.rs`), reproduced the propose-gate-
ordering mutation by hand, and driven one real `resume` → `advance_any` walk end-to-end from a bare
state string, confirming it lands on the same state `lifecycle.rs`'s `canonical_next` would have
produced.

---

## Divergence from MIGRATION-PLAN (for Opus)

MIGRATION-PLAN §3 row 15 and §7's teach-back entry describe `fleet-lifecycle` as lifting "the type-
state `Task<S>` machine … pure — no persistence" plus the compile-fail suite. Having read the full
1478-line file, three things need an explicit decision:

1. **`Refusal` is not re-declared here — this crate uses `fleet_types::GateRefusal`.** The MIGRATION-
   PLAN teach-back entry says "`fleet-types` keeps only `LifecycleState` vocabulary," which reads as
   if `lifecycle.rs`'s own `Refusal` struct (`lifecycle.rs:134-163`) travels with the machine into
   this crate. But `fleet-types::BLUEPRINT.md` §3 already defines `GateRefusal` as "the general
   `{code, message}` shape every other refusal site uses," lifted from this exact struct, and
   explicitly disclaims owning the `Task<S>` machine itself. Re-declaring a second identical
   `{code: &'static str, message: String}` type in this crate would defeat the point of `fleet-
   types` existing — every crate's refusal would need its own conversion layer. This blueprint
   resolves it by depending on `fleet_types::GateRefusal` directly and NOT re-declaring `Refusal`.
   **MIGRATION-PLAN §3 row 15 should say this explicitly** — right now a reader could go either way.

2. **The private-field constructor trick (`typed_advance`, `propose_change`) cannot literally move to
   the caller, because `Task<S>`'s fields are private to this crate by design.** MIGRATION-PLAN's
   framing ("pure — no persistence") correctly identifies that persistence must move out, but doesn't
   address that `typed_advance`/`propose_change` were ALSO the only sanctioned way for
   string-driven code (the CLI's `advance`/`show`, and `fleet pr emit`) to get from "a state name
   loaded from disk" back into a real `Task<S>`. If that bridging code moves to the caller wholesale
   (as "IO, therefore not here" would naively suggest), it either (a) needs `Task<S>`'s fields made
   `pub(crate)`-visible somehow — breaking the exact guarantee `construct_verified`'s compile-fail
   test protects — or (b) has nowhere to live. This blueprint's answer is `resume`/`AnyTask`/
   `advance_any` (§3): a small, NEW, pure public API that stays inside this crate (because only this
   crate may touch the private fields) and gives the caller a sanctioned, auditable re-entry point.
   This is genuinely new surface area beyond "port `lifecycle.rs` verbatim" — Opus should confirm
   `AnyTask`'s 16-variant enum shape (rather than, say, a `Box<dyn Any>` or a different erasure
   scheme) is the right call before implementation starts, since it's the one design decision in this
   blueprint that isn't a straight lift.

3. **`STATES` (the string-keyed edge-legality table, `lifecycle.rs:16-33`) does not travel with this
   crate**, even though it looks like "the machine's data." It is used ONLY by the runtime CLI's
   `advance_to` to pre-validate a requested string→string edge before dispatching into
   `typed_advance` — a duplicate of the SAME information `fleet_types::LifecycleState::allowed_next`
   already commits to owning (per `fleet-types` BLUEPRINT.md §3's own doc comment: "Whichever crate
   ends up owning `Task<S>` must keep its own marker-type transitions in lockstep with this table").
   This blueprint's position: the compiled `impl Task<$From>` blocks in THIS crate are the actual
   source of truth for legality (a caller literally cannot call an illegal edge — it won't compile),
   so the caller-side runtime check should validate against `fleet_types::LifecycleState::
   allowed_next` (data, cheap to check before even trying to `resume`), not a second string table
   copied into this crate too. The "keep two tables in lockstep" risk `fleet-types` flagged is real,
   but it's a lockstep between `fleet-types::LifecycleState::allowed_next` and THIS crate's compiled
   edge set (§9's `resume_bridge.rs` tests are effectively that lockstep check, run every `cargo
   test`) — not a third copy. Worth a line in MIGRATION-PLAN §7 once built, if the lockstep test
   catches real drift.
