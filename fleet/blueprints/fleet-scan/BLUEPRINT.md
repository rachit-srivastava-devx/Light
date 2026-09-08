# BLUEPRINT — `fleet-scan`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-scan`
- **One-line purpose:** Given a requirement, run four read-only ambiguity probes concurrently,
  merge and deduplicate whatever questions they raise, and return either `Clear` or at most 4
  ranked questions — never more, never a silent guess — with zero probe ever able to block the
  other three.
- **Build branch:** `build-new` (MIGRATION-PLAN §3 row 5) — absent in fleet. **Name-clash warning
  (read §2): fleet's existing `fleet/scan.sh` is a SECURITY scanner (semgrep/gitleaks/trivy),
  owned by `fleet-verify`. `fleet-scan` shares no code, no config, and no output shape with it —
  the shared word "scan" is coincidental and must not cause anyone to merge or alias the two.**
- **Imports (Cargo-level):** `fleet-types` only.
- **Imports (named logical edges, not Cargo deps — see divergence note):** `fleet-context` (via
  this crate's own `CodebasePort` trait, §3), `fleet-memory` (via `MemoryPort`), `fleet-worker`
  (via `ConcurrentRunner` — this crate never spawns a thread/process itself). All three sibling
  crates are `todo` in MIGRATION-PLAN §5 as of this writing; this blueprint defines the minimal
  trait shape it needs from each so `fleet-scan` is hand-codeable and testable today, against
  mocks, without waiting on their blueprints. The real bridge from each crate's eventual public API
  to these traits is an adapter struct living in `src/` (composition root), not in this crate.
- **Imported by:** `fleet-plan` (runs the ambiguity gate before generating an LLD — MIGRATION-PLAN
  §3 row 6 does not currently name this edge; flagged in the divergence note), `src/` (composition
  root — CLI dispatch, JSON/human printing, wiring the real port adapters, receipt-write via
  `fleet-store`).

## 2. Responsibility & non-goals

**Owns:** the requirement-ambiguity probe pipeline: the `Probe` trait and its four
implementations (Business, Technical, Memory, Research); the merge/dedup/rank/cap algorithm that
turns however many raw candidate questions the probes raised into an ADHD-safe `Clear` or
`Open(≤4 questions)` verdict; and the typed `EnvFault` shape a probe reports instead of ever
propagating a panic or a raw IO error into the merge.

**Non-goals (the seam):**
- Does **not** scan for secrets, CVEs, or code-pattern vulnerabilities — that is `fleet/scan.sh`
  today and `fleet-verify` after extraction. `fleet-scan` never reads a diff for security findings
  and never shells out to semgrep/gitleaks/trivy. If anyone reaches for this crate to do that, it's
  the wrong crate — the name collision is coincidental (see §1).
- Does **not** spawn a thread, a process, or an async task itself — concurrency is injected via
  `ConcurrentRunner` (§3), supplied by the caller (ultimately backed by `fleet-worker`). This crate
  contains no `std::thread::spawn`, no `tokio::spawn`, no `rayon::join` anywhere in its own source.
- Does **not** read the filesystem, network, or environment directly. `TechnicalProbe`,
  `MemoryProbe`, and `ResearchProbe` reach the outside world only through the `CodebasePort`/
  `MemoryPort`/`ResearchPort` traits the caller supplies — this crate never opens a file, never
  makes an HTTP request, never reads an env var.
- Does **not** decide what to *do* about an open question (ask the human, block the task, auto-
  resolve it) — that is the caller's job (`fleet-plan`/`src/`). This crate only produces the
  `Assessment`; presenting it, waiting on an answer, or gating a downstream stage on it belongs
  upstream.
- Does **not** write receipts, ledger entries, or any persistent record of an assessment —
  `fleet-store`'s job, invoked by the caller after `assess()` returns.
- Does **not** retry a failed probe, apply backoff, or manage a timeout clock itself — a probe (in
  particular `ResearchProbe`) reports `EnvFault` the instant its port returns one; timeout/retry
  policy is `ConcurrentRunner`'s (i.e. `fleet-worker`'s) concern, injected, not owned here.

## 3. Public API contract

```rust
//! Read-only requirement-ambiguity probing: run 4 independent probes concurrently, merge and
//! dedup whatever questions they raise, and return an ADHD-safe verdict -- Clear, or at most 4
//! ranked questions. No probe may block the other 3: a probe that cannot complete (no network, no
//! credential, a caught panic) reports a typed `EnvFault` instead, and the merge proceeds with
//! whatever the other probes returned. This crate performs no IO of its own -- every fact about
//! the outside world (codebase lookups, memory recall, external research, and the concurrency to
//! run probes in parallel) arrives through a caller-supplied port trait.

use std::time::Duration;
use fleet_types::TaskId;

// =====================================================================================
// A. Input & probe identity
// =====================================================================================

/// The requirement text to probe, plus optional correlation metadata. This crate never mutates
/// or persists it -- `text` is read, nothing else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequirementInput {
    /// The raw requirement/task description as written by the human or upstream stage.
    pub text: String,
    /// Correlates this assessment with a task for receipt-writing by the caller. This crate never
    /// reads or writes anything keyed on it -- it is opaque pass-through metadata.
    pub task_id: Option<TaskId>,
}

/// Which of the 4 fixed probes produced a `Question` or `EnvFault`. Fixed set, not extensible at
/// runtime -- adding a 5th probe is a change to this enum and to `ProbeSet`, reviewed like any
/// other API change, not a runtime plugin registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeKind {
    Business,
    Technical,
    Memory,
    Research,
}

// =====================================================================================
// B. Typed environment failure -- never a panic, never a bare String
// =====================================================================================

/// Why a probe could not complete. A probe reports this instead of panicking or blocking; the
/// merge treats a faulted probe exactly like a probe that raised zero questions, and separately
/// surfaces the fault for observability (see `AssessmentReport::faults`) -- a fault is recorded,
/// never silently dropped, and never allowed to fail the other 3 probes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EnvFault {
    #[error("network unavailable: {0}")]
    NetworkUnavailable(String),
    #[error("required credential missing: {0}")]
    CredentialMissing(String),
    #[error("timed out after {0:?}")]
    TimedOut(Duration),
    #[error("rate limited, retry after {0:?}")]
    RateLimited(Duration),
    /// A probe panicked; the runner caught it (see `ConcurrentRunner`) and this crate converted
    /// it into data rather than letting the panic unwind past probe boundaries.
    #[error("probe panicked: {0}")]
    Internal(String),
}

// =====================================================================================
// C. Ports this crate needs from its siblings (named edges -- see header + divergence note)
// =====================================================================================

/// The subset of `fleet-context`'s eventual API this crate needs: "does this identifier already
/// exist in the current repo under some name" -- `TechnicalProbe` uses it to ask whether a
/// requirement's named thing is genuinely new or is ambiguously re-describing existing code.
pub trait CodebasePort: Send + Sync {
    fn symbol_exists(&self, name: &str) -> Result<bool, EnvFault>;
}

/// The subset of `fleet-memory`'s eventual API this crate needs: recall prior requirements/
/// decisions textually similar to this one, so `MemoryProbe` can ask "this looks like it
/// contradicts/duplicates a past decision -- which one governs?".
pub trait MemoryPort: Send + Sync {
    fn recall_similar(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>, EnvFault>;
}

/// One remembered item similar to the current requirement text.
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryHit {
    pub text: String,
    /// Similarity score as reported by `fleet-memory`'s own ranking; opaque to this crate beyond
    /// "higher is more similar" -- never compared for exact equality, only used to pick top-N.
    pub score: f32,
}

/// The subset of external research (web/docs lookup) `ResearchProbe` needs. This is the port most
/// likely to fault (§2) -- no network, no API key, no result found -- and its trait signature
/// reflects that every call is fallible in the ordinary case, not the exceptional one.
pub trait ResearchPort: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<String>, EnvFault>;
}

/// Runs the 4 probe jobs concurrently and returns their outcomes in the fixed
/// business/technical/memory/research order, regardless of completion order. Implemented by
/// (eventually) `fleet-worker`; this crate never spawns anything itself. An implementation MUST
/// isolate a panic in one job into `ProbeRun::Panicked` for that job alone -- it must never let a
/// panic in one job prevent the other 3 jobs' results from being returned.
pub trait ConcurrentRunner: Send + Sync {
    fn run4<'a>(&self, jobs: [ProbeJob<'a>; 4]) -> [ProbeRun; 4];
}

/// One probe invocation, boxed so `ConcurrentRunner` doesn't need to know each probe's concrete
/// type -- only that it is a `Send` closure producing a `ProbeOutcome` when run.
pub type ProbeJob<'a> = Box<dyn FnOnce() -> ProbeOutcome + Send + 'a>;

/// The result of running one `ProbeJob`: either it completed (with questions or a fault it
/// reported itself), or the runner caught a panic escaping it.
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeRun {
    Completed(ProbeOutcome),
    Panicked(String),
}

// =====================================================================================
// D. The Probe trait and its 4 fixed implementations
// =====================================================================================

/// One ambiguity-probing strategy. Every impl is read-only: it must not write to disk, mutate any
/// external state, or block waiting on anything not reachable through its own injected port.
pub trait Probe: Send + Sync {
    fn kind(&self) -> ProbeKind;
    /// Examine `input` and return zero or more candidate questions, or a fault if this probe's
    /// port could not be reached. Never panics by contract (a bug that panics is caught by
    /// `ConcurrentRunner`, not by this fn) and never blocks beyond its port's own call.
    fn probe(&self, input: &RequirementInput) -> ProbeOutcome;
}

/// What one `Probe::probe` call produced.
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeOutcome {
    Questions(Vec<Question>),
    Fault(EnvFault),
}

/// One candidate ambiguity question, before merge/dedup/rank/cap.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Question {
    pub probe: ProbeKind,
    pub text: String,
    /// Why this ambiguity matters -- mandatory content, not mandatory non-emptiness at
    /// construction (a probe may legitimately emit a malformed row; `merge_questions` is the
    /// enforcement boundary, see §6). A `Question` with an empty `why` is representable on
    /// purpose so the merge step has something concrete to drop and a test can assert it drops it.
    pub why: String,
    pub gap: GapSeverity,
    /// Optional grounding -- a file:line, a memory-hit id, a prior decision reference -- so a
    /// human reading the question can see where it came from. Never required.
    pub evidence: Option<String>,
}

/// How severe the ambiguity gap is. Ordered so `Blocking` sorts first (§6 "sort by gap").
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GapSeverity {
    Low,
    Medium,
    High,
    Blocking,
}

/// Reads `RequirementInput.text` for missing business-outcome signal (no stated success metric,
/// no stated audience). Pure heuristic text analysis -- no port, greenfield (§5/§7).
pub struct BusinessProbe;

/// Uses `CodebasePort` to check whether a named thing the requirement describes as new already
/// exists under some name in the current repo.
pub struct TechnicalProbe<'a> {
    pub codebase: &'a dyn CodebasePort,
}

/// Uses `MemoryPort` to recall prior requirements/decisions textually similar to this one and
/// flag apparent contradiction/duplication.
pub struct MemoryProbe<'a> {
    pub memory: &'a dyn MemoryPort,
}

/// Uses `ResearchPort` to check an external claim the requirement makes (a named library, a
/// competitor behavior, a standard). The probe most likely to fault -- see §2/§6.
pub struct ResearchProbe<'a> {
    pub research: &'a dyn ResearchPort,
}

impl Probe for BusinessProbe {
    fn kind(&self) -> ProbeKind { ProbeKind::Business }
    fn probe(&self, input: &RequirementInput) -> ProbeOutcome { unimplemented!() }
}
impl Probe for TechnicalProbe<'_> {
    fn kind(&self) -> ProbeKind { ProbeKind::Technical }
    fn probe(&self, input: &RequirementInput) -> ProbeOutcome { unimplemented!() }
}
impl Probe for MemoryProbe<'_> {
    fn kind(&self) -> ProbeKind { ProbeKind::Memory }
    fn probe(&self, input: &RequirementInput) -> ProbeOutcome { unimplemented!() }
}
impl Probe for ResearchProbe<'_> {
    fn kind(&self) -> ProbeKind { ProbeKind::Research }
    fn probe(&self, input: &RequirementInput) -> ProbeOutcome { unimplemented!() }
}

// =====================================================================================
// E. Merge: drop no-"why" -> Jaccard>0.6 dedup -> sort by gap -> cap 4 -> Clear|Open
// =====================================================================================

/// 1..=4 deduplicated, "why"-bearing questions, sorted by descending gap severity. The only way
/// to construct one is `merge_questions`, so an `Assessment::Open` can never carry 0 or >4 items
/// -- the illegal state (an "open" verdict with nothing to ask, or more than the ADHD-safe cap)
/// is unrepresentable by construction, not by convention.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct OpenQuestions(Vec<Question>);

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OpenQuestionsError {
    #[error("cannot construct OpenQuestions with 0 items -- use Assessment::Clear instead")]
    Empty,
    #[error("cannot construct OpenQuestions with more than 4 items ({0} given) -- caller must cap")]
    TooMany(usize),
}

impl OpenQuestions {
    pub fn new(items: Vec<Question>) -> Result<Self, OpenQuestionsError> { unimplemented!() }
    pub fn as_slice(&self) -> &[Question] { &self.0 }
    pub fn into_vec(self) -> Vec<Question> { self.0 }
}

/// The ADHD-safe verdict: either the requirement is clear enough to proceed, or here are at most
/// 4 ranked questions -- never a raw, unbounded list of every ambiguity a probe could imagine.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "verdict", rename_all = "lowercase")]
pub enum Assessment {
    Clear,
    Open(OpenQuestions),
}

/// Merge raw candidate questions from all 4 probes into an `Assessment`. Deterministic: the same
/// multiset of `Question`s in any input order always yields the same `Assessment` (same surviving
/// questions, same order) -- see §9 `merge_is_order_independent`.
///
/// Steps, always in this order: (1) drop every `Question` whose `why` is empty/whitespace-only --
/// a question with no stated stake is noise, not ambiguity. (2) pairwise Jaccard-similarity dedup
/// on word-tokenized `text` (case-folded, punctuation-stripped): any pair with similarity > 0.6
/// collapses to one, keeping the higher `gap` (tie: fixed probe priority
/// Business > Technical > Memory > Research, matching `ProbeKind`'s declaration order). (3) sort
/// the survivors by descending `gap`, ties broken by the same fixed probe priority, then by `text`
/// for total determinism. (4) truncate to the first 4. (5) empty result -> `Assessment::Clear`;
/// non-empty -> `Assessment::Open(OpenQuestions::new(..).expect("len is 1..=4 by construction"))`.
pub fn merge_questions(candidates: Vec<Question>) -> Assessment { unimplemented!() }

/// Word-tokenize (lowercase, strip non-alphanumeric) and compute Jaccard similarity
/// `|A∩B| / |A∪B|` between two strings' token sets. `0.0` if both are empty (defined, not NaN).
/// Pure, total, no allocation beyond the two token sets.
pub fn jaccard_similarity(a: &str, b: &str) -> f32 { unimplemented!() }

// =====================================================================================
// F. Orchestration: run all 4 probes via the injected runner, then merge
// =====================================================================================

/// The 4 probes bundled for one `assess` call. A struct (not 4 positional args) so the fixed
/// business/technical/memory/research order is named at every call site, not positional.
pub struct ProbeSet<'a> {
    pub business: &'a dyn Probe,
    pub technical: &'a dyn Probe,
    pub memory: &'a dyn Probe,
    pub research: &'a dyn Probe,
}

/// The full result of one `assess` call: the ADHD-safe verdict, plus every probe fault that
/// occurred along the way (for logs/observability -- never shown to the ADHD-safe surface, which
/// only ever sees `result`).
#[derive(Clone, Debug, PartialEq)]
pub struct AssessmentReport {
    pub result: Assessment,
    pub faults: Vec<(ProbeKind, EnvFault)>,
}

/// Run all 4 probes concurrently via `runner` (never spawning anything itself), collect whatever
/// questions/faults they produced, and merge per `merge_questions`. A fault or a caught panic in
/// any one probe (most likely `ResearchProbe`, see §2) contributes zero questions and one entry to
/// `faults` -- it never prevents the other 3 probes' questions from reaching the merge, and it
/// never turns into an `Err` returned to the caller: `assess` is infallible by design, matching
/// `fleet-router::decide`'s "refusal is data, not an error" shape.
pub fn assess(input: &RequirementInput, probes: &ProbeSet<'_>, runner: &dyn ConcurrentRunner) -> AssessmentReport {
    unimplemented!()
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `ProbeKind` | Exactly 4 variants, fixed set, `Ord`-derived in declaration order (Business < Technical < Memory < Research) — this order IS the tie-break priority used by `merge_questions` and by `ConcurrentRunner::run4`'s fixed array position. | A 5th ad-hoc probe being registered at runtime without a reviewed code change, or a tie-break whose priority silently drifts from the declared order. |
| `EnvFault` | A closed, named set of degrade reasons — no bare `String`/`anyhow::Error` anywhere a probe reports failure. | A probe failure reaching the merge as an untyped panic or an opaque error string a caller can't match on. |
| `Question.why` | Representable empty (probes aren't trusted to always fill it) but **enforced non-empty only at the `merge_questions` boundary** (dropped, not rejected at construction). | A malformed probe silently corrupting the whole `assess` call — `merge_questions` degrades gracefully by dropping just that row, not by failing the batch. |
| `OpenQuestions` | Smart-constructed only via `OpenQuestions::new`, which is the sole path `merge_questions` uses; length is always `1..=4`. | `Assessment::Open` carrying 0 items (should have been `Clear`) or more than 4 (violates the ADHD-safe cap) — both are impossible to construct, not just discouraged by convention. |
| `Assessment` | Exactly 2 variants, `Clear` xor `Open(OpenQuestions)` — no third "error" variant, matching `fleet-router::Decision`'s "outcome, not exception" shape. | A caller having to handle a bare `Result`/`Option` around ambiguity — there is no failure mode for `assess` itself, only the two expected verdicts. |
| `ProbeRun` | `Completed(ProbeOutcome)` xor `Panicked(String)` — a caught panic is data, never re-panics past the runner boundary. | A single buggy probe crashing the whole `assess` call and losing the other 3 probes' results. |
| `MemoryHit.score: f32` | A ranking score, never compared with `==` for identity/dedup logic (Jaccard on `text` is the dedup mechanism, not `score`). | Two `MemoryHit`s being treated as "the same" purely because of float equality — the crate never relies on float equality anywhere. |

**Money/precision:** no money type in this crate. `MemoryHit.score` is the only float in the crate
and is a similarity ranking score, not a precision-sensitive count — never used in an equality
check, arithmetic accumulation, or any money/token computation.

**Clock/RNG/IO injection points:** none owned by this crate. `ResearchProbe`/`TechnicalProbe`/
`MemoryProbe` each touch the outside world only through their injected port
(`ResearchPort`/`CodebasePort`/`MemoryPort`); `ConcurrentRunner` is the sole concurrency
injection point (this crate spawns nothing itself); no fn in this crate calls
`SystemTime::now()`/`Instant::now()`, reads an env var, or opens a file/socket directly. Any
timeout (`EnvFault::TimedOut`) is measured and enforced by the `ConcurrentRunner`/port
implementation, not by this crate — this crate only receives the already-decided fault as data.

## 5. Reuse map

`fleet-scan` is **greenfield** (MIGRATION-PLAN §3 row 5: absent in fleet; the only fleet artifact
sharing its name, `fleet/scan.sh`, is a different tool owned by `fleet-verify` — see §1/§2, no
logic is shared or lifted from it). Chosen approach and libraries:

| Concern | Chosen approach | Why |
|---|---|---|
| Concurrency | Trait-injected `ConcurrentRunner`, no crate dependency of its own | Keeps this crate's own dependency surface to `fleet-types`+`serde`+`thiserror`; the actual thread/task pool policy is a single owned concern in `fleet-worker`, not duplicated per consumer — mirrors `fleet-router`'s "caller computes `RuntimeState`" pattern (its blueprint, §5). |
| Text similarity (dedup) | Hand-rolled `jaccard_similarity` (tokenize + set intersection/union over `f32`) | The domain (short question strings, dozen-ish candidates) doesn't need a text-similarity crate (e.g. `strsim`); Jaccard on token sets is ~20 lines, fully unit-testable, and avoids a dependency whose edit-distance semantics don't match "same underlying ambiguity, different wording" as well as set overlap does. |
| Typed errors | `thiserror` | Already resolved in fleet's workspace lock (see `fleet-types`/`fleet-router` blueprints); this crate's error enums (`EnvFault`, `OpenQuestionsError`) are hand-total, no `anyhow`. |
| Serialization | `serde` (derive only) | `Question`/`Assessment`/`ProbeKind`/`GapSeverity` need `Serialize` for the caller's `--json` output and for receipt bodies, matching `fleet-router`'s `Decision`/`Stage`/`Refusal` precedent. |

## 6. Behavior spec

### `fn merge_questions(candidates: Vec<Question>) -> Assessment`

| Input dimension | Behavior |
|---|---|
| empty | `candidates = vec![]` → no step has anything to act on → `Assessment::Clear`. |
| null / `None` | n/a — `Vec<Question>`, not `Option`; an empty vec is the "none" case above. |
| wrong-type | n/a — `Question` is a typed struct; no stringly-typed input crosses this boundary. |
| huge | 4 probes each realistically emit a handful of questions (bounded by the requirement text's length in practice), but the algorithm is `O(n²)` in the dedup step (pairwise Jaccard) — documented as fine for expected `n` (single digits to low tens); a pathological probe emitting thousands of questions is a probe bug, not something this fn silently handles efficiently, and is exactly what §9's `merge_caps_regardless_of_input_size` property test guards: correctness (cap at 4) is preserved even if performance degrades. |
| negative | n/a — no numeric input to this fn. |
| duplicate | Two (or more) `Question`s with `jaccard_similarity(a.text, b.text) > 0.6` collapse to one, keeping the higher `gap` (tie broken by `ProbeKind` priority order, then lexicographically by `text` for total determinism) — never silently keeps both, never silently keeps the wrong one. |
| concurrent | Pure fn over an owned `Vec` — no shared state, trivially safe to call from multiple threads with distinct inputs. |
| unicode / non-ASCII | `jaccard_similarity` tokenizes on Unicode word boundaries after lowercasing (via `char::is_alphanumeric`, not ASCII-only) — a non-ASCII question compares correctly; no panic, no mojibake, no silent ASCII-only truncation. |
| already-exists | Calling `merge_questions` twice with the identical input vec (in any permutation) yields an identical `Assessment` — this is a required property (§9 `merge_is_order_independent`), not merely allowed. |
| partial-failure | n/a — this fn has no IO and cannot fail partially; it always returns a complete `Assessment` synchronously. |

### `fn assess(input: &RequirementInput, probes: &ProbeSet<'_>, runner: &dyn ConcurrentRunner) -> AssessmentReport`

| Input dimension | Behavior |
|---|---|
| empty | `input.text = ""` → every probe receives it and is free to report zero questions (most will); `merge_questions(vec![])` → `Assessment::Clear`; `faults` empty unless a port itself faults on empty input (port's choice, not this fn's). |
| null / `None` | `input.task_id = None` → passed through unread; no behavior change (this fn never branches on it). |
| wrong-type | n/a — `RequirementInput`/`ProbeSet`/`&dyn ConcurrentRunner` are typed; no stringly-typed input. |
| huge | `input.text` at 100x-1000x expected length: this fn does not itself scan the text (probes do) — it is a fixed-size (4-job) dispatch regardless of `text`'s size; a slow probe on huge input is bounded by whatever timeout `ConcurrentRunner`/the port enforces (this crate's own dispatch overhead is `O(1)` in `text`'s size). |
| negative | n/a — no numeric input. |
| duplicate | Calling `assess` twice with an identical `input` (and deterministic probes/ports) yields an identical `AssessmentReport` — required by the same determinism property as `merge_questions`, composed with whatever determinism the probes/ports themselves provide (this crate does not introduce non-determinism of its own; `ConcurrentRunner`'s completion-order independence is exactly why `run4` returns a fixed-position array, not arrival order). |
| concurrent | `assess` itself holds no shared mutable state; it is safe to call from multiple threads with distinct `ProbeSet`/`ConcurrentRunner` values. Whether two probes sharing one `&dyn CodebasePort`/etc. are safe to call concurrently is that port implementation's contract (`Send + Sync` bound on every port trait makes this an explicit, checked requirement, not an assumption). |
| unicode / non-ASCII | Passed through to probes/`merge_questions` unchanged; see `merge_questions`' row above. |
| already-exists | n/a — no persisted state; each `assess` call is independent. |
| partial-failure | The core case this fn exists to handle: if `ResearchProbe` (or any probe) faults or panics, `assess` still returns a complete `AssessmentReport` built from the other 3 probes' `Questions`, with the failure recorded as one `(ProbeKind, EnvFault)` entry in `faults` — never propagated as an `Err`, never silently dropped, and never allowed to reduce the other 3 probes' contribution to the merge. This is verified by §9's `one_faulted_probe_never_blocks_the_others`. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path dependency (`{ path = "../fleet-types" }`) | Supplies `TaskId` for `RequirementInput`'s correlation metadata. |
| `serde` | `1.0` (match `fleet/keel/fleet/Cargo.lock`'s resolved version) | `Question`/`Assessment`/`ProbeKind`/`GapSeverity`/`OpenQuestions` derive `Serialize` for `--json` output and receipt bodies. |
| `thiserror` | `2.0.20` (match `fleet-types`' pinned version — see its blueprint §7) | `EnvFault`/`OpenQuestionsError` are typed, total error enums — no `String`/`anyhow::Error` anywhere fallible. |

No dependency on `fleet-context`/`fleet-memory`/`fleet-worker` at the Cargo level (see header +
divergence note) — those three crates are named only through this crate's own port traits.

## 8. Crate file layout

```
crates/fleet-scan/
  Cargo.toml
  src/
    lib.rs          # ~20 — module decls + re-exports only
    input.rs        # ~25 — RequirementInput, ProbeKind
    fault.rs         # ~25 — EnvFault
    ports.rs         # ~55 — CodebasePort, MemoryPort, MemoryHit, ResearchPort, ConcurrentRunner, ProbeJob, ProbeRun
    probe.rs         # ~30 — Probe trait, ProbeOutcome, Question, GapSeverity
    business.rs       # ~60 — BusinessProbe (heuristic text checks, no port)
    technical.rs     # ~55 — TechnicalProbe (uses CodebasePort)
    memory.rs        # ~55 — MemoryProbe (uses MemoryPort)
    research.rs       # ~55 — ResearchProbe (uses ResearchPort; the most fault-prone)
    jaccard.rs        # ~40 — tokenize() + jaccard_similarity()
    merge.rs          # ~75 — OpenQuestions, OpenQuestionsError, Assessment, merge_questions()
    assess.rs         # ~65 — ProbeSet, AssessmentReport, assess() orchestration
  tests/
    probes.rs           # ~70 — one behavior-spec case per Probe impl, against mock ports
    merge_rules.rs       # ~75 — drop-no-why / dedup / sort / cap behavior from §6, §9
    assess_pipeline.rs   # ~70 — end-to-end assess() incl. fault-isolation and determinism
```
> If any file above still projects over 80 lines once bodies land, split again (e.g. `merge.rs` →
> `open_questions.rs` + `merge_questions.rs`). The file-size gate (§10) is run before Opus review.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-scan"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0.20"
fleet-types = { path = "../fleet-types" }

[dev-dependencies]
serde_json = "1.0"
```

## 9. Test plan

**Unit tests:**
- `jaccard_similarity_is_symmetric_and_bounded` — `jaccard_similarity(a, b) == jaccard_similarity(b, a)`, always in `[0.0, 1.0]`, and `jaccard_similarity("", "") == 0.0` (not NaN).
- `jaccard_similarity_handles_unicode_tokens` — a question phrased with non-ASCII words tokenizes and compares correctly (no panic, no ASCII-only silent truncation), per §6's unicode row.
- `open_questions_rejects_empty` — `OpenQuestions::new(vec![])` is `Err(OpenQuestionsError::Empty)`.
- `open_questions_rejects_more_than_four` — `OpenQuestions::new(vec![...; 5])` is `Err(OpenQuestionsError::TooMany(5))`.
- `open_questions_accepts_one_to_four` — `OpenQuestions::new` succeeds for lengths 1, 2, 3, 4.
- `merge_drops_empty_why` — a candidate with `why: "".into()` never appears in the resulting `Assessment::Open`'s items, and if it was the only candidate, the result is `Assessment::Clear`.
- `merge_dedups_above_jaccard_point_six` — two near-identical-wording `Question`s (similarity > 0.6) collapse to one, keeping the higher `gap`; two clearly distinct questions (similarity ≤ 0.6) both survive.
- `merge_sorts_by_descending_gap_severity` — a mixed-severity input list comes out `Blocking, High, Medium, Low` ordered.
- `merge_caps_at_four` — 10 distinct, non-dedupable, all-`why`-bearing candidates yield exactly 4 in the result, the 4 highest-`gap` ones.

**Integration tests** (calling only the public API):
- `merge_is_order_independent` — feeding the same multiset of `Question`s in every permutation of a representative small set yields byte-identical `Assessment` output each time (property test, §9 below covers the generalized form; this is the fixed-example version).
- `one_faulted_probe_never_blocks_the_others` — a `ProbeSet` where `research` is wired to a `ResearchPort` mock returning `Err(EnvFault::NetworkUnavailable(..))` (so `ResearchProbe::probe` returns `ProbeOutcome::Fault(..)`), while business/technical/memory return real questions — assert `AssessmentReport.result` reflects the 3 working probes' merged questions AND `faults` contains exactly one `(ProbeKind::Research, EnvFault::NetworkUnavailable(..))` entry.
- `panicking_probe_is_isolated_by_the_runner` — a `ConcurrentRunner` test-double whose `run4` simulates one job panicking (returns `ProbeRun::Panicked("boom".into())` for that slot) — assert `assess` still returns a complete `AssessmentReport`, that slot contributes an `EnvFault::Internal("boom")` fault, and the other 3 slots' questions reach the merge unaffected.
- `all_four_clear_yields_clear` — every probe returns `ProbeOutcome::Questions(vec![])` → `AssessmentReport.result == Assessment::Clear`, `faults` empty.
- `assess_never_returns_more_than_four_questions_end_to_end` — all 4 probes wired to return several plausible, non-dedupable, `why`-bearing questions each (more than 4 total) → `AssessmentReport.result` is `Assessment::Open` with `len() <= 4`.

**Mutation-testing targets** (`cargo mutants -p fleet-scan`):
- Flipping the Jaccard dedup threshold comparison from `> 0.6` to `>= 0.6` (or `<`) must be killed by `merge_dedups_above_jaccard_point_six`'s boundary case (construct two questions whose similarity is exactly at the boundary alongside the existing above/below cases).
- Flipping the cap from `.truncate(4)` to `.truncate(5)` (or removing it) must be killed by `merge_caps_at_four` and `assess_never_returns_more_than_four_questions_end_to_end`.
- Deleting the empty-`why` filter must be killed by `merge_drops_empty_why`.
- Swapping the sort direction (ascending instead of descending `gap`) must be killed by `merge_sorts_by_descending_gap_severity`.
- Making a probe fault propagate as a hard `panic!`/`Result::Err` out of `assess` instead of populating `faults` must be killed by `one_faulted_probe_never_blocks_the_others` and `panicking_probe_is_isolated_by_the_runner`.

**Property tests** (`proptest`, ≥100 cases):
- `merge_is_order_independent_generalized` — for any `Vec<Question>` (bounded arbitrary generator: 0-20 items, `why` sometimes empty, `text` drawn from a small vocabulary to force some dedup collisions), `merge_questions(v.clone())` equals `merge_questions(shuffled(v))` for any permutation.
- `merge_output_never_exceeds_four_and_never_zero_unless_clear` — for any generated input, the resulting `Assessment` is either `Clear` or `Open` with `1 <= len <= 4` — the illegal states from §4 never appear, generated adversarially.

## 10. Verification recipe

```bash
cd crates/fleet-scan
cargo test -p fleet-scan --all-targets
cargo clippy -p fleet-scan --all-targets -- -D warnings
cargo mutants -p fleet-scan
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration + property tests pass, 0 skipped — publish as `<passed>/<total>`
(e.g. `18/18`, never just "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9
caught — publish `<caught>/<total mutants>`; given this crate's small, pure surface, the floor is
**100% of viable mutants caught**, matching `fleet-router`'s precedent (its blueprint §10) — any
survivor gets a new test, not a lowered floor.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`EnvFault`, `OpenQuestionsError`) — none
      swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code. `assess`/
      `merge_questions` themselves are infallible by design (an ambiguity verdict is an expected
      outcome, not an exception), matching `fleet-router::decide`'s precedent.
- [x] Clock/RNG/IO are injected, never read ambient inside logic — `CodebasePort`/`MemoryPort`/
      `ResearchPort`/`ConcurrentRunner` are the only 4 boundaries to the outside world, all
      caller-supplied traits (§4).
- [x] Thread-safety documented: every port trait and `Probe` itself requires `Send + Sync`;
      `merge_questions`/`jaccard_similarity` take owned/`&` values with no interior mutability —
      `Send + Sync` for free, safe to call concurrently.
- [x] No float used for money, tokens, or any precision-sensitive count — the only float in the
      crate, `MemoryHit.score`, is a similarity ranking, never compared by `==` for logic (§4).
- [ ] No self-grading — verification runs `cargo mutants` and the property tests, not just the
      named unit tests; denominator published per §10 (mark done once actually run and recorded).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate
      the real numbers in the PR once the crate is built (mark done then).
- [ ] Tests that touch the filesystem: none needed — every port is mocked in-memory in tests; if a
      future test needs a temp file it must use `tempdir()`, never the repo tree.
- [ ] Every non-goal in §2 is absent from the code — no `std::thread::spawn`/`tokio::spawn`/
      `rayon::` call, no `std::fs`/`std::net`/`env::var` call anywhere in
      `crates/fleet-scan/src/`; enforce with `grep -rn 'thread::spawn\|tokio::spawn\|std::fs\|env::var\|reqwest\|std::net' crates/fleet-scan/src/` returning nothing.
- [ ] No source file exceeds 80 lines — §8's split verified by the §10 `wc -l ... awk '$1>80'` gate
      before Opus review.

## 12. Definition of Done

`fleet-scan` is DONE when: §10's four commands all pass with a published denominator (tests `N/N`,
clippy clean, mutants `M/M` caught, file-size gate silent) run from `crates/fleet-scan/`; every
unchecked box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` (or
`features/REGISTRY.md` per C1/L2 — ambiguity-probing is closer to a shared platform service than a
feature; likely `services/`) lists the crate; and Opus has re-derived the merge algorithm
(drop-no-why → Jaccard>0.6 dedup → sort-by-gap → cap-4 → Clear|Open) from this blueprint alone,
reproduced the Jaccard-threshold mutation by hand, and driven one real `assess()` call end-to-end
(with a genuinely unreachable `ResearchPort` mock) confirming the other 3 probes' questions still
surface and the fault is recorded, not swallowed or blocking.

---

## Divergence from MIGRATION-PLAN (for Opus)

1. **Sibling dependency inversion.** MIGRATION-PLAN §3 row 5 and the brief that produced this
   blueprint both describe `fleet-scan` as depending on `fleet-context`/`fleet-memory` and calling
   *through* `fleet-worker` for concurrency, which reads as literal Cargo path-dependencies. All
   three of those crates are `todo` in MIGRATION-PLAN §5 — none has a blueprint yet, so their real
   public API shape is unknown. Coding `fleet-scan` against a guessed real API today risks a
   rewrite the moment those blueprints land. This blueprint instead defines the *minimal* trait
   each sibling must eventually satisfy (`CodebasePort`, `MemoryPort`, `ConcurrentRunner`) inside
   `fleet-scan` itself, with **no Cargo dependency on any of the three** — the bridge from each
   real crate's eventual API to these traits is an adapter written in `src/` (composition root)
   once those crates exist. This mirrors the already-Opus-approved `fleet-router` pattern (its
   blueprint's `RuntimeState` is caller-computed, not read by `fleet-router` itself). **Action for
   Opus:** either bless this inversion (my recommendation — it unblocks building `fleet-scan` now,
   in parallel with `fleet-context`/`fleet-memory`/`fleet-worker`, consistent with P3's "build-new
   after fast wins" but not blocked by a specific ordering among build-new crates), or, once those
   3 blueprints exist, reconcile this crate's port traits against their real exported types and
   flip to literal Cargo deps at that point — either way, note the decision in §7 (teach-back).
2. **`fleet-plan` as an importer not currently named.** MIGRATION-PLAN §3 row 6 (`fleet-plan`)
   doesn't cite `fleet-scan` as a dependency, but an ambiguity gate logically runs before an LLD is
   generated (the brief's own framing — "run read-only probes... return either Clear or ≤4
   questions for the user" — is a pre-planning gate). This blueprint's header lists `fleet-plan` as
   an importer; MIGRATION-PLAN §3 row 6's evidence column should add this edge explicitly so the
   DAG documentation and this blueprint don't silently diverge.
3. **Name clash restated as a hard non-goal, not just a note.** The brief flagged that
   `fleet/scan.sh` (security scanner, → `fleet-verify`) is a different concern from this crate. This
   blueprint makes that explicit in §1 and §2 as a non-goal with a grep-able boundary check (§11),
   since a future agent skimming crate names is the most likely way this collision causes real
   confusion (an agent reading MIGRATION-PLAN row 5 in isolation, without this file, could plausibly
   assume `fleet-scan` extracts `scan.sh`).
