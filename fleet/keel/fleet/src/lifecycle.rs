//! Compile-time lifecycle for a fleet task.
//!
//! A state is represented by the type parameter of [`Task`]. Legal transitions
//! consume the old task, append a receipt, and return a task with a new state.

use serde_json::{json, Value};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

const EXIT_ENVIRONMENT: i32 = 3;
const EXIT_REFUSAL: i32 = 7;

const STATES: &[(&str, &[&str])] = &[
    ("Intake", &["Specified", "Refused"]),
    ("Specified", &["Reviewed", "Refused"]),
    ("Reviewed", &["Decomposed"]),
    ("Decomposed", &["Contracted"]),
    ("Contracted", &["Briefed"]),
    ("Briefed", &["Leased"]),
    ("Leased", &["Building"]),
    ("Building", &["Built", "Refused"]),
    ("Built", &["Verifying", "Refused"]),
    ("Verifying", &["Verified", "Refused"]),
    ("Verified", &["Attested"]),
    ("Attested", &["Accepted", "Refused"]),
    ("Accepted", &["Proposed", "Refused"]),
    ("Proposed", &["Observed"]),
    ("Observed", &["Intake"]),
    ("Refused", &[]),
];

mod sealed {
    pub trait Sealed {}
}

/// Marker implemented only by lifecycle states declared in this module.
pub trait State: sealed::Sealed {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intake;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Specified;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reviewed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decomposed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contracted;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Briefed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leased;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Building;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Built;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Verifying;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Verified;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attested;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Accepted;
/// F08: the change has been pushed and a real pull request opened, carrying the attested
/// diff. Not `Merged` or `Landed` -- the change is not delivered, it is proposed for human
/// integration. Every existing state is a past participle (`Attested`, `Accepted`,
/// `Observed`); `Proposed` fits the vocabulary and states the human-merge boundary in the
/// name itself. This is the last state an agent can reach on its own: there is no `merge`
/// method and never will be (author != integrator, FLEET-LEARNINGS.md "SDLC gate model").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Proposed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Observed;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refused;

macro_rules! seal_states {
    ($($state:ident),+ $(,)?) => {
        $(
            impl sealed::Sealed for $state {}
            impl State for $state {}
        )+
    };
}

seal_states!(
    Intake, Specified, Reviewed, Decomposed, Contracted, Briefed, Leased, Building, Built,
    Verifying, Verified, Attested, Accepted, Proposed, Observed, Refused,
);

/// Stable identifier carried through every transition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Result<Self, Refusal> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(Refusal::new("EMPTY_TASK_ID", "a task id must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A transition receipt awaiting parent-side timestamp and actor stamping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionReceipt {
    pub task_id: TaskId,
    pub from: &'static str,
    pub to: &'static str,
    pub evidence: String,
}

/// The only capability a lifecycle transition needs from the evidence ledger.
pub trait ReceiptLedger {
    fn append(&self, receipt: TransitionReceipt) -> Result<(), Refusal>;
}

/// A failed gate or failed attempt to append its receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    code: &'static str,
    message: String,
}

impl Refusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Refusal {}

/// Proof that a human, rather than an agent, approved a gated edge.
///
/// Only code in this crate can mint this token. The daemon's human approval
/// boundary will call this constructor; callers of the public library cannot.
#[derive(Debug)]
pub struct HumanApproval {
    evidence: String,
}

impl HumanApproval {
    #[allow(dead_code)]
    pub(crate) fn recorded(evidence: impl Into<String>) -> Result<Self, Refusal> {
        let evidence = evidence.into();
        if evidence.trim().is_empty() {
            return Err(Refusal::new(
                "EMPTY_HUMAN_APPROVAL",
                "human approval evidence must not be empty",
            ));
        }
        Ok(Self { evidence })
    }
}

/// A task whose legal operations are determined by `S`.
#[derive(Debug)]
pub struct Task<S: State> {
    id: TaskId,
    retry_depth: u32,
    _s: PhantomData<S>,
}

impl Task<Intake> {
    pub fn new(id: TaskId) -> Self {
        Self {
            id,
            retry_depth: 0,
            _s: PhantomData,
        }
    }

    pub fn specify(
        self,
        evidence: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Specified>, Refusal> {
        self.transition(evidence, ledger)
    }

    pub fn refuse(
        self,
        reason: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Refused>, Refusal> {
        self.transition(reason, ledger)
    }
}

impl Task<Specified> {
    pub fn review(
        self,
        approval: HumanApproval,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Reviewed>, Refusal> {
        self.transition(approval.evidence, ledger)
    }

    pub fn refuse(
        self,
        reason: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Refused>, Refusal> {
        self.transition(reason, ledger)
    }
}

macro_rules! edge {
    ($from:ident, $method:ident, $to:ident) => {
        impl Task<$from> {
            pub fn $method(
                self,
                evidence: impl Into<String>,
                ledger: &impl ReceiptLedger,
            ) -> Result<Task<$to>, Refusal> {
                self.transition(evidence, ledger)
            }
        }
    };
}

edge!(Reviewed, decompose, Decomposed);
edge!(Decomposed, contract, Contracted);
edge!(Contracted, brief, Briefed);
edge!(Briefed, lease, Leased);
edge!(Leased, build, Building);
edge!(Building, finish_build, Built);
edge!(Built, begin_verification, Verifying);
edge!(Verifying, verify, Verified);
edge!(Verified, attest, Attested);
edge!(Proposed, observe, Observed);
edge!(Observed, reopen, Intake);

macro_rules! refusal_edge {
    ($from:ident) => {
        impl Task<$from> {
            pub fn refuse(
                self,
                reason: impl Into<String>,
                ledger: &impl ReceiptLedger,
            ) -> Result<Task<Refused>, Refusal> {
                self.transition(reason, ledger)
            }
        }
    };
}

refusal_edge!(Building);
refusal_edge!(Built);
refusal_edge!(Verifying);
refusal_edge!(Attested);
refusal_edge!(Accepted);

impl Task<Attested> {
    pub fn accept(
        self,
        approval: HumanApproval,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Accepted>, Refusal> {
        self.transition(approval.evidence, ledger)
    }
}

/// The only capability the `propose` edge needs from the outside world: push the branch and
/// open a pull request. Lives here (not a direct `git`/`gh` shell-out from this module)
/// for the same reason `ReceiptLedger` does -- `lifecycle.rs` is in the **lib** crate and
/// `main.rs` is a **bin** that depends on it (`main.rs:14`, `main.rs:53`); the dependency
/// cannot be inverted, so this module cannot shell out to `git`/`gh` itself, and the
/// production implementation (real worktree, real push, real `gh pr create`) lives in
/// `main.rs` instead.
pub trait ChangeEmitter {
    /// Push `request.head` and open a pull request. MUST return the real PR URL -- an
    /// emitter that "succeeded" without one must return `Err`, never `Ok` (`PR_URL_ABSENT`:
    /// exit 0 from `gh` is not a pull request).
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, Refusal>;
}

/// Everything `propose` needs to ask a [`ChangeEmitter`] to open a pull request for the
/// attested diff. `diff` carries the actual attested bytes (read by the caller from
/// `$FLEET_STATE/artifacts/<artifact_id>`) so `propose` can verify the digest itself rather
/// than trusting that whoever built this request already checked it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalRequest {
    pub repo: PathBuf,
    pub base: String,
    pub head: String,
    pub artifact_id: String,
    pub diff: Vec<u8>,
    pub title: String,
    pub body: String,
}

/// The extracted `work_landed:{branch,commit,status,changed_files}` record from
/// `dispatch.sh:1052-1057`, typed: what a successful `propose` actually did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedChange {
    pub url: String,
    pub head: String,
    pub commit: String,
    pub changed_files: u64,
}

/// The elements of a delivery attestation the `propose` edge must find complete before it
/// will emit a pull request. Built from the attestation's `predicate.elements` object (see
/// `main.rs`'s `run_with_evidence` / `attest_verify_inner`) -- this type does not re-derive
/// that trust (`attest_verify_inner` remains the one real validator, and `fleet pr emit`
/// still runs it), it only names WHICH element is missing when the gate refuses, which an
/// opaque exit code cannot.
#[derive(Clone, Debug)]
pub struct AttestationBundle {
    elements: Value,
}

impl AttestationBundle {
    /// `elements` is the attestation's `predicate.elements` object.
    pub fn new(elements: Value) -> Self {
        Self { elements }
    }

    /// The pinned set `attest_verify_inner` enforces today (`main.rs:4062-4074`) -- exactly
    /// these 8, not the aspirational 9th (F08 contract §8.2: "the 9-element attestation is
    /// aspirational; 8 is what the code enforces today"). `pinned_element_names_match_the_
    /// enforced_set` fails on sight if a 9th element ever lands, forcing a conscious update
    /// instead of silent drift.
    const REQUIRED: [&'static str; 8] = [
        "sow",
        "blind_suite",
        "independent_verification",
        "adequacy",
        "blast_radius",
        "rollback",
        "cost",
        "oracle_independence",
    ];

    /// The first required element that is missing or structurally incomplete, or `None` if
    /// every element is present. Presence alone is not the property `attest_verify_inner`
    /// enforces (`main.rs:3992-4059`): the three elements with a documented deep shape
    /// (`independent_verification`, `oracle_independence`, `blind_suite`) are checked to that
    /// same depth here. The other five need only be present -- their deeper cross-checks
    /// (e.g. `blast_radius` against the actual diff bytes) need the raw artifact, which this
    /// type deliberately never reads; `attest_verify_inner` still runs that full check before
    /// `fleet pr emit` ever calls into this edge.
    pub fn missing_element(&self) -> Option<&'static str> {
        let object = self.elements.as_object();
        for name in Self::REQUIRED {
            let complete = match object.and_then(|map| map.get(name)) {
                None => false,
                Some(value) => match name {
                    "independent_verification" => independent_verification_is_complete(value),
                    "oracle_independence" => oracle_independence_is_complete(value),
                    "blind_suite" => blind_suite_is_complete(value),
                    _ => !value.is_null(),
                },
            };
            if !complete {
                return Some(name);
            }
        }
        None
    }
}

fn independent_verification_is_complete(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 5
        && object.get("builder").and_then(Value::as_str).is_some()
        && object.get("verifier").and_then(Value::as_str).is_some()
        && object.get("distinct") == Some(&Value::Bool(true))
        && object.get("reproduced").and_then(Value::as_bool) == Some(true)
        && object.get("verdict").and_then(Value::as_str) == Some("ACCEPT")
}

fn oracle_independence_is_complete(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 6
        && object.get("o1_author").and_then(Value::as_str).is_some()
        && object.get("o2_author").and_then(Value::as_str).is_some()
        && object
            .get("o1_hash")
            .and_then(Value::as_str)
            .is_some_and(is_valid_artifact_id)
        && object
            .get("o2_hash")
            .and_then(Value::as_str)
            .is_some_and(is_valid_artifact_id)
        && object.get("distinct") == Some(&Value::Bool(true))
        && object
            .get("quadrant")
            .and_then(Value::as_str)
            .is_some_and(|quadrant| {
                [
                    "ACCEPT",
                    "ORACLE_INADEQUATE",
                    "ORACLE_OVERCONSTRAINED",
                    "BUILDER_FAULT",
                ]
                .contains(&quadrant)
            })
}

fn blind_suite_is_complete(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 5
        && object
            .get("in_worktree_tree")
            .and_then(Value::as_bool)
            .is_some()
        && object
            .get("in_object_store")
            .and_then(Value::as_bool)
            .is_some()
        && object.get("in_env").and_then(Value::as_bool).is_some()
        && object.get("on_any_fd").and_then(Value::as_bool).is_some()
        && object.get("suite_hash").and_then(Value::as_str).is_some()
}

/// Mirrors `main.rs`'s `valid_artifact_id`. `lifecycle.rs` cannot call it (a bin cannot be a
/// lib dependency; see `ChangeEmitter`'s doc comment) and this is two lines of format
/// checking, not a second attestation validator -- the deep shape checks above are the ones
/// the F08 contract calls out as needing to be "carried forward", not this.
fn is_valid_artifact_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl Task<Accepted> {
    /// `Accepted -> Proposed`: emit a real pull request carrying the attested diff.
    ///
    /// This is the agent's terminal act. There is no `merge` edge and no `Merged` state:
    /// author != integrator (`FLEET-LEARNINGS.md`, SDLC gate model). Merging is a human
    /// action on the forge, under branch protection -- never a lifecycle transition.
    ///
    /// Gate order is deliberate: attestation completeness, then the artifact digest, then
    /// emptiness, all BEFORE `emitter.emit` runs -- the gate must run before the side effect,
    /// not after (a refused proposal must push nothing and must never invoke `gh`).
    pub fn propose(
        self,
        bundle: &AttestationBundle,
        request: &ProposalRequest,
        emitter: &impl ChangeEmitter,
        ledger: &impl ReceiptLedger,
    ) -> Result<(Task<Proposed>, ProposedChange), Refusal> {
        if let Some(missing) = bundle.missing_element() {
            return Err(Refusal::new(
                "INCOMPLETE_ATTESTATION",
                format!("attestation is missing required element: {missing}"),
            ));
        }
        // Defense in depth, not a duplicate validator: the caller (main.rs) already re-derives
        // and checks this digest before it ever builds a `ProposalRequest`, but the compiled
        // edge must refuse on its own too -- that is the whole point of a typed gate over a
        // caller's discipline (F08 contract §3.1). Uses the `blake3` crate directly (as
        // `safe_task_name` above already does): `blake3_hex` is a bin-only wrapper this module
        // cannot call.
        let digest = blake3::hash(&request.diff).to_hex().to_string();
        if digest != request.artifact_id {
            return Err(Refusal::new(
                "ARTIFACT_DIGEST_MISMATCH",
                format!(
                    "artifact {} does not match the frozen diff bytes (recomputed {digest})",
                    request.artifact_id
                ),
            ));
        }
        if request.diff.is_empty() {
            return Err(Refusal::new(
                "EMPTY_DIFF",
                "a run that changes nothing must not produce an attested artifact",
            ));
        }
        let change = emitter.emit(request)?;
        let proposed: Task<Proposed> =
            self.transition(format!("opened {}", change.url), ledger)?;
        Ok((proposed, change))
    }
}

impl<S: State> Task<S> {
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    pub fn retry_depth(&self) -> u32 {
        self.retry_depth
    }

    fn transition<N: State>(
        self,
        evidence: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<N>, Refusal> {
        let evidence = evidence.into();
        if evidence.trim().is_empty() {
            return Err(Refusal::new(
                "EMPTY_TRANSITION_EVIDENCE",
                "a lifecycle transition requires evidence",
            ));
        }
        ledger.append(TransitionReceipt {
            task_id: self.id.clone(),
            from: std::any::type_name::<S>(),
            to: std::any::type_name::<N>(),
            evidence,
        })?;
        Ok(Task {
            id: self.id,
            retry_depth: self.retry_depth,
            _s: PhantomData,
        })
    }
}

fn state_dir() -> Result<PathBuf, Refusal> {
    let value = std::env::var_os("FLEET_STATE").ok_or_else(|| {
        Refusal::new(
            "MISSING_FLEET_STATE",
            "$FLEET_STATE must name the state directory",
        )
    })?;
    if value.is_empty() {
        return Err(Refusal::new(
            "MISSING_FLEET_STATE",
            "$FLEET_STATE must not be empty",
        ));
    }
    Ok(PathBuf::from(value))
}

fn safe_task_name(id: &TaskId) -> String {
    // D51: this sanitised the task text but did not BOUND it, so the filename grew with the task.
    // A structured SOW (leaves, challenges, alternatives, estimates, edge cases) is hundreds of
    // characters and the dispatch died with "File name too long (os error 63)" -- on the one input
    // the SOW gate exists to require. Keep a readable prefix, then a hash of the FULL id so two
    // long tasks sharing a prefix never collide.
    let sanitised: String = id
        .as_str()
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => byte as char,
            _ => '_',
        })
        .collect();
    const PREFIX: usize = 48;
    if sanitised.len() <= PREFIX {
        return sanitised;
    }
    let digest = blake3::hash(id.as_str().as_bytes()).to_hex();
    let head: String = sanitised.chars().take(PREFIX).collect();
    format!("{head}-{}", &digest[..16])
}

fn task_path(root: &Path, id: &TaskId) -> PathBuf {
    root.join("lifecycle")
        .join(format!("{}.state", safe_task_name(id)))
}

struct FileLedger {
    root: PathBuf,
}

impl ReceiptLedger for FileLedger {
    fn append(&self, receipt: TransitionReceipt) -> Result<(), Refusal> {
        fs::create_dir_all(&self.root).map_err(io_refusal)?;
        let path = self.root.join("lifecycle-receipts.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(io_refusal)?;
        writeln!(
            file,
            "{{\"task_id\":\"{}\",\"from\":\"{}\",\"to\":\"{}\",\"evidence\":\"{}\"}}",
            json_escape(receipt.task_id.as_str()),
            short_state(receipt.from),
            short_state(receipt.to),
            json_escape(&receipt.evidence),
        )
        .map_err(io_refusal)
    }
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            other => vec![other],
        })
        .collect()
}

fn short_state(value: &str) -> &str {
    value.rsplit("::").next().unwrap_or(value)
}

fn io_refusal(error: std::io::Error) -> Refusal {
    Refusal::new("LIFECYCLE_IO", error.to_string())
}

/// Read a task's persisted runtime state (`"Intake"` if nothing has been written yet).
/// Public so `main.rs`'s `fleet pr emit` can check "is this task exactly `Accepted`"
/// (F08 contract §4.4 step 1) before it does anything else.
pub fn load_state(root: &Path, id: &TaskId) -> Result<String, Refusal> {
    let path = task_path(root, id);
    match fs::read_to_string(path) {
        Ok(value) => Ok(value.trim().to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("Intake".to_owned()),
        Err(error) => Err(io_refusal(error)),
    }
}

/// Persist a task's runtime state atomically. Public so `main.rs`'s `fleet pr emit` can
/// record `"Proposed"` once `propose_change` succeeds (F08 contract §4.4 step 6); also used
/// internally by `propose_change` itself.
pub fn persist_state(root: &Path, id: &TaskId, state: &str) -> Result<(), Refusal> {
    let directory = root.join("lifecycle");
    fs::create_dir_all(&directory).map_err(io_refusal)?;
    let destination = task_path(root, id);
    let temporary = directory.join(format!(
        ".{}.{}.tmp",
        safe_task_name(id),
        std::process::id()
    ));
    fs::write(&temporary, format!("{state}\n")).map_err(io_refusal)?;
    fs::rename(temporary, destination).map_err(io_refusal)
}

/// Role-specific stopping points used by swarm dispatch. Each projection starts
/// with its own `Task<Intake>` and reaches the milestone only through typed edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentMilestone {
    Specified,
    Reviewed,
    Built,
    Verified,
    Observed,
}

pub fn project_agent_task(
    root: &Path,
    raw_id: impl Into<String>,
    milestone: AgentMilestone,
) -> Result<&'static str, Refusal> {
    let id = TaskId::new(raw_id)?;
    let ledger = FileLedger {
        root: root.to_owned(),
    };
    let specified = Task::new(id.clone()).specify("lead specification", &ledger)?;
    if milestone == AgentMilestone::Specified {
        persist_state(root, &id, "Specified")?;
        return Ok("Specified");
    }
    let reviewed = specified.review(HumanApproval::recorded("role review")?, &ledger)?;
    if milestone == AgentMilestone::Reviewed {
        persist_state(root, &id, "Reviewed")?;
        return Ok("Reviewed");
    }
    let decomposed = reviewed.decompose("role decomposition", &ledger)?;
    let contracted = decomposed.contract("role contract", &ledger)?;
    let briefed = contracted.brief("role brief", &ledger)?;
    let leased = briefed.lease("role lease", &ledger)?;
    let building = leased.build("role work started", &ledger)?;
    let built = building.finish_build("role work completed", &ledger)?;
    if milestone == AgentMilestone::Built {
        persist_state(root, &id, "Built")?;
        return Ok("Built");
    }
    let verifying = built.begin_verification("independent verification started", &ledger)?;
    let verified = verifying.verify("independent verification passed", &ledger)?;
    if milestone == AgentMilestone::Verified {
        persist_state(root, &id, "Verified")?;
        return Ok("Verified");
    }
    let attested = verified.attest("verified result attested", &ledger)?;
    let accepted = attested.accept(
        HumanApproval::recorded("verified result accepted")?,
        &ledger,
    )?;
    // F08 inserted `Proposed` into the canonical chain (`Accepted -> Proposed -> Observed`);
    // this role projection now passes through it like any other task. There is no real
    // diff/repo/PR here -- `synthetic_proposal` and `NoopEmitter` exist only so the SAME
    // role's own task can walk the SAME typed graph every other path now walks.
    let (bundle, request) = synthetic_proposal(id.as_str());
    let (proposed, _change) = accepted.propose(&bundle, &request, &NoopEmitter, &ledger)?;
    proposed.observe("outcome measured", &ledger)?;
    persist_state(root, &id, "Observed")?;
    Ok("Observed")
}

/// A synthetic, always-complete attestation used only by `project_agent_task`'s per-role
/// status projection (`swarm.rs`'s `AgentMilestone::Observed`, the "meter" role's own lane).
/// There is no real diff or repo behind it -- it exists solely so that projection can pass
/// through the `propose` edge like every real task now must.
fn synthetic_proposal(task: &str) -> (AttestationBundle, ProposalRequest) {
    let diff = format!("diff --git a/{task}.role b/{task}.role\nrole projection\n").into_bytes();
    let artifact_id = blake3::hash(&diff).to_hex().to_string();
    let elements = json!({
        "sow": {"task": task},
        "blind_suite": {
            "in_worktree_tree": true,
            "in_object_store": false,
            "in_env": false,
            "on_any_fd": false,
            "suite_hash": "role-projection"
        },
        "independent_verification": {
            "builder": "role",
            "verifier": "role-verifier",
            "distinct": true,
            "reproduced": true,
            "verdict": "ACCEPT"
        },
        "adequacy": {"status": "no-measurable-surface"},
        "blast_radius": {"files": [format!("{task}.role")], "count": 1, "source": "recorded-diff"},
        "rollback": {"executed": true, "suite_went_red": false, "reverted_files": 1},
        "cost": {
            "wall_ms": 0,
            "characters_in_diff": diff.len(),
            "tokens": null,
            "tokenizer_generation": null,
            "source": "observed"
        },
        "oracle_independence": {
            "o1_author": "lead",
            "o2_author": "verifier",
            "o1_hash": blake3::hash(b"role-projection-o1").to_hex().to_string(),
            "o2_hash": blake3::hash(b"role-projection-o2").to_hex().to_string(),
            "distinct": true,
            "quadrant": "ACCEPT"
        }
    });
    let request = ProposalRequest {
        repo: PathBuf::new(),
        base: "main".to_string(),
        head: format!("fleet/{task}"),
        artifact_id,
        diff,
        title: format!("fleet: {task}"),
        body: "role status projection -- not a real change".to_string(),
    };
    (AttestationBundle::new(elements), request)
}

struct NoopEmitter;

impl ChangeEmitter for NoopEmitter {
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, Refusal> {
        Ok(ProposedChange {
            url: "synthetic://role-projection".to_string(),
            head: request.head.clone(),
            commit: "0".repeat(40),
            changed_files: 1,
        })
    }
}

pub fn refuse_agent_task(
    root: &Path,
    raw_id: impl Into<String>,
    reason: impl Into<String>,
) -> Result<&'static str, Refusal> {
    let id = TaskId::new(raw_id)?;
    let ledger = FileLedger {
        root: root.to_owned(),
    };
    Task::new(id.clone()).refuse(reason, &ledger)?;
    persist_state(root, &id, "Refused")?;
    Ok("Refused")
}

fn canonical_next(state: &str) -> Option<&'static str> {
    match state {
        "Intake" => Some("Specified"),
        "Specified" => Some("Reviewed"),
        "Reviewed" => Some("Decomposed"),
        "Decomposed" => Some("Contracted"),
        "Contracted" => Some("Briefed"),
        "Briefed" => Some("Leased"),
        "Leased" => Some("Building"),
        "Building" => Some("Built"),
        "Built" => Some("Verifying"),
        "Verifying" => Some("Verified"),
        "Verified" => Some("Attested"),
        "Attested" => Some("Accepted"),
        "Accepted" => Some("Proposed"),
        "Proposed" => Some("Observed"),
        "Observed" => Some("Intake"),
        _ => None,
    }
}

fn typed_advance(
    state: &str,
    id: TaskId,
    ledger: &impl ReceiptLedger,
) -> Result<&'static str, Refusal> {
    macro_rules! task {
        ($state:ty) => {
            Task::<$state> {
                id,
                retry_depth: 0,
                _s: PhantomData,
            }
        };
    }
    match state {
        "Intake" => {
            task!(Intake).specify("fleet lifecycle advance", ledger)?;
            Ok("Specified")
        }
        "Specified" => {
            task!(Specified).review(HumanApproval::recorded("fleet lifecycle advance")?, ledger)?;
            Ok("Reviewed")
        }
        "Reviewed" => {
            task!(Reviewed).decompose("fleet lifecycle advance", ledger)?;
            Ok("Decomposed")
        }
        "Decomposed" => {
            task!(Decomposed).contract("fleet lifecycle advance", ledger)?;
            Ok("Contracted")
        }
        "Contracted" => {
            task!(Contracted).brief("fleet lifecycle advance", ledger)?;
            Ok("Briefed")
        }
        "Briefed" => {
            task!(Briefed).lease("fleet lifecycle advance", ledger)?;
            Ok("Leased")
        }
        "Leased" => {
            task!(Leased).build("fleet lifecycle advance", ledger)?;
            Ok("Building")
        }
        "Building" => {
            task!(Building).finish_build("fleet lifecycle advance", ledger)?;
            Ok("Built")
        }
        "Built" => {
            task!(Built).begin_verification("fleet lifecycle advance", ledger)?;
            Ok("Verifying")
        }
        "Verifying" => {
            task!(Verifying).verify("fleet lifecycle advance", ledger)?;
            Ok("Verified")
        }
        "Verified" => {
            task!(Verified).attest("fleet lifecycle advance", ledger)?;
            Ok("Attested")
        }
        "Attested" => {
            task!(Attested).accept(HumanApproval::recorded("fleet lifecycle advance")?, ledger)?;
            Ok("Accepted")
        }
        "Accepted" => {
            // `Accepted -> Proposed` emits a real pull request: it needs an artifact, a repo
            // and a base branch that `fleet lifecycle advance` does not carry (F08 contract
            // §4.4/§4.6). Widening this command to accept them would let it fabricate a PR
            // that never happened; instead it refuses and names the command that actually
            // can. `advance_to` must route this specific refusal to exit 7 (EXIT_REFUSAL),
            // not the generic exit 3 (EXIT_ENVIRONMENT) every other `Err` here gets.
            Err(Refusal::new(
                "PR_EMIT_REQUIRES_EVIDENCE",
                "Accepted -> Proposed emits a real pull request and needs an artifact and a repo; \
                 drive it with `fleet pr emit --task <ID> --artifact <ID> --repo <PATH> --base <BRANCH>`",
            ))
        }
        "Proposed" => {
            task!(Proposed).observe("fleet lifecycle advance", ledger)?;
            Ok("Observed")
        }
        "Observed" => {
            task!(Observed).reopen("fleet lifecycle advance", ledger)?;
            Ok("Intake")
        }
        _ => Err(Refusal::new(
            "ILLEGAL_LIFECYCLE_TRANSITION",
            format!("{state} has no canonical forward edge"),
        )),
    }
}

fn append_runtime_receipt(
    ledger: &impl ReceiptLedger,
    id: &TaskId,
    from: &str,
    to: &str,
    evidence: &str,
) -> Result<(), Refusal> {
    // Runtime dispatch has already validated this pair against the same graph as
    // the typed API. Leak the two tiny state names so the receipt retains its
    // deliberately static schema without weakening TransitionReceipt.
    let from = Box::leak(from.to_owned().into_boxed_str());
    let to = Box::leak(to.to_owned().into_boxed_str());
    ledger.append(TransitionReceipt {
        task_id: id.clone(),
        from,
        to,
        evidence: evidence.to_owned(),
    })
}

fn refuse_runtime(root: &Path, id: &TaskId, state: &str, message: &str) -> Result<(), i32> {
    let ledger = FileLedger {
        root: root.to_owned(),
    };
    let receipt = append_runtime_receipt(&ledger, id, state, "Refused", message);
    if let Err(error) = receipt {
        eprintln!("{error}");
        return Err(EXIT_ENVIRONMENT);
    }
    eprintln!("ILLEGAL_LIFECYCLE_TRANSITION: {message}");
    Err(EXIT_REFUSAL)
}

fn advance_to(root: &Path, id: &TaskId, state: &str, target: &str) -> Result<(), i32> {
    let legal = STATES
        .iter()
        .find(|(name, _)| *name == state)
        .is_some_and(|(_, edges)| edges.contains(&target));
    if !legal {
        return refuse_runtime(
            root,
            id,
            state,
            &format!("{state} -> {target} is not a legal lifecycle edge"),
        );
    }

    let ledger = FileLedger {
        root: root.to_owned(),
    };
    let next = match typed_advance(state, id.clone(), &ledger) {
        Ok(next) => next,
        // F08: `Accepted -> Proposed` needs an artifact/repo/base this command does not have
        // (§4.6). That is a real refusal of the edge, not an environment fault -- it must
        // exit 7 and leave a "Refused" receipt like every other runtime refusal here, not
        // exit 3 in silence.
        Err(refusal) if refusal.code() == "PR_EMIT_REQUIRES_EVIDENCE" => {
            return refuse_runtime(root, id, state, refusal.message());
        }
        Err(refusal) => return Err(report_environment(refusal)),
    };
    if next != target {
        return refuse_runtime(
            root,
            id,
            state,
            "typed and runtime lifecycle graphs disagree",
        );
    }
    persist_state(root, id, next).map_err(report_environment)
}

fn parse_task(args: &[String]) -> Result<TaskId, i32> {
    if args.len() != 2 || args[0] != "--task" {
        eprintln!("fleet lifecycle <advance|show> --task ID");
        return Err(EXIT_REFUSAL);
    }
    TaskId::new(args[1].clone()).map_err(|error| {
        eprintln!("{error}");
        EXIT_REFUSAL
    })
}

/// CLI entry point for the persisted runtime projection of the typestate graph.
pub fn command(args: &[String]) -> Result<(), i32> {
    match args.first().map(String::as_str) {
        Some("states") if args.len() == 1 => {
            for (state, edges) in STATES {
                println!(
                    "{state}: {}",
                    if edges.is_empty() {
                        "(terminal)".to_owned()
                    } else {
                        edges.join(", ")
                    }
                );
            }
            Ok(())
        }
        Some("show") => {
            let id = parse_task(&args[1..])?;
            let root = state_dir().map_err(report_environment)?;
            let state = load_state(&root, &id).map_err(report_environment)?;
            let edges = STATES.iter().find(|(name, _)| *name == state);
            match edges {
                Some((_, legal)) => {
                    println!("task: {id}");
                    println!("state: {state}");
                    println!(
                        "legal: {}",
                        if legal.is_empty() {
                            "(none)".to_owned()
                        } else {
                            legal.join(", ")
                        }
                    );
                    Ok(())
                }
                None => refuse_runtime(
                    &root,
                    &id,
                    &state,
                    "persisted state is not a lifecycle state",
                ),
            }
        }
        Some("advance") => {
            let id = parse_task(&args[1..])?;
            let root = state_dir().map_err(report_environment)?;
            let state = load_state(&root, &id).map_err(report_environment)?;
            let Some(expected) = canonical_next(&state) else {
                return refuse_runtime(
                    &root,
                    &id,
                    &state,
                    "the current state has no canonical forward edge",
                );
            };
            advance_to(&root, &id, &state, expected)?;
            println!("{id}: {state} -> {expected}");
            Ok(())
        }
        _ => {
            eprintln!("fleet lifecycle <states|advance --task ID|show --task ID>");
            Err(EXIT_REFUSAL)
        }
    }
}

fn report_environment(error: Refusal) -> i32 {
    eprintln!("{error}");
    EXIT_ENVIRONMENT
}

/// Drive the real run operation through the typed states corresponding to its
/// preparation, build, verification, attestation, and acceptance phases.
pub fn drive_run<F>(args: &[String], operation: F) -> Result<(), i32>
where
    F: FnOnce() -> Result<(), i32>,
{
    let root = state_dir().map_err(report_environment)?;
    let ledger = FileLedger { root };
    let raw_id = args
        .windows(2)
        .find(|pair| pair[0] == "--task")
        .map(|pair| pair[1].clone())
        .unwrap_or_else(|| format!("run-{}", std::process::id()));
    // D29: do NOT pre-empt run_command's own refusals. An empty --task used to be refused HERE,
    // before `run_command` reached the guard that writes a refusal receipt -- so `fleet run
    // --task ""` exited 7 and left an empty state dir. p0's F2 ("the refusal WROTE A RECEIPT")
    // caught it. When the id is unusable, hand the case to the inner operation, which refuses
    // with a receipt as it always did.
    let id = match TaskId::new(raw_id) {
        Ok(id) => id,
        Err(_) => return operation(),
    };
    let task = Task::new(id);
    let task = task
        .specify("run arguments parsed", &ledger)
        .map_err(report_environment)?;
    let task = task
        .review(
            HumanApproval::recorded("run authorized").map_err(report_environment)?,
            &ledger,
        )
        .map_err(report_environment)?;
    let task = task
        .decompose("run work selected", &ledger)
        .map_err(report_environment)?;
    let task = task
        .contract("run contract loaded", &ledger)
        .map_err(report_environment)?;
    let task = task
        .brief("worker brief assembled", &ledger)
        .map_err(report_environment)?;
    let task = task
        .lease("run lease acquired", &ledger)
        .map_err(report_environment)?;
    let task = task
        .build("run started", &ledger)
        .map_err(report_environment)?;
    if let Err(code) = operation() {
        task.refuse("run failed", &ledger)
            .map_err(report_environment)?;
        return Err(code);
    }
    let task = task
        .finish_build("run completed", &ledger)
        .map_err(report_environment)?;
    let task = task
        .begin_verification("run verification started", &ledger)
        .map_err(report_environment)?;
    let task = task
        .verify("run verification passed", &ledger)
        .map_err(report_environment)?;
    let task = task
        .attest("run evidence attested", &ledger)
        .map_err(report_environment)?;
    task.accept(
        HumanApproval::recorded("run accepted").map_err(report_environment)?,
        &ledger,
    )
    .map_err(report_environment)?;
    Ok(())
}

/// Lib-side driver for `Accepted -> Proposed`, called from `main.rs`'s `fleet pr emit` (and
/// the test-only `__pr_emit_probe` that drives the identical path -- F08 contract §4.4 step
/// 5). Loads the persisted state and refuses unless it is exactly `Accepted`, mints the
/// internal token (only code in this module can construct a `Task<S>` directly -- the same
/// trick `typed_advance` uses above), runs the typed `propose` edge, and persists
/// `"Proposed"` on success. Mirrors `drive_run`'s shape: `main.rs` never touches `Task<S>`'s
/// private fields.
pub fn propose_change(
    root: &Path,
    id: TaskId,
    bundle: &AttestationBundle,
    request: &ProposalRequest,
    emitter: &impl ChangeEmitter,
) -> Result<ProposedChange, Refusal> {
    let state = load_state(root, &id)?;
    if state != "Accepted" {
        return Err(Refusal::new(
            "NOT_ACCEPTED",
            format!("persisted state is {state}, not Accepted"),
        ));
    }
    let task = Task::<Accepted> {
        id,
        retry_depth: 0,
        _s: PhantomData,
    };
    let ledger = FileLedger {
        root: root.to_owned(),
    };
    let (proposed, change) = task.propose(bundle, request, emitter, &ledger)?;
    persist_state(root, proposed.id(), "Proposed")?;
    Ok(change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MemoryLedger(RefCell<Vec<TransitionReceipt>>);

    impl ReceiptLedger for MemoryLedger {
        fn append(&self, receipt: TransitionReceipt) -> Result<(), Refusal> {
            self.0.borrow_mut().push(receipt);
            Ok(())
        }
    }

    /// A structurally complete `predicate.elements` object -- every key
    /// `AttestationBundle::missing_element` requires, each shaped exactly as
    /// `attest_verify_inner` (`main.rs`) requires it.
    fn complete_elements() -> Value {
        let o1_hash = "a".repeat(64);
        let o2_hash = "b".repeat(64);
        json!({
            "sow": {"task": "unit-test-task"},
            "blind_suite": {
                "in_worktree_tree": true,
                "in_object_store": false,
                "in_env": false,
                "on_any_fd": false,
                "suite_hash": "deadbeef"
            },
            "independent_verification": {
                "builder": "builder-agent",
                "verifier": "verifier-agent",
                "distinct": true,
                "reproduced": true,
                "verdict": "ACCEPT"
            },
            "adequacy": {"status": "no-measurable-surface"},
            "blast_radius": {"files": ["main.rs"], "count": 1, "source": "recorded-diff"},
            "rollback": {"executed": true, "suite_went_red": false, "reverted_files": 1},
            "cost": {
                "wall_ms": 1,
                "characters_in_diff": 32,
                "tokens": null,
                "tokenizer_generation": null,
                "source": "observed"
            },
            "oracle_independence": {
                "o1_author": "lead",
                "o2_author": "verifier",
                "o1_hash": o1_hash,
                "o2_hash": o2_hash,
                "distinct": true,
                "quadrant": "ACCEPT"
            }
        })
    }

    /// A recording double for [`ChangeEmitter`]: proves the side effect either ran exactly
    /// once (the pass case) or never ran at all (every refusal case) -- unit tests 2 and 3
    /// of the F08 contract's §6.4.
    #[derive(Default)]
    struct RecordingEmitter(RefCell<Vec<ProposalRequest>>);

    impl ChangeEmitter for RecordingEmitter {
        fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, Refusal> {
            self.0.borrow_mut().push(request.clone());
            Ok(ProposedChange {
                url: "https://example.test/acme/repo/pull/1".to_string(),
                head: request.head.clone(),
                commit: "0".repeat(40),
                changed_files: 1,
            })
        }
    }

    fn sample_request(head: &str) -> ProposalRequest {
        let diff = b"diff --git a/main.rs b/main.rs\nunit test fixture\n".to_vec();
        let artifact_id = blake3::hash(&diff).to_hex().to_string();
        ProposalRequest {
            repo: PathBuf::from("/nonexistent/lifecycle-unit-test"),
            base: "main".to_string(),
            head: head.to_string(),
            artifact_id,
            diff,
            title: format!("fleet: {head}"),
            body: "evidence bundle -- not to be self-merged".to_string(),
        }
    }

    #[test]
    fn missing_element_names_the_first_incomplete_or_absent_element() {
        let complete = complete_elements();
        assert_eq!(AttestationBundle::new(complete.clone()).missing_element(), None);

        // The real, routinely-produced incomplete attestation (F08 contract §4.5 note):
        // `run_with_evidence` writes this exact shape before adjudication fills it in.
        let mut pending = complete.clone();
        pending["oracle_independence"] = json!({"status": "pending-adjudication"});
        assert_eq!(
            AttestationBundle::new(pending).missing_element(),
            Some("oracle_independence")
        );

        let mut no_adequacy = complete;
        no_adequacy
            .as_object_mut()
            .unwrap()
            .remove("adequacy");
        assert_eq!(
            AttestationBundle::new(no_adequacy).missing_element(),
            Some("adequacy")
        );
    }

    #[test]
    fn pinned_element_names_match_the_enforced_set() {
        // Exactly the 8 elements `attest_verify_inner` enforces today (main.rs:4062-4074),
        // in the order the F08 contract's §4.5 states them -- not the aspirational 9th
        // (§8.2). When a 9th element lands, this must change consciously, not silently.
        assert_eq!(
            AttestationBundle::REQUIRED,
            [
                "sow",
                "blind_suite",
                "independent_verification",
                "adequacy",
                "blast_radius",
                "rollback",
                "cost",
                "oracle_independence",
            ]
        );
    }

    #[test]
    fn complete_bundle_advances_to_proposed_and_records_one_receipt() {
        let ledger = MemoryLedger::default();
        let task = Task::new(TaskId::new("propose-pass").unwrap());
        let task = task.specify("SOW", &ledger).unwrap();
        let task = task
            .review(HumanApproval::recorded("operator approval").unwrap(), &ledger)
            .unwrap();
        let task = task.decompose("atomic leaves", &ledger).unwrap();
        let task = task.contract("blind suite", &ledger).unwrap();
        let task = task.brief("assembled brief", &ledger).unwrap();
        let task = task.lease("worktree lease", &ledger).unwrap();
        let task = task.build("supervised spawn", &ledger).unwrap();
        let task = task.finish_build("non-empty diff", &ledger).unwrap();
        let task = task.begin_verification("different model", &ledger).unwrap();
        let task = task.verify("reproduced", &ledger).unwrap();
        let task = task.attest("eight elements", &ledger).unwrap();
        let task = task
            .accept(HumanApproval::recorded("operator acceptance").unwrap(), &ledger)
            .unwrap();

        let bundle = AttestationBundle::new(complete_elements());
        let request = sample_request("fleet/propose-pass");
        let emitter = RecordingEmitter::default();
        let before = ledger.0.borrow().len();
        let (proposed, change) = task.propose(&bundle, &request, &emitter, &ledger).unwrap();

        assert_eq!(proposed.id().as_str(), "propose-pass");
        assert_eq!(change.url, "https://example.test/acme/repo/pull/1");
        assert_eq!(emitter.0.borrow().len(), 1, "the emitter must run exactly once");
        assert_eq!(ledger.0.borrow().len(), before + 1, "propose must append exactly one receipt");
    }

    #[test]
    fn incomplete_bundle_refuses_before_the_emitter_runs() {
        let ledger = MemoryLedger::default();
        let task = Task::new(TaskId::new("propose-refuse").unwrap());
        let task = task.specify("SOW", &ledger).unwrap();
        let task = task
            .review(HumanApproval::recorded("operator approval").unwrap(), &ledger)
            .unwrap();
        let task = task.decompose("atomic leaves", &ledger).unwrap();
        let task = task.contract("blind suite", &ledger).unwrap();
        let task = task.brief("assembled brief", &ledger).unwrap();
        let task = task.lease("worktree lease", &ledger).unwrap();
        let task = task.build("supervised spawn", &ledger).unwrap();
        let task = task.finish_build("non-empty diff", &ledger).unwrap();
        let task = task.begin_verification("different model", &ledger).unwrap();
        let task = task.verify("reproduced", &ledger).unwrap();
        let task = task.attest("eight elements", &ledger).unwrap();
        let task = task
            .accept(HumanApproval::recorded("operator acceptance").unwrap(), &ledger)
            .unwrap();

        let mut elements = complete_elements();
        elements.as_object_mut().unwrap().remove("adequacy");
        let bundle = AttestationBundle::new(elements);
        let request = sample_request("fleet/propose-refuse");
        let emitter = RecordingEmitter::default();
        let before = ledger.0.borrow().len();

        let refusal = task.propose(&bundle, &request, &emitter, &ledger).unwrap_err();

        assert_eq!(refusal.code(), "INCOMPLETE_ATTESTATION");
        assert!(emitter.0.borrow().is_empty(), "the gate must run before the side effect");
        assert_eq!(ledger.0.borrow().len(), before, "a refusal must not append a receipt");
    }

    #[test]
    fn legal_path_consumes_each_state_and_records_every_edge() {
        let ledger = MemoryLedger::default();
        let task = Task::new(TaskId::new("task-1").unwrap());
        let task = task.specify("SOW", &ledger).unwrap();
        let task = task
            .review(
                HumanApproval::recorded("operator approval").unwrap(),
                &ledger,
            )
            .unwrap();
        let task = task.decompose("atomic leaves", &ledger).unwrap();
        let task = task.contract("blind suite", &ledger).unwrap();
        let task = task.brief("assembled brief", &ledger).unwrap();
        let task = task.lease("worktree lease", &ledger).unwrap();
        let task = task.build("supervised spawn", &ledger).unwrap();
        let task = task.finish_build("non-empty diff", &ledger).unwrap();
        let task = task.begin_verification("different model", &ledger).unwrap();
        let task = task.verify("reproduced", &ledger).unwrap();
        let task = task.attest("seven elements", &ledger).unwrap();
        let task = task
            .accept(
                HumanApproval::recorded("operator acceptance").unwrap(),
                &ledger,
            )
            .unwrap();
        let bundle = AttestationBundle::new(complete_elements());
        let request = sample_request("fleet/task-1");
        let emitter = RecordingEmitter::default();
        let (task, _change) = task.propose(&bundle, &request, &emitter, &ledger).unwrap();
        let task = task.observe("measured baseline", &ledger).unwrap();
        let task = task.reopen("control band breach", &ledger).unwrap();

        assert_eq!(task.id().as_str(), "task-1");
        assert_eq!(ledger.0.borrow().len(), 15);
    }

    #[test]
    fn empty_evidence_refuses_without_advancing_or_writing() {
        let ledger = MemoryLedger::default();
        let task = Task::new(TaskId::new("task-2").unwrap());
        let refusal = task.specify("  ", &ledger).unwrap_err();
        assert_eq!(refusal.code(), "EMPTY_TRANSITION_EVIDENCE");
        assert!(ledger.0.borrow().is_empty());
    }

    #[test]
    fn persisted_task_advances_across_invocations() {
        let output = std::process::Command::new("mktemp")
            .arg("-d")
            .output()
            .expect("mktemp must be available");
        assert!(output.status.success());
        let root = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        let id = TaskId::new("persisted-task").unwrap();
        let ledger = FileLedger { root: root.clone() };
        let first = load_state(&root, &id).unwrap();
        let next = typed_advance(&first, id.clone(), &ledger).unwrap();
        persist_state(&root, &id, next).unwrap();
        let second = load_state(&root, &id).unwrap();
        let next = typed_advance(&second, id.clone(), &ledger).unwrap();
        persist_state(&root, &id, next).unwrap();
        assert_eq!(load_state(&root, &id).unwrap(), "Reviewed");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn illegal_runtime_steps_refuse_in_both_directions() {
        let output = std::process::Command::new("mktemp")
            .arg("-d")
            .output()
            .expect("mktemp must be available");
        assert!(output.status.success());
        let root = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        let id = TaskId::new("illegal-task").unwrap();
        assert_eq!(
            advance_to(&root, &id, "Intake", "Observed"),
            Err(EXIT_REFUSAL)
        );
        assert_eq!(
            advance_to(&root, &id, "Observed", "Accepted"),
            Err(EXIT_REFUSAL)
        );

        let receipts = fs::read_to_string(root.join("lifecycle-receipts.jsonl")).unwrap();
        assert_eq!(receipts.lines().count(), 2);
        assert!(receipts.contains("Intake -> Observed"));
        assert!(receipts.contains("Observed -> Accepted"));
        fs::remove_dir_all(root).unwrap();
    }
}
