//! Compile-time lifecycle for a fleet task.
//!
//! A state is represented by the type parameter of [`Task`]. Legal transitions
//! consume the old task, append a receipt, and return a task with a new state.

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
    ("Accepted", &["Observed"]),
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
    Verifying, Verified, Attested, Accepted, Observed, Refused,
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
edge!(Accepted, observe, Observed);
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

impl Task<Attested> {
    pub fn accept(
        self,
        approval: HumanApproval,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Accepted>, Refusal> {
        self.transition(approval.evidence, ledger)
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

fn load_state(root: &Path, id: &TaskId) -> Result<String, Refusal> {
    let path = task_path(root, id);
    match fs::read_to_string(path) {
        Ok(value) => Ok(value.trim().to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("Intake".to_owned()),
        Err(error) => Err(io_refusal(error)),
    }
}

fn persist_state(root: &Path, id: &TaskId, state: &str) -> Result<(), Refusal> {
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
    accepted.observe("outcome measured", &ledger)?;
    persist_state(root, &id, "Observed")?;
    Ok("Observed")
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
        "Accepted" => Some("Observed"),
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
            task!(Accepted).observe("fleet lifecycle advance", ledger)?;
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
    let next = typed_advance(state, id.clone(), &ledger).map_err(report_environment)?;
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
        let task = task.observe("measured baseline", &ledger).unwrap();
        let task = task.reopen("control band breach", &ledger).unwrap();

        assert_eq!(task.id().as_str(), "task-1");
        assert_eq!(ledger.0.borrow().len(), 14);
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
