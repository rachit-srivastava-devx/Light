# BLUEPRINT — `fleet-plan`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-plan`
- **One-line purpose:** Own the spec-production pipeline's *decisions* — the intake ambiguity
  rubric and stage-artifact validation, LLD draft assembly, the LLD-readiness depth gate, the
  review-verdict decision, and the teach-back step that turns one settled outcome into a
  corpus-citable lesson row — as pure, typed, zero-IO functions over already-in-memory data.
- **Build branch:** `refactor` (MIGRATION-PLAN §3 row 6: `intake.sh` · `lld.rs` · `lld_ready.rs` ·
  `review.sh` — "unify + add teach"). Per-piece detail: `lld.rs`/`lld_ready.rs` are **extract**
  (already pure, already zero-IO, lift near-verbatim — see §5); `intake.sh`'s validators and
  `review.sh`'s decision predicates are **refactor** (the logic is pure today but interleaved with
  bash file-locking/state-directory/subprocess IO that does not come along — see §5's per-row
  "change needed" column); the **teach** step is **build-new** (no such assembly exists in fleet
  today — see §5's greenfield row).
- **Imports:** `fleet-types` (`Role`, `TaskId`, `NodeId`, `LifecycleState`, `GateRefusal`,
  `ExitCode`). **None yet** from `fleet-lifecycle`/`fleet-store` — see the divergence note at the
  end of this file: this blueprint's read of §2's non-goals is that fleet-plan *names* a
  lifecycle/ledger fact (which state a decision implies, what a receipt event should be called)
  without importing the crates that enforce/persist it, so today's edge list is `fleet-types` only.
  If Opus decides the composition root needs fleet-plan to *construct* a `fleet_types::Receipt`
  value directly (rather than the caller assembling one from fleet-plan's plain output), that stays
  within `fleet-types` and does not add an edge.
- **Imported by:** `src/` (fleet-cli, the composition root — wires state-directory IO, subprocess
  calls to worker adapters, ledger appends, and lifecycle transitions around this crate's pure
  decisions). No sibling crate is a declared caller today; **flagged as a divergence** if
  `fleet-worker` later needs `validate_review_contract` before dispatching a review task itself
  (mirrors `fleet-types`'s own flagged `Role`-check-promotion trigger) — not added preemptively.

## 2. Responsibility & non-goals

**Owns:** every *decision* in the spec-production pipeline, expressed as a pure function from
already-parsed input to a typed result — nothing here reads a file, acquires a lock, spawns a
process, or appends to a ledger. Concretely, five stages:

1. **Intake** — deriving the fixed-plus-triggered open-question rubric from an intent string
   (`intake.sh`'s `write_questions`/`emit_core`/`emit_derived`), which questions remain open given
   which have been answered, and the structural validity of the four intake artifacts (SOW,
   atomic decomposition, challenge register, clarification register) — each a pure parse+shape
   check over text/rows already read into memory.
2. **LLD draft** — assembling the four accepted intake artifacts into the blueprint document body
   and its 4-check acceptance-checks draft (`intake.sh`'s `blueprint()`/`summary()`, text assembly
   only — no file writes).
3. **LLD-ready gate** — the `module-brief.v1`/`freeze.v1`/`lld.v1` shape validator plus the
   14-check depth-readiness gate (`lld.rs`/`lld_ready.rs`, already exactly this shape today —
   lifted with the smallest possible change).
4. **Review** — given a worker/reviewer role pair's declared capabilities, a task's lifecycle
   state, and a retry-attempt count, deciding: is this role pairing authorized, is a submission
   eligible, and what does an accept/revise/reject verdict actually resolve to (which state it
   implies, whether the retry ceiling forces an escalation instead) — the decision half of
   `review.sh`'s `validate_review_contract`/`submit`/`verdict`, stripped of every subprocess call
   and state mutation those functions also perform today.
5. **Teach** — turning one settled, already-decided outcome (an accept, a reject, an escalation, or
   a caught defect) into the same `{id, source, affected_leaf, risk, trigger, mitigation}` row
   shape `challenges.tsv` and `FAILURE-CORPUS.md` already use as a *citable* corpus entry — closing
   the loop `intake.sh`'s `challenge_source_exists` only ever reads from, never writes to.

**Non-goals (the seam):**
- Does **not** touch the filesystem, acquire a lock, or manage a state directory
  (`intake.sh`'s `STATE`, `acquire_lock`, `atomic_write`, the `answers/*.txt` layout) — that is the
  composition root's (`src/`) job, most likely backed by `fleet-store`.
- Does **not** append a ledger receipt (`intake.sh`'s `record_stage`/`receipt_append`,
  `review.sh`'s `append_review_receipt`) — `fleet-store` owns the hash-chained ledger; this crate
  at most returns the `ReceiptEvent` name and JSON body a caller should append, as a plain value.
- Does **not** perform a lifecycle state transition (`review.sh`'s `transition()` calling
  `statemachine.sh`, or `lifecycle.rs`'s `Task<S>` methods) — `fleet-lifecycle` (MIGRATION-PLAN row
  15) owns the type-state machine; this crate only *reports* which `fleet_types::LifecycleState` a
  decision implies, and never enforces that a transition is legal.
- Does **not** run a subprocess of any kind — not `node --input-type=module` to read
  `RETRY_POLICY.maxAttemptsPerTask` (`review.sh:27-35`), not `roles.sh show ROLE` to fetch a role's
  declared JSON entity (`review.sh:37-43`), not `ratchet.sh check`/`accept` to run the mutation
  floor (`review.sh:142,146`). Every one of these becomes a plain value this crate's functions take
  as a parameter — the composition root or `fleet-worker` is who actually shells out, per this
  repo's "call through fleet-worker, don't spawn" rule.
- Does **not** decide *which* agent gets dispatched, what its token budget is, or when it cools
  down (`fleet-govern`'s job) — this crate only ever judges the artifact/decision already in hand.
- Does **not** implement the corpus lookup itself (`intake.sh`'s `challenge_source_exists`, a live
  `grep` against `FAILURE-CORPUS.md`/`THREAD-LESSONS.md`) — takes a caller-supplied predicate/set of
  known corpus keys instead (§3, `challenge_source_known`), same pattern `fleet-types` used for
  `Receipt.ts_wall` staying an opaque `String`: the IO-shaped knowledge stays with whoever can do
  the IO.
- Does **not** define `Role`, `TaskId`, `NodeId`, `LifecycleState`, or any wire (de)serialization
  format for a receipt/attestation — all owned by `fleet-types`, imported by name.

## 3. Public API contract

```rust
//! fleet-plan: the spec-production pipeline's decisions — intake, LLD draft, the LLD-ready gate,
//! review verdicts, and teach-back — as pure, typed functions. Zero IO, zero clock, zero RNG,
//! zero subprocess of its own; every fact this crate needs is a parameter, not a read.

use fleet_types::{LifecycleState, NodeId, Role, TaskId};
use serde_json::Value;
use std::collections::BTreeSet;

// =====================================================================================
// A. Intake — fleet/registry-reference/registry/features/intake/intake.sh
// =====================================================================================

/// The 15-id catalog `intake.sh`'s `valid_id()` (`intake.sh:55-60`) hard-codes. Closed set, not a
/// bare `String`, so an unknown id is a compile-time-impossible construction, not a runtime typo.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum QuestionId {
    Scope, Success, CliFlag, SchemaMigration, UiView, ApiEndpoint, Deletion, Scale, Tenancy,
    Precision, Auth, Ownership, Failure, Unhappy, RenameRefactor,
}

/// `id` did not match any of the 15 known question ids.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0:?} is not a known intake question id")]
pub struct UnknownQuestionId(pub String);

impl QuestionId {
    /// Mirrors `valid_id`'s case arms (`intake.sh:56-59`) plus `emit_core`'s two ids.
    pub fn parse(id: &str) -> Result<Self, UnknownQuestionId> { unimplemented!() }
    pub fn as_str(self) -> &'static str { unimplemented!() }
}

/// Why a question is on the rubric: the two fixed items every intent gets (`emit_core`,
/// `intake.sh:105-106`), or a derived item that fired because `intent` contained `token`
/// (`emit_derived`, `intake.sh:84-89`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    Core,
    Token(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntakeQuestion {
    pub id: QuestionId,
    pub dimension: &'static str,
    pub question: &'static str,
    pub trigger: Trigger,
}

/// Derives the rubric for one intent string. Byte-faithful port of `write_questions`
/// (`intake.sh:91-126`): the 2 core rows always present, the 13 derived rows each gated on a fixed
/// ERE matching the lower-cased intent (`intake.sh:111-123`) — matches `first_match`'s
/// case-insensitive, first-occurrence semantics (`intake.sh:66-68`) exactly.
pub fn derive_questions(intent: &str) -> Vec<IntakeQuestion> { unimplemented!() }

/// The subset of `all` not present in `answered`. Mirrors `open_count`/`emit_questions`'s filter
/// (`intake.sh:127-148`), split from I/O: `answered` is the caller's already-read
/// `answers/*.txt` presence set, not a live directory scan.
pub fn open_questions<'a>(
    all: &'a [IntakeQuestion],
    answered: &BTreeSet<QuestionId>,
) -> Vec<&'a IntakeQuestion> { unimplemented!() }

/// One SOW/atomic/challenge/clarification structural defect: a human-readable reason, matching the
/// `echo '...' >&2; return 1` shape every `validate_*` function in `intake.sh` uses today (never a
/// bare `bool` — the reason is load-bearing, it is what `stage_blocked` prints).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct StageViolation(pub String);

/// Byte-faithful port of `validate_sow`'s structural checks (`intake.sh:181-194`) EXCLUDING the
/// hash comparison, which `validate_sow` computes via `hash_file` (`intake.sh:50`, a `shasum`
/// subprocess) — that hash is a parameter here (`intent_hash`), computed by the caller.
pub fn validate_sow_text(sow_text: &str, intent_hash: &str) -> Vec<StageViolation> { unimplemented!() }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicTier { Feature, Service, Module }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicRow {
    pub id: String,
    pub tier: AtomicTier,
    pub parents: Vec<String>,
    pub description: String,
    pub inputs: String,
    pub outputs: String,
    pub acceptance: String,
    pub design_decision: String,
}

/// Byte-faithful port of `validate_atomic`'s row/tier/parent-chain rules (`intake.sh:203-227`).
/// Takes already-TSV-parsed rows (the caller does the `IFS=$'\t' read` and header check) — this fn
/// owns only the row-shape, tier-composition, and design-decision-leakage decisions.
pub fn validate_atomic_rows(rows: &[AtomicRow]) -> Vec<StageViolation> { unimplemented!() }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengeRow {
    pub id: String,
    pub source: String,
    pub affected_leaf: String,
    pub risk: String,
    pub trigger: String,
    pub mitigation: String,
}

/// Byte-faithful port of `validate_challenges` (`intake.sh:243-254`) EXCLUDING
/// `challenge_source_exists`'s own file grep (`intake.sh:236-242`) — `corpus_known` stands in for
/// it: `true` iff the caller already resolved that exact `source` string against
/// `FAILURE-CORPUS.md`/`THREAD-LESSONS.md`.
pub fn validate_challenge_rows(
    rows: &[ChallengeRow],
    atomic_ids: &BTreeSet<String>,
    corpus_known: impl Fn(&str) -> bool,
) -> Vec<StageViolation> { unimplemented!() }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClarificationKind { Business, Technical }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClarificationRow {
    pub id: String,
    pub kind: ClarificationKind,
    pub gap_ref: String,
    pub question: String,
    pub blocking: bool,
    pub answer: String,
}

/// Byte-faithful port of `validate_clarifications` (`intake.sh:272-284`) EXCLUDING
/// `clarification_ref_exists`'s own file lookups (`intake.sh:264-271`) — `ref_known` stands in.
pub fn validate_clarification_rows(
    rows: &[ClarificationRow],
    ref_known: impl Fn(&str) -> bool,
) -> Vec<StageViolation> { unimplemented!() }

/// The 4-stage readiness a caller has already independently checked (SOW/atomic/challenges/
/// clarifications each valid or not) — mirrors `stage_ready`'s four-way AND (`intake.sh:310`),
/// computed by the caller from the four `validate_*` fns above rather than re-validated here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageReadiness {
    pub sow: bool,
    pub atomic: bool,
    pub challenges: bool,
    pub clarifications: bool,
}

impl StageReadiness {
    pub fn all_ready(self) -> bool { unimplemented!() }
}

/// Why code-start is refused, matching `gate()`'s three named reasons (`intake.sh:311-322`) plus
/// the success case — a closed enum instead of `gate()`'s bare `reason` string, so a caller cannot
/// misspell or invent a fourth reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDecision {
    Ready,
    BlockedByRubric,
    BlockedByStages,
    BlockedByOpenClarifications { count: u32 },
}

/// Byte-faithful port of `gate()`'s decision tree (`intake.sh:311-322`), stripped of the
/// `rubric_gate`/`stage_ready`/`open_blockers` file reads it interleaves today — each is now a
/// caller-supplied fact.
pub fn intake_gate(
    rubric_open: usize,
    stages: StageReadiness,
    open_blocking_clarifications: u32,
) -> GateDecision { unimplemented!() }

// =====================================================================================
// B. LLD draft assembly — fleet/registry-reference/.../intake.sh's blueprint()/summary()
// =====================================================================================

/// Pure text assembly of the four accepted artifacts into the build blueprint document, matching
/// `blueprint()`'s five-section layout (`intake.sh:326`) verbatim — this fn does not gate on
/// readiness itself (the caller calls `intake_gate` first and only calls this on `Ready`).
pub fn assemble_blueprint_doc(
    sow_text: &str,
    atomic_tsv: &str,
    challenges_tsv: &str,
    clarifications_business_tsv: &str,
    clarifications_technical_tsv: &str,
) -> String { unimplemented!() }

/// The fixed 4-check acceptance-checks draft template (`intake.sh:327`), parameterised only by the
/// drafting model's name (today `GENERATOR_MODEL`, an env var read at the top of the script,
/// `intake.sh:11` — the caller resolves that env var, not this crate).
pub fn assemble_acceptance_checks_draft(drafting_model: &str) -> String { unimplemented!() }

/// The human sign-off summary (`summary()`, `intake.sh:330-335`) — `artifact_refs` stands in for
/// the five `$STATE/...` file paths the bash version interpolates; this crate never constructs a
/// path, so the caller passes whatever labels/paths it wants shown.
pub fn assemble_summary_doc(
    intent: &str,
    artifact_refs: &[(&str, &str)],
    acceptance_checks_draft: &str,
) -> String { unimplemented!() }

// =====================================================================================
// C. LLD-ready shape validator — fleet/keel/fleet/src/lld.rs (near-verbatim lift)
// =====================================================================================

/// One shape violation: a dotted/bracket-indexed field path plus a human-readable reason.
/// Verbatim from `lld.rs:24-28`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub message: String,
}

/// Hand-written mirror of `module-brief.v1.json`, collecting every violation (not just the first)
/// so the cross-language comparator can diff error-path SETS across the Rust/TS/Python mirrors.
/// Verbatim from `lld.rs:182-425` (`validate_module_brief`) — see §5 for why this crate, not
/// `fleet-types`, is the right home even though it is a pure JSON-shape validator.
pub fn validate_module_brief(value: &Value) -> Vec<Violation> { unimplemented!() }

/// Hand-written mirror of `lld.v1.json`, the wrapper crossing the orb->fleet seam. Verbatim from
/// `lld.rs:705-755` (`validate_lld_v1`), delegating to `validate_module_brief` for the nested
/// `module_brief` object.
pub fn validate_lld_v1(value: &Value) -> Vec<Violation> { unimplemented!() }

/// A JSON number was encountered while canonicalizing — refused structurally (three-language
/// float-formatting divergence, `lld.rs:762-767`), never silently coerced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericLeafError;

/// Deterministic JSON serialisation: object keys sorted recursively, array order preserved.
/// Verbatim from `lld.rs:781-804` (`canonical_json`).
pub fn canonical_json(value: &Value) -> Result<String, NumericLeafError> { unimplemented!() }

/// `"sha256:" + hex(sha256(canonical_json(value)))`. Verbatim from `lld.rs:825-827`
/// (`content_hash`) — the one place this crate touches a hash function, and it is a pure
/// in-memory digest, not IO.
pub fn content_hash(value: &Value) -> Result<String, NumericLeafError> { unimplemented!() }

// =====================================================================================
// D. LLD-ready depth gate — fleet/keel/fleet/src/lld_ready.rs (near-verbatim lift)
// =====================================================================================

/// The 14 check ids, byte-identical to and in the same order as `lld_ready.rs:24-39`.
pub const GATE_CHECK_IDS: [&str; 14] = [
    "C1-OPEN", "C2-OWNER", "C3-ACC-PARSE", "C3-ACC-GROUND", "C3-ACC-NONTAUT", "R17-DERIV",
    "R19-ABSOLUTE", "R21-ALTS", "R21-FAIL", "C12-STORE", "C12-DEPS", "REG-VERDICT", "IFACE",
    "SHAPE",
];

/// Caller-supplied reference sets. Verbatim shape from `lld_ready.rs:44-48` (`GateRefs`) — NOT
/// read from disk here; the composition root reads `owners.v1.json`/`gate-refs.v1.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateRefs {
    pub owners: Vec<String>,
    pub registry_paths: Vec<String>,
    pub known_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReason {
    pub check_id: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthScore {
    pub checks_total: u32,
    pub checks_passed: u32,
    pub ratio: f64,
    pub failed_check_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome { Ready, NotReady, MeasuredNothing }

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub outcome: Outcome,
    pub checked: usize,
    pub score: Option<DepthScore>,
    pub reasons: Vec<GateReason>,
}

pub type Check = (&'static str, fn(&Value, &GateRefs) -> bool);

/// Verbatim from `lld_ready.rs:85-100` (`CHECKS`) — the 14 check fns themselves stay
/// `pub(crate)`/private, matching today's visibility; only the table, types, and entry points
/// below are the public surface.
pub const CHECKS: &[Check; 14] = &[/* elided — see §5, lifted verbatim */];

/// Verbatim from `lld_ready.rs:105` — the gate's identity, compared (never re-constructed) by
/// `check_freeze`'s successor in this crate (§C's `validate_lld_v1` chain).
pub const STAMPED_BY: &str = "keel:lld-ready";

/// The public entry point; always measures the full 14-check vocabulary. Verbatim from
/// `lld_ready.rs:107-110`.
pub fn evaluate(brief: &Value, refs: &GateRefs) -> Verdict { unimplemented!() }

/// The same evaluator over an explicit check slice — `pub` so an empty slice (⇒
/// `Outcome::MeasuredNothing`) is directly testable. Verbatim from `lld_ready.rs:112-161`.
pub fn evaluate_with(brief: &Value, refs: &GateRefs, checks: &[Check]) -> Verdict { unimplemented!() }

/// The `depth_evidence` block for `freeze.v1`. Verbatim from `lld_ready.rs:171-186`.
pub fn depth_evidence(v: &Verdict) -> Option<Value> { unimplemented!() }

/// Shape-then-readiness, as one pure function. Verbatim from `lld_ready.rs:193-204`
/// (`EntryOutcome`/`check_and_evaluate`).
pub enum EntryOutcome {
    ShapeInvalid(Vec<Violation>),
    Gate(Verdict),
}

pub fn check_and_evaluate(brief: &Value, refs: &GateRefs) -> EntryOutcome { unimplemented!() }

// =====================================================================================
// E. Review verdict decision — fleet/registry-reference/registry/services/review/review.sh
// =====================================================================================

/// The declared entity shape `review.sh` fetches per-role via `roles.sh show ROLE`
/// (`review.sh:37-43`) — here a plain caller-supplied value, never fetched by this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleContract {
    pub reviewer: String,
    pub may_produce: BTreeSet<String>,
    pub must_consume: BTreeSet<String>,
    pub cycle_states: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewContractViolation {
    /// `review.sh:50-52` — the worker's declared `reviewer` field names a different role.
    #[error("role {worker_role:?} must be reviewed by {expected:?}, not {reviewer_role:?}")]
    WrongAuthority { worker_role: String, expected: String, reviewer_role: String },
    /// `review.sh:53`.
    #[error("role {0:?} cannot produce work_output")]
    RoleCannotProduce(String),
    /// `review.sh:54`.
    #[error("reviewer role {0:?} cannot consume work_output through the review cycle state")]
    ReviewerCycleMissing(String),
}

/// Byte-faithful port of `validate_review_contract` (`review.sh:45-55`) — the two role-JSON reads
/// it performs today (`role_json "$worker_role"`/`role_json "$reviewer_role"`) are the caller's
/// job; both `RoleContract` values arrive already resolved.
pub fn validate_review_contract(
    worker_role: &str,
    worker: &RoleContract,
    reviewer_role: &str,
    reviewer: &RoleContract,
) -> Result<(), ReviewContractViolation> { unimplemented!() }

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SubmissionRefusal {
    /// `review.sh:116`.
    #[error("worker and reviewer must be different agent identities")]
    SelfReview,
    /// `review.sh:119` — worker must be at the `verify` lifecycle state to submit.
    #[error("worker is at {state:?}; submission requires the verify state")]
    OutputNotReady { state: LifecycleState },
    /// `review.sh:121` — reviewer must be at `new` or `plan`.
    #[error("reviewer is at {state:?}; expected new or plan")]
    ReviewerNotReady { state: LifecycleState },
    /// `review.sh:124` — the review loop for this task already used its attempt budget.
    #[error("review loop is exhausted at {attempts}/{limit} attempts")]
    Exhausted { attempts: u32, limit: u32 },
}

/// Byte-faithful port of `submit`'s eligibility preconditions (`review.sh:113-124`) EXCLUDING the
/// `transition()`/`append_review_receipt` side effects it also performs (`review.sh:125-127`) —
/// this fn only judges eligibility; the caller performs the transition/receipt if `Ok`.
#[allow(clippy::too_many_arguments)]
pub fn submission_eligible(
    worker_id: &TaskId,
    reviewer_id: &TaskId,
    worker_state: LifecycleState,
    reviewer_state: LifecycleState,
    attempts: u32,
    limit: u32,
) -> Result<(), SubmissionRefusal> { unimplemented!() }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerdictName { Accept, Revise, Reject }

/// What a verdict resolves to, once eligibility and the retry ceiling are accounted for. `Revise`
/// carries the fixed re-entry state (`review.sh:160`'s `--reenter-state`, always `"plan"` today —
/// `REENTER_STATE` defaults to and is validated against exactly that one value).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerdictDecision {
    Accept,
    Revise { reentered_state: &'static str },
    Reject,
    /// The retry ceiling was already hit when a `Revise` was requested (`review.sh:153-159`) —
    /// `verdict()` silently turns this into an escalation instead of another revise cycle.
    Escalate { attempts: u32, limit: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VerdictRefusal {
    /// `review.sh:134` — every verdict requires a non-empty `--reason`.
    #[error("a reason is required for every verdict")]
    ReasonMissing,
    /// `review.sh:138` — verdict requires the worker to be at `review`.
    #[error("worker is at {0:?}; verdict requires the review state")]
    WorkerNotInReview(LifecycleState),
    /// `review.sh:160` — today's `review.sh` only supports re-entering at `"plan"`.
    #[error("re-entering at {0:?} is not supported; only \"plan\" is")]
    ReenterStateUnsupported(String),
}

/// Byte-faithful port of `verdict`'s branch selection (`review.sh:131-173`) EXCLUDING the
/// `ratchet.sh check`/`accept` subprocess calls (`review.sh:143,146`), the `transition()` calls,
/// and `append_review_receipt` — those are the composition root's job once it has this decision.
/// `reason` is validated for presence only (its content is never inspected — `review.sh` never
/// inspects `$reason` beyond requiring it be non-empty either).
pub fn verdict_decision(
    reason: &str,
    worker_state: LifecycleState,
    verdict: VerdictName,
    attempts: u32,
    limit: u32,
    reenter_state: &str,
) -> Result<VerdictDecision, VerdictRefusal> { unimplemented!() }

// =====================================================================================
// F. Teach — build-new: turning a settled outcome into a corpus-citable lesson
// =====================================================================================

/// Mirrors `challenge_source_exists`'s two recognised prefixes (`intake.sh:236-242`) as a closed
/// type instead of a colon-split string, so a lesson can only cite a corpus this crate already
/// knows how to format a reference into.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LessonSource {
    FailureCorpus(String),
    ThreadLessons(String),
}

impl LessonSource {
    /// The exact `"FAILURE-CORPUS:<key>"` / `"THREAD-LESSONS:<key>"` wire form
    /// `challenge_source_exists` pattern-matches on (`intake.sh:237,238`).
    pub fn as_wire_ref(&self) -> String { unimplemented!() }
}

/// The one thing that happened and is now being turned into a lesson: which decision fired, and
/// (for a caught defect) which specific check/mutant caught it — named so a later `derive_lesson`
/// call can write a `trigger`/`mitigation` pair that is actually specific, not the generic-question
/// text `intake.sh:281`'s own denylist already refuses (`'^(what do you want|tell me more|...)'`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaughtOutcome {
    Rejected { reviewer_role: String, reason: String },
    Escalated { task: TaskId, attempts: u32, limit: u32 },
    GateRefused { check_id: &'static str, detail: String },
    MutationSurvived { mutant: String, killed_by: Option<String> },
}

/// The `challenges.tsv` row shape (`id\tsource\taffected_leaf\trisk\ttrigger\tmitigation`,
/// `intake.sh:246`) reused verbatim as the wire shape a teach step emits — greenfield: nothing in
/// fleet today assembles this row from an outcome, only reads existing rows
/// (`challenge_source_exists`, `intake.sh:236-242`). No third-party crate needed — plain data
/// construction over the outcome's own fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lesson {
    pub source: LessonSource,
    pub affected_leaf: NodeId,
    pub risk: String,
    pub trigger: String,
    pub mitigation: String,
}

/// Turns one settled outcome into a `Lesson` ready for the caller to append (as a new
/// `challenges.tsv` row, or a new `FAILURE-CORPUS.md` entry) via `fleet-store`. `source` names
/// which corpus this lesson is destined for and its key — this fn does not invent a corpus key, it
/// is supplied by the caller (mirrors this crate's existing pattern of never inventing an
/// identifier that must be globally unique/registry-checked; that check is `fleet-store`'s job).
pub fn derive_lesson(
    outcome: &TaughtOutcome,
    source: LessonSource,
    affected_leaf: NodeId,
    role: Role,
) -> Lesson { unimplemented!() }
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `QuestionId` | Exactly the 15 ids `valid_id` recognises (`intake.sh:56-59`); `parse` is the only construction path. | A rubric question referencing an id `valid_id` would have rejected, silently accepted downstream because a bare `&str` was never checked against the catalog. |
| `AtomicRow.tier` / `AtomicTier` | Exactly `feature`/`service`/`module` (`intake.sh:212`'s `case` arms); a row cannot claim a fourth tier. | A typo'd tier (`"servcie"`) silently falling through `validate_atomic`'s `case` default and being treated as neither valid nor explicitly rejected — today's bash falls into the `*` arm and errors; the enum makes the 4th value simply not constructible. |
| `StageReadiness` | Plain 4-flag struct; `all_ready()` is the only way to collapse it to one bool — mirrors `stage_ready`'s single `&&` chain (`intake.sh:310`) as an explicit, named AND rather than four re-run shell tests. | One of the four stage checks being silently skipped because a caller forgot one of `stage_ready`'s four `&&` clauses — the struct's four named fields make "which stage" visible at every call site, not just in the boolean result. |
| `GateDecision` | Exactly the 4 outcomes `gate()`'s decision tree produces (`intake.sh:311-322`); `BlockedByOpenClarifications` always carries the count, never a bare "blocked" with the reason lost. | A caller printing "blocked" without knowing *why* — the exact defect `gate()`'s own `$reason`/`$blockers` pair exists to prevent, now enforced by the type instead of by convention. |
| `Verdict` (LLD-ready gate) | `score` is `None` **iff** `outcome == MeasuredNothing` (`lld_ready.rs:77-79`) — carried over unchanged from the exemplar's own invariant. | A verdict reporting a depth score with a zero denominator, which would silently read as "0/0 = 100%" to a careless caller. |
| `ReviewContractViolation` / `SubmissionRefusal` / `VerdictRefusal` | Each variant carries exactly the state `review.sh`'s corresponding `die` call already names in its message (`review.sh:50-54,116,119,121,124,134,138,160`) — never collapsed into one generic "review refused" error. | A caller catching a review refusal and being unable to tell a wrong-authority pairing from an exhausted retry budget from a missing reason — today's bash makes this distinguishable only by grepping the printed message; the enum makes it a `match` arm. |
| `LessonSource` | Exactly the two prefixes `challenge_source_exists` recognises (`intake.sh:237-238`); a lesson cannot cite a third, unrecognised corpus. | A teach-back step silently producing a challenge row whose `source` no validator will ever recognise — `validate_challenges` would reject it today (`intake.sh:250`), and this type makes that failure mode unconstructible instead of merely caught later. |
| `TaughtOutcome` | Closed enum over the four outcome shapes this crate's own decisions can produce (rejected, escalated, gate-refused, mutation-survived) — a teach step cannot be invoked with an outcome this crate never actually decided. | A lesson being fabricated for an outcome no upstream decision function ever returned, silently decoupling the corpus from what actually happened. |

**Money/precision:** no money type in this crate. `attempts`/`limit`/`checks_total`/`checks_passed`
are all `u32`, never float, matching `lld_ready.rs`'s own `u32` choice (only `DepthScore.ratio`
stays `f64`, carried over unchanged from `lld_ready.rs:62` because it is a reporting statistic, not
a decision input — same justification `fleet-types` §4 already gives for Wilson-score rates).

**Clock/RNG/IO injection points:** none — this crate is pure, by construction rather than by
discipline: `GateRefs`, `RoleContract`, `corpus_known`/`ref_known` closures, and every `attempts`/
`state`/`limit` parameter are the caller's already-resolved facts. The one place a real clock
appears in the source material — `intake.sh`'s `NOW`/`now_iso()` env-var-or-`date` stamp
(`intake.sh:12,31`) — never enters this crate at all: every assembled document (`assemble_*`) is
timestamp-free; the caller stamps a receipt's `ts_wall` itself via `fleet-store`.

## 5. Reuse map

Source read in full: `fleet/registry-reference/registry/features/intake/intake.sh` (358 lines),
`fleet/registry-reference/registry/services/review/review.sh` (208 lines), `fleet/keel/fleet/src/
lld.rs` (1027 lines), `fleet/keel/fleet/src/lld_ready.rs` (557 lines).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `intake.sh:55-60` (`valid_id`) | `case` match over 15 literal ids. | logic yes, shape no | Becomes `QuestionId` enum + `parse`, returning a typed `UnknownQuestionId` instead of a shell exit code. |
| `intake.sh:91-126` (`write_questions`, `emit_core`, `emit_derived`) | Builds `questions.tsv` from 2 fixed + 13 ERE-triggered rows, writing to a temp file then `mv`. | logic yes, IO no | `derive_questions` returns `Vec<IntakeQuestion>` in memory; the temp-file-then-atomic-rename dance is gone entirely — there is no file to write. The 13 EREs (`intake.sh:111-123`) are ported verbatim as Rust string/regex-free matching (see note below on the no-`regex` convention this workspace already follows, `lld.rs:59-63`). |
| `intake.sh:127-148` (`open_count`, `emit_questions`) | Reads `questions.tsv`, checks each id's answer file, prints open ones. | logic yes, IO no | `open_questions` takes `all: &[IntakeQuestion]` and `answered: &BTreeSet<QuestionId>` instead of re-reading `questions.tsv` and stat-ing `answers/*.txt` per id. |
| `intake.sh:149-152` (`rubric_gate`) | Boolean: no intent recorded, or `open_count() == 0`. | logic yes, IO no | The "no intent recorded" branch is a precondition the caller enforces (it already has `intent: &str` in hand to call `derive_questions`) — `intake_gate` takes `rubric_open: usize` directly. |
| `intake.sh:181-194` (`validate_sow`) | File-shape + `source_intent_hash` check against `hash_file`'s `shasum` output. | logic yes, hash no | `validate_sow_text` takes `intent_hash: &str` as a parameter; `hash_file`'s `shasum -a 256` subprocess (`intake.sh:50`) is the caller's job (or `fleet-store`'s, if intent hashing is centralised there — flagged, not decided, in the notes at the end). |
| `intake.sh:203-227` (`validate_atomic`) | TSV header/row/tier/parent-chain validation, reading `$file` line-by-line with `IFS=$'\t' read`. | logic yes, parsing no | `validate_atomic_rows` takes `&[AtomicRow]`; the caller does the TSV read (or a future `fleet-store` reader does). The duplicate-id and parent-tier checks (`intake.sh:211,219-226`) port directly to `BTreeSet`/lookup logic. |
| `intake.sh:236-254` (`challenge_source_exists`, `validate_challenges`) | Greps `FAILURE-CORPUS.md`/`THREAD-LESSONS.md` per row; validates the rest structurally. | logic yes, corpus lookup no | `validate_challenge_rows` takes `corpus_known: impl Fn(&str) -> bool` in place of the two `grep` calls (`intake.sh:238-239`) — the caller resolves both files once and passes a closure/set. |
| `intake.sh:264-284` (`clarification_ref_exists`, `validate_clarifications`) | Same shape, three `grep`/`cut` lookups against `sow.md`/`atomic.tsv`/`challenges.tsv`. | logic yes, lookup no | `validate_clarification_rows` takes `ref_known: impl Fn(&str) -> bool` the same way. |
| `intake.sh:309-322` (`stage_ready`, `open_blockers`, `gate`) | Four-way AND of file-backed validity checks, plus a blocking-clarification count, collapsed into one `$reason`. | logic yes, IO no | `StageReadiness`/`intake_gate` as in §3 — the four `[ -s FILE ] && validate_* FILE` calls become four bools the caller already computed by calling this crate's own `validate_*` fns. |
| `intake.sh:323-329` (`blueprint`) | Concatenates 5 sections from 5 state files into `blueprint.md`, plus a fixed 4-line acceptance-checks-draft template. | logic yes, IO no | `assemble_blueprint_doc`/`assemble_acceptance_checks_draft` take the already-read text of each section; no file write. |
| `intake.sh:330-335` (`summary`) | Concatenates intent + 5 `$STATE/...` paths + the draft's 4 checks into `summary.md`. | logic yes, IO no | `assemble_summary_doc` takes `artifact_refs: &[(&str, &str)]` (label, ref) pairs instead of constructing `$STATE`-rooted paths itself — this crate never knows what `$STATE` is. |
| `lld.rs:1-1027` (whole file: `Violation`, `validate_module_brief`, `validate_lld_v1`, `check_freeze`, `check_sow_seed`, `check_registry_verdict`, `check_accepts_when`, `canonical_json`, `content_hash`, `cross_lang_report`) | Hand-written `module-brief.v1`/`freeze.v1`/`lld.v1` shape mirror + canonical-JSON/content-hash. Already `pub`, already zero-IO (only `cross_lang_report` touches `std::fs`, and only to read test fixtures — that fn is test tooling, not pipeline logic, and does **not** move here; see the notes below). | yes, near-verbatim | Path/module only: `pub(crate)` helpers used only within `lld.rs` (`valid_node_id`, `number_calc_ok`, `structural_enforced_by_ok`) become plain `pub(crate)` items inside this crate's `ready_gate` module tree instead of the `fleet` binary's `lld` module — no signature or behavior change. `cross_lang_report` and its `std::fs::read_dir`/`read_to_string` calls are **not** lifted (real filesystem IO reading a fixtures directory) — the cross-language comparator becomes a test-harness concern for whichever crate owns `fleet/tests/acceptance/lld-crosslang.sh`'s Rust side, most likely this crate's own `tests/` directory calling `validate_lld_v1`/`canonical_json` directly against fixtures the *test* reads (a test is allowed to touch the filesystem via `tempdir`/fixture dirs; library code is not). |
| `lld_ready.rs:1-557` (whole file: `GATE_CHECK_IDS`, `GateRefs`, `GateReason`, `DepthScore`, `Outcome`, `Verdict`, `Check`, `CHECKS`, `STAMPED_BY`, `evaluate`, `evaluate_with`, `depth_evidence`, `EntryOutcome`, `check_and_evaluate`, the 14 check fns) | The 14-check depth-readiness gate. Already `pub`, already zero-IO, already exactly this crate's shape (its own doc comment says "ZERO file I/O, ZERO clock reads, ZERO non-deterministic inputs", `lld_ready.rs:6`). | yes, verbatim | None beyond the module path — this file needed no reshaping to become part of `fleet-plan`; it already is what this crate's charter asks for. |
| `review.sh:27-35` (`retry_limit`) | `node --input-type=module -e '...'` reading `RETRY_POLICY.maxAttemptsPerTask` from `console/server/sdlc.mjs`. | **no** | Pure subprocess + JS-module read; `limit: u32` is a parameter to every fn here that needs it (`submission_eligible`, `verdict_decision`). Whoever owns re-reading that policy (`src/`, or a future `fleet-govern` policy source) resolves it once per call, not per this crate. |
| `review.sh:37-43` (`role_json`) | `roles.sh show ROLE` subprocess, parsed with `jq`. | **no** | `RoleContract` is the parameter standing in for its resolved JSON; `fleet-worker` or `src/` is the caller who actually runs `roles.sh` (or its eventual Rust replacement) and parses the result. |
| `review.sh:45-55` (`validate_review_contract`) | Given two already-fetched role JSON blobs, checks reviewer-authority/produce/consume-cycle facts. | logic yes, fetch no | `validate_review_contract` takes two `&RoleContract` values directly — the two `role_json` subprocess calls it wraps today (`review.sh:47-48`) are gone; only the three `jq -e` boolean checks (`review.sh:50,53,54`) port over as struct-field lookups. |
| `review.sh:57-72` (`state_of`, `transition`) | `statemachine.sh replay`/`transition` subprocess calls, output-string parsed with `sed`. | **no** | `fleet-lifecycle`'s job entirely — this crate takes `worker_state`/`reviewer_state: LifecycleState` as already-resolved parameters, never calls `state_of`/`transition` itself. |
| `review.sh:74-81` (`append_review_receipt`) | `receipt_append` call into the ledger. | **no** | `fleet-store`'s job — this crate's `verdict_decision`/`submission_eligible` return a decision; the caller decides what receipt event that decision implies and appends it. |
| `review.sh:83-87` (`attempts_for`) | `jq`-filters the ledger file for prior `review_submitted` receipts, counts them. | **no** | `fleet-store`'s job (a ledger query) — `attempts: u32` is a parameter here, already counted by the caller. |
| `review.sh:89-111` (`reviewer_cycle_to_review`) | Drives the reviewer's state machine through 6 named transitions to reach `review`. | **no** | Entirely `fleet-lifecycle`'s job (repeated `transition()` subprocess calls) — this crate assumes the caller has already done this and passes the resulting `reviewer_state`. |
| `review.sh:113-129` (`submit`) | Validates contract, checks worker/reviewer states + retry ceiling, THEN transitions both + appends a receipt. | logic (preconditions) yes, transitions/receipt no | `submission_eligible` is the precondition half only (`review.sh:114-124`); the `transition()`/`append_review_receipt` calls (`review.sh:125-127`) stay with the caller. |
| `review.sh:131-174` (`verdict`) | Validates reason, drives reviewer to `review`, then per-verdict: ratchet subprocess + 2 transitions + receipt (`accept`); retry-ceiling check + 2 transitions + receipt (`revise`/escalate); 2 transitions + receipt (`reject`). | logic (branch selection) yes, ratchet/transitions/receipt no | `verdict_decision` is the pure branch-selection half; `reviewer_cycle_to_review`'s own transitions (`review.sh:139`) plus every verdict branch's `RATCHET`/`transition`/`append_review_receipt` calls stay with the caller — most likely `fleet-verify` for the ratchet check (row 8, "the re-run-every-check gate wall") and `fleet-lifecycle`/`fleet-store` for the rest. |
| — (greenfield) | No file in fleet today assembles a `challenges.tsv`-shaped row from a settled outcome; `challenge_source_exists` only ever reads existing rows. | greenfield | `LessonSource`/`TaughtOutcome`/`Lesson`/`derive_lesson` in §3 F — new vocabulary this blueprint introduces per the task's explicit "teach" requirement, built from the existing `challenges.tsv` row shape (`intake.sh:246`) so a lesson this crate produces is immediately valid input to `validate_challenge_rows` above. No third-party crate needed. |

## 6. Behavior spec

### `fn derive_questions(intent: &str) -> Vec<IntakeQuestion>`

| Input dimension | Behavior |
|---|---|
| empty | `""` → the 2 core questions only (`scope`, `success`); none of the 13 derived EREs match an empty string, matching `write_questions`'s behavior on an empty intent today. |
| null / `None` | n/a — takes `&str`, not `Option<&str>`. |
| wrong-type | n/a — no type erasure at this boundary. |
| huge | A 100,000-character intent containing every trigger word at least once → all 15 questions, each derived row's `Trigger::Token` carrying only the FIRST case-insensitive match per `first_match`'s semantics (`intake.sh:68`), never a list of all matches — ported verbatim, not "improved" into collecting every occurrence. |
| negative | n/a — not numeric. |
| duplicate | Calling `derive_questions` twice with the identical intent string is idempotent — same 15-or-fewer rows both times (pure fn, no counter, no id generation). |
| concurrent | Pure value fn, no shared state — trivially safe from any number of threads. |
| unicode / non-ASCII | An intent containing only non-ASCII text (e.g. entirely Chinese) → still gets the 2 core questions (they are unconditional) and zero derived ones, since none of the 13 EREs (`intake.sh:111-123`) are written to match non-Latin scripts — matches today's bash behavior exactly, not a silent behavior change. |
| already-exists | n/a — no persisted state; every call is a fresh derivation. |
| partial-failure | n/a — no IO, cannot fail partially. |

### `fn intake_gate(rubric_open: usize, stages: StageReadiness, open_blocking_clarifications: u32) -> GateDecision`

| Input dimension | Behavior |
|---|---|
| empty | `rubric_open == 0`, all four `stages` flags `true`, `open_blocking_clarifications == 0` → `GateDecision::Ready`. |
| null / `None` | n/a — every parameter is a required, already-resolved value; there is no "unknown" state, matching `gate()`'s own behavior (`rubric_gate` failing is treated as "blocked", never as "unknown"). |
| wrong-type | n/a — no type erasure; `stages`'s four fields are each a plain `bool`. |
| huge | `open_blocking_clarifications = u32::MAX` with `rubric_open == 0` and `stages.all_ready() == true` → `BlockedByOpenClarifications { count: u32::MAX }`, no overflow, no saturation logic needed (it is only ever compared to `0`, never arithmetically combined). |
| negative | n/a — `usize`/`u32`, cannot go negative; a caller computing a negative count before calling this fn is a caller-side bug this fn cannot see. |
| duplicate | n/a — no identity-bearing input. |
| concurrent | Pure value fn — trivially safe. |
| unicode / non-ASCII | n/a — no string input. |
| already-exists | n/a — no persisted state. |
| partial-failure | n/a — no IO, cannot fail partially; unlike `gate()`, which can fail to even reach its own logic if `rubric_gate`'s file reads fail (`intake.sh:150`), this fn has already been handed the resolved facts and cannot itself observe a partial failure. |

### `fn verdict_decision(reason: &str, worker_state: LifecycleState, verdict: VerdictName, attempts: u32, limit: u32, reenter_state: &str) -> Result<VerdictDecision, VerdictRefusal>`

| Input dimension | Behavior |
|---|---|
| empty | `reason = ""` → `Err(VerdictRefusal::ReasonMissing)` regardless of every other argument, mirroring `review.sh:134`'s check running before any state/verdict logic. |
| null / `None` | n/a — `reason`/`reenter_state` are `&str`, never `Option`; `worker_state` is always a concrete `LifecycleState`, never "unknown" (the caller resolved it via `fleet-lifecycle` before calling). |
| wrong-type | n/a at this boundary — `VerdictName`'s three variants are the only representable verdict; there is no "invalid verdict string" case here because `review.sh:172`'s `*) die ... invalid_verdict` branch is unreachable once the caller has already parsed a bash `--verdict` string into a `VerdictName` (that parse, not this fn, is where an invalid string is caught — flagged in the notes as a small piece of `review.sh` this blueprint does not explicitly assign a home to). |
| huge | `attempts = u32::MAX`, `limit = 1`, `verdict = Revise` → `Ok(VerdictDecision::Escalate { attempts: u32::MAX, limit: 1 })`, no overflow (`attempts >= limit` is a plain comparison, never subtracted). |
| negative | n/a — `u32`, cannot represent a negative attempt count. |
| duplicate | Calling twice with identical arguments is idempotent — same `Result` both times (pure fn). |
| concurrent | Pure value fn — trivially safe. |
| unicode / non-ASCII | `reason` containing only emoji/non-ASCII text → accepted (only checked for non-empty, never inspected for content, matching `review.sh:134`'s bash `[ -n "$reason" ]` which is also byte-length-based, not content-based). `reenter_state` containing non-ASCII text (e.g. `"plän"`) → `Err(VerdictRefusal::ReenterStateUnsupported("plän".into()))`, since it does not byte-match the literal `"plan"` — no normalization, matching `review.sh:160`'s exact string comparison. |
| already-exists | n/a — no persisted state; every call is independent. |
| partial-failure | n/a — no IO; unlike `verdict()`, which can fail partway through (e.g. the ratchet subprocess succeeds but a subsequent `transition()` fails, `review.sh:143-147`), this fn either returns a complete decision or a complete refusal, never a partial one — that partial-failure risk lives entirely in the caller's orchestration of the IO steps this fn does not perform. |

### `fn derive_lesson(outcome: &TaughtOutcome, source: LessonSource, affected_leaf: NodeId, role: Role) -> Lesson`

| Input dimension | Behavior |
|---|---|
| empty | An outcome with an empty `reason`/`detail` string (e.g. `TaughtOutcome::Rejected { reason: String::new(), .. }`) → still produces a `Lesson`, with `trigger`/`mitigation` derived from the outcome's *kind* (which variant) rather than solely from its empty text field — never an empty `trigger`, since `validate_challenge_rows` (§3 A) would reject an empty one anyway (`intake.sh:252`'s `[ -n "$risk" ] && [ -n "$trigger" ] && [ -n "$mitigation" ]`). |
| null / `None` | `TaughtOutcome::MutationSurvived { killed_by: None, .. }` (a mutant with no test named as its killer) → `mitigation` names that gap explicitly ("no test currently kills this mutant") rather than silently omitting the field. |
| wrong-type | n/a — `TaughtOutcome`'s four variants are the only representable outcomes; there is no fifth kind that could be malformed. |
| huge | A `reason`/`detail` string of 100,000 characters → copied into `risk`/`trigger` without truncation (no length cap in `validate_challenge_rows`/`validate_challenges` either — `intake.sh:252` only checks non-empty, never a max length). |
| negative | n/a — `attempts`/`limit` inside `Escalated` are `u32`, already validated non-negative by whatever produced the `TaughtOutcome` in the first place. |
| duplicate | Calling `derive_lesson` twice with the same outcome produces two structurally-identical `Lesson` values (this fn does not assign an `id` — per §3's reuse-map note, `id` uniqueness is the caller's/`fleet-store`'s registry-check job, matching how `fleet-types` never invents a globally-unique identifier either). |
| concurrent | Pure value fn — trivially safe. |
| unicode / non-ASCII | A `reason` containing non-ASCII text → copied through unchanged (no ASCII-only validation anywhere in this fn — matches `validate_challenges`'s own byte/text-agnostic checks). |
| already-exists | n/a — this fn does not check the corpus for a duplicate lesson before constructing one; that check (mirrors `validate_challenge_rows`' `corpus_known` predicate) happens when the caller later tries to install the lesson as a new challenge row, not here. |
| partial-failure | n/a — no IO; this fn cannot fail, by construction (no `Result` in its signature — every input, however degenerate, produces *some* `Lesson`). |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | `path` (workspace) | `Role`, `TaskId`, `NodeId`, `LifecycleState` — this crate's public API names them directly per §1/§3; no local re-declaration. |
| `serde_json` | `1.0.151` (matches `fleet/keel/Cargo.lock`) | §C/§D's `Value`-based validators (`validate_module_brief`, `validate_lld_v1`, `evaluate`/`evaluate_with`) operate directly on `serde_json::Value`, unchanged from `lld.rs`/`lld_ready.rs`. |
| `sha2` | `0.10.9` (matches `fleet/keel/Cargo.lock`) | `content_hash`'s SHA-256 digest (`lld.rs:820-827`) — same crate fleet already resolves, not a second hash implementation. |
| `thiserror` | `2.0.20` (matches `fleet/keel/Cargo.lock`) | Every fallible fn here returns a `thiserror`-derived typed error (`UnknownQuestionId`, `StageViolation`, `ReviewContractViolation`, `SubmissionRefusal`, `VerdictRefusal`) instead of `String`/`anyhow`/bare `bool`. |

No async runtime, no regex crate (this workspace's existing no-`regex` convention, `lld.rs:59-63`,
extends to `derive_questions`'s 13 EREs — hand-written pattern matching, not a new dependency), no
logging framework, no filesystem/network/subprocess crate: matches this crate's zero-IO charter.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** `lib.rs` is a thin re-export hub; each pipeline
> stage gets its own module directory, and `lld.rs`/`lld_ready.rs`'s bulk (1027 + 557 lines) is
> split by *section* (mirroring the source files' own `// ====` banners) rather than lifted as two
> giant files. Verify before review: `find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80'`
> must print nothing.

```
crates/fleet-plan/
  Cargo.toml
  src/
    lib.rs                    # ~20 — module decls + re-exports only
    intake/
      mod.rs                  # ~10 — re-exports for the intake module
      question_id.rs          # ~45 — QuestionId, UnknownQuestionId, parse/as_str
      rubric.rs                # ~75 — IntakeQuestion, Trigger, derive_questions, open_questions
      sow.rs                   # ~40 — StageViolation, validate_sow_text
      atomic.rs                # ~75 — AtomicTier, AtomicRow, validate_atomic_rows
      challenges.rs            # ~55 — ChallengeRow, validate_challenge_rows
      clarifications.rs        # ~55 — ClarificationKind, ClarificationRow, validate_clarification_rows
      gate.rs                  # ~40 — StageReadiness, GateDecision, intake_gate
    lld_draft.rs                # ~55 — assemble_blueprint_doc, assemble_acceptance_checks_draft, assemble_summary_doc
    ready_gate/
      mod.rs                    # ~10 — re-exports for the ready_gate module
      violation.rs              # ~15 — Violation
      module_brief_core.rs      # ~75 — is_non_empty_str/str_trim_len/as_str/is_string_array + node_id/freeze_id/content_hash validators (lld.rs:37-127)
      module_brief_fields.rs    # ~75 — ALLOWED_*/FORBIDDEN_* const arrays (lld.rs:129-178)
      module_brief.rs           # ~80 — validate_module_brief part 1: top-level scalar fields (lld.rs:182-275)
      module_brief_nested.rs   # ~80 — validate_module_brief part 2: registry/acceptance/guarantees/alternatives/failure_story (lld.rs:275-425)
      registry_verdict.rs       # ~30 — check_registry_verdict (lld.rs:428-456)
      accepts_when.rs           # ~65 — check_accepts_when (lld.rs:459-519)
      freeze.rs                 # ~80 — check_freeze part 1: scalar fields (lld.rs:521-587)
      freeze_depth_evidence.rs  # ~60 — check_freeze part 2: depth_evidence/supersedes/stamped_by (lld.rs:588-667)
      sow_seed.rs               # ~35 — check_sow_seed (lld.rs:669-699)
      lld_v1.rs                 # ~55 — validate_lld_v1 (lld.rs:701-755)
      canonical.rs              # ~50 — NumericLeafError, canonical_json, hex_encode, hash_canonical_string, content_hash (lld.rs:757-827)
      gate_types.rs             # ~40 — GateRefs, GateReason, DepthScore, Outcome, Verdict, Check, GATE_CHECK_IDS, STAMPED_BY
      gate_eval.rs              # ~55 — evaluate, evaluate_with, round3, depth_evidence, EntryOutcome, check_and_evaluate
      gate_checks_a.rs          # ~70 — check_c1_open .. check_r21_fail (7 of 14 checks; lld_ready.rs:317-431)
      gate_checks_b.rs          # ~65 — check_c12_store .. check_shape (7 of 14 checks; lld_ready.rs:433-511)
      gate_text.rs              # ~55 — as_str/trim_len/as_array/as_str_array/is_word_char/contains_word_ci/contains_absolute_claim/matches_tautology/valid_revive_trigger (lld_ready.rs:212-311)
    review/
      mod.rs                    # ~10 — re-exports for the review module
      contract.rs               # ~40 — RoleContract, ReviewContractViolation, validate_review_contract
      submission.rs             # ~40 — SubmissionRefusal, submission_eligible
      verdict.rs                # ~55 — VerdictName, VerdictDecision, VerdictRefusal, verdict_decision
    teach.rs                    # ~55 — LessonSource, TaughtOutcome, Lesson, derive_lesson
  tests/
    intake_rubric.rs            # ~75 — derive_questions/open_questions/intake_gate behavior-spec cases
    intake_stage_validation.rs  # ~75 — validate_sow_text/validate_atomic_rows/validate_challenge_rows/validate_clarification_rows
    lld_draft.rs                 # ~40 — assemble_* text-assembly cases
    ready_gate_shape.rs          # ~75 — validate_module_brief/validate_lld_v1/canonical_json/content_hash (ported from lld.rs's own #[cfg(test)] module)
    ready_gate_depth.rs          # ~75 — evaluate/evaluate_with/depth_evidence/check_and_evaluate (ported from lld_ready.rs's own #[cfg(test)] module)
    review_decision.rs           # ~75 — validate_review_contract/submission_eligible/verdict_decision
    teach.rs                     # ~40 — derive_lesson behavior-spec cases
```

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-plan"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
serde_json = "1.0.151"
sha2 = "0.10.9"
thiserror = "2.0.20"

[dev-dependencies]
serde_json = "1.0.151"
```

## 9. Test plan

**Unit tests** (in each module's `#[cfg(test)]`, one per behavior-spec row not trivially covered
elsewhere):
- `question_id_parse_matches_all_15_and_only_15` — every `QuestionId` variant round-trips through
  `as_str()` → `parse()`; any string outside the 15-id catalog is `Err(UnknownQuestionId(_))`.
- `derive_questions_core_present_even_on_empty_intent` — `derive_questions("")` returns exactly
  `[Scope, Success]`, both `Trigger::Core`.
- `derive_questions_first_match_only_not_every_occurrence` — an intent repeating the same trigger
  word 3 times still produces one row for that id, `Trigger::Token` carrying the first match.
- `open_questions_excludes_answered_ids` — a 5-question rubric with 2 ids in `answered` returns
  exactly the other 3.
- `atomic_row_rejects_wrong_parent_tier` — a `service`-tier row whose parent is `module`-tier (not
  `feature`) is a `StageViolation` (mirrors `intake.sh:222`'s check).
- `challenge_row_rejects_unknown_corpus_source` — `corpus_known` returning `false` for a row's
  `source` produces a `StageViolation` naming that row's id.
- `intake_gate_priority_order_matches_bash` — with both `rubric_open > 0` AND
  `open_blocking_clarifications > 0`, asserts `BlockedByRubric` wins (mirrors `gate()`'s own
  sequential `if` chain, `intake.sh:312-319`, where rubric is checked first and short-circuits).
- `verdict_decision_revise_past_limit_escalates_not_revises` — `attempts >= limit` with
  `verdict = Revise` returns `Ok(VerdictDecision::Escalate{..})`, never `Revise`.
- `verdict_decision_reason_missing_wins_over_every_other_check` — an empty `reason` with an
  otherwise-invalid `worker_state`/`reenter_state` still returns `Err(ReasonMissing)` specifically
  (mirrors `review.sh:134`'s check running first, before `state_of` is even consulted).

**Integration tests** (calling only the public API):
- `ready_gate_shape.rs::validate_module_brief_accepts_a_minimal_well_formed_brief` /
  `rejects_an_extra_top_level_field` — ported verbatim from `lld.rs`'s own `#[cfg(test)]` module
  (lines 928-987), same fixture literals, proving the lift changed no behavior.
- `ready_gate_depth.rs::evaluate_is_total_over_a_completely_empty_object` — ported verbatim from
  `lld_ready.rs:552-556`.
- `review_decision.rs::full_submit_to_accept_happy_path` — chains
  `validate_review_contract` → `submission_eligible` → `verdict_decision(Accept)` with a
  consistent role/state/attempt fixture, asserting each stage's `Ok` feeds the next.
- `teach.rs::derive_lesson_from_gate_refusal_is_valid_challenge_input` — round-trips a `Lesson`
  produced from a `TaughtOutcome::GateRefused` back through `validate_challenge_rows` (with a
  `corpus_known` stub returning `true` for the lesson's own `source`) and asserts zero violations —
  the property this crate's charter actually requires ("a lesson this crate writes is one this
  crate's own validator would accept").

**Mutation-testing targets** (`cargo mutants -p fleet-plan`):
- Flipping `intake_gate`'s check order (rubric vs. stages vs. blockers) must be killed by
  `intake_gate_priority_order_matches_bash`.
- Changing `submission_eligible`'s `attempts < limit` to `attempts <= limit` must be killed by a
  dedicated `submission_eligible_boundary_at_exact_limit` test (exact-boundary, not `>=`/`<`
  ambiguity — mirrors `review.sh:124`'s `[ "$attempts" -lt "$limit" ]`).
- Swapping `verdict_decision`'s `Revise`/`Escalate` branches (returning `Revise` past the ceiling)
  must be killed by `verdict_decision_revise_past_limit_escalates_not_revises`.
- Deleting `derive_questions`'s "first match only" behavior (collecting all matches instead) must
  be killed by `derive_questions_first_match_only_not_every_occurrence`.
- Every `lld_ready.rs`/`lld.rs`-inherited mutation target the exemplar crates already name for
  their own copies of this logic (e.g. `fleet-types`'s blake3-length-boundary style) applies
  identically here to the ported `canonical_json`/`content_hash`/`evaluate_with` — killed by the
  ported tests in `ready_gate_shape.rs`/`ready_gate_depth.rs`.

**Property tests** (`proptest`, recommended):
- *`derive_questions` is a pure function of `intent` alone*: for any two calls with the same
  `intent` string, the returned `Vec<IntakeQuestion>` is identical (order included). 256 cases
  minimum, generated from a mix of ASCII words drawn from the 13 trigger vocabularies plus random
  filler text.
- *`intake_gate` is monotonic*: increasing `open_blocking_clarifications` from 0 while holding
  `rubric_open == 0` and `stages.all_ready() == true` never turns a `Ready` result back into
  `Ready` for a larger count once it has become `BlockedByOpenClarifications` — i.e. the decision
  never "un-blocks" as the blocker count grows. 128 cases minimum.

## 10. Verification recipe

```bash
cd crates/fleet-plan
cargo test -p fleet-plan --all-targets
cargo clippy -p fleet-plan --all-targets -- -D warnings
cargo mutants -p fleet-plan
```
Expected: all unit + integration + property tests pass, 0 skipped — publish as `<passed>/<total>`
(never just "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9 caught — publish
`<caught>/<total mutants>`; given this crate is a near-verbatim lift of two already-mutation-tested
files (`lld.rs`/`lld_ready.rs`) plus newly-typed decision logic, the floor is **100% of viable
mutants caught**, matching `fleet-types`' precedent — any survivor gets a new test before this
crate is marked done, never a lowered floor.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`UnknownQuestionId`, `StageViolation`,
      `ReviewContractViolation`, `SubmissionRefusal`, `VerdictRefusal`, `NumericLeafError`) — none
      swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code. Mark done once
      implemented and `grep -rn '\.unwrap()\|panic!' crates/fleet-plan/src/` outside `#[cfg(test)]`
      returns nothing.
- [ ] Clock/RNG/IO are injected — trivially, by having none: every public fn takes only
      already-in-memory values (`GateRefs`, `RoleContract`, closures for corpus/ref lookups,
      already-resolved `LifecycleState`/attempt counts) and returns owned values.
- [x] Thread-safety documented: every public type is a plain value with no interior mutability —
      `Send + Sync` for free, safe to construct, clone, and evaluate concurrently from any number
      of threads (mirrors `fleet-types`' and `lld_ready.rs`'s own documented purity).
- [x] No float used for money, tokens, or any precision-sensitive count — `DepthScore.ratio` is the
      one `f64` in this crate, a reporting statistic never used as a decision input (carried over
      unchanged from `lld_ready.rs`'s own documented rule, §4 above).
- [ ] No self-grading — verification runs `cargo mutants`, not just the crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10) — restate the real
      numbers in the PR once the crate is built.
- [x] Tests that touch the filesystem: none in `src/`'s own logic (zero-IO by charter); any test
      needing a fixtures directory (the `lld-crosslang.sh` comparator's eventual Rust-side
      equivalent, if built here at all — see §5's `cross_lang_report` note) must use
      `tempdir()`/a checked-in `tests/fixtures/` dir, never the repo tree at large or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from the code — no file open, no lock acquire, no
      subprocess spawn (`std::process::Command`), no ledger append, no lifecycle transition
      anywhere in `crates/fleet-plan/src/`. Enforce with
      `grep -rn 'std::fs::\|std::process::Command\|File::open\|File::create' crates/fleet-plan/src/`
      returning nothing.
- [ ] **No source file exceeds 80 lines** (verified: `find src tests -name '*.rs' | xargs wc -l` —
      every file ≤ 80). `lib.rs` is a thin hub, not a dumping ground.

## 12. Definition of Done

`fleet-plan` is DONE when: §10's three commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught, floor 100%) run from `crates/fleet-plan/`; every
unchecked box in §11 is checked with its real numbers; `registry/features/REGISTRY.md` (per
C1/L2 — this is a features-layer capability, the spec-production pipeline, not base
infrastructure) lists the crate; the two divergence notes below are resolved by Opus (which crate
actually shells out to `roles.sh`/`ratchet.sh`/`node` on this crate's behalf, and whether intent/SOW
hashing centralises in `fleet-store`); and Opus has independently re-derived `evaluate_with`'s
`MeasuredNothing` branch and `verdict_decision`'s escalate-vs-revise branch from
`lld_ready.rs`/`review.sh` alone (without trusting this blueprint's citations), reproduced one
mutation from §9 by hand, and confirmed a `Lesson` this crate's `derive_lesson` produces round-trips
cleanly through `validate_challenge_rows` with zero violations.

---

## Divergence / open questions (for Opus)

1. **Who actually shells out on this crate's behalf is unnamed.** `review.sh`'s `retry_limit`
   (a `node` subprocess reading a JS constant), `role_json` (a `roles.sh` subprocess), and the
   `verdict()` accept-branch's `ratchet.sh check`/`accept` calls are all excluded from this crate by
   design (§2/§5), but no crate in the 14-crate roster is explicitly named as their new home. The
   most likely owners by proximity — `fleet-worker` for `role_json` (it already owns CLI-driving,
   MIGRATION-PLAN row 13), `fleet-verify` for the ratchet check (row 8, "the re-run-every-check gate
   wall"), and `src/` itself for the one-off retry-policy read — are this blueprint's best guess,
   not a decision. Opus should confirm or reassign each of the three.

2. **Intent/SOW hash computation (`hash_file`'s `shasum -a 256`) has no named owner either.**
   `validate_sow_text` takes `intent_hash: &str` as a parameter (§3/§5) rather than computing it,
   consistent with this crate's zero-IO charter, but nothing yet says whether that hash is computed
   inline in `src/` per call or centralised in `fleet-store` alongside its own `blake3`
   content-hashing (row 2, `append_receipt`'s hash chain) — worth unifying under one crate rather
   than leaving two independent hash-computation call sites, but that unification is not this
   blueprint's call to make.

3. **`fleet-plan`'s two "no IO" siblings, `fleet-lifecycle` and `fleet-store`, are not imported
   today**, even though every `LifecycleState`/attempt-count/receipt-event-name this crate's fns
   take as a parameter is exactly the kind of value those two crates produce. This blueprint kept
   the edge list at `fleet-types`-only (per §1) on the theory that fleet-plan should stay a pure
   decision library the composition root wires together, never importing a crate only to read its
   *output* type — but if Opus judges that, say, `verdict_decision` should return a
   `fleet_types::Receipt`-shaped value directly (rather than the caller assembling one), that is a
   `fleet-types`-only addition and does not change this note; if Opus instead wants
   `submission_eligible` to accept a `&dyn ReceiptLedgerQuery` trait object *defined* in
   `fleet-store` (rather than a plain `attempts: u32` the caller already counted), that would add a
   new edge this blueprint did not take and should be flagged back into MIGRATION-PLAN §3 row 6's
   evidence column.

4. **`lld.rs`'s `cross_lang_report` (lines 829-921) and its filesystem-reading test harness role are
   deliberately NOT lifted into this crate's `src/`** (§5's explicit note) — only into a future
   `tests/` fixture-reading test, if this crate ends up being the Rust side of
   `fleet/tests/acceptance/lld-crosslang.sh`'s three-mirror comparison at all. Opus should confirm
   that comparator's Rust side belongs here (since `validate_lld_v1`/`canonical_json` do) rather
   than in a test-only tool outside the crate roster entirely — MIGRATION-PLAN does not currently
   name an owner for the cross-language comparator script itself.
