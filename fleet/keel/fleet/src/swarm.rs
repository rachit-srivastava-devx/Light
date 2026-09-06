use fleet::agent::{self, Registry, ScorecardOutcome};
use fleet::lifecycle::{self, AgentMilestone};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const STATES_POSSIBLE: usize = 15;

const STATES: &[&str] = &[
    "Intake",
    "Specified",
    "Reviewed",
    "Decomposed",
    "Contracted",
    "Briefed",
    "Leased",
    "Building",
    "Built",
    "Verifying",
    "Verified",
    "Attested",
    "Accepted",
    "Observed",
    "Refused",
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentStatus {
    pub id: String,
    pub role: String,
    pub state: String,
    pub task: String,
    #[serde(default = "default_capacity")]
    pub capacity: u64,
    #[serde(default)]
    pub load: u64,
    #[serde(default)]
    pub acceptance_score: Option<u64>,
}

const ACCEPTANCE_SCALE: u64 = 10_000;

fn default_capacity() -> u64 {
    1
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Allocation {
    pub requested: u64,
    pub checked: usize,
    pub total: usize,
    pub assignments: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Bandwidth {
    pub id: String,
    pub role: String,
    pub capacity: u64,
    pub load: u64,
    pub free: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BandwidthReport {
    pub agents: Vec<Bandwidth>,
    pub checked: usize,
    pub total: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StatusError {
    Environment(String),
    Invariant(String),
    Refusal { agent: String, reason: String },
    ZeroAgents,
    TooFewAgents { found: usize },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DispatchReport {
    pub task: String,
    pub agent: String,
    pub checked: usize,
    pub total: usize,
    pub states: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceOutcome {
    Verified,
    Faulted,
    Unknown,
}

fn agent_error(code: i32, action: &str) -> StatusError {
    if code == 3 {
        StatusError::Environment(format!("cannot {action}"))
    } else {
        StatusError::Invariant(format!("cannot {action} (exit {code})"))
    }
}

fn lifecycle_error(error: lifecycle::Refusal) -> StatusError {
    if error.code() == "LIFECYCLE_IO" {
        StatusError::Environment(error.to_string())
    } else {
        StatusError::Invariant(error.to_string())
    }
}

fn refuse_role(
    state_dir: &Path,
    agents: &mut [AgentStatus],
    task: &str,
    role: crate::roles::Role,
    reason: &str,
) -> Result<StatusError, StatusError> {
    let agent = agents
        .iter_mut()
        .find(|agent| agent.role == role.name())
        .ok_or_else(|| StatusError::Invariant(format!("swarm has no {} agent", role.name())))?;
    let task_id = format!("{}::{}", task, agent.id);
    lifecycle::refuse_agent_task(state_dir, task_id, reason).map_err(lifecycle_error)?;
    agent.state = "Refused".to_string();
    agent.task = task.to_string();
    agent::record_scorecard_outcome(state_dir, &agent.id, ScorecardOutcome::Faulted)
        .map_err(|code| agent_error(code, "record refused agent scorecard"))?;
    let refused = StatusError::Refusal {
        agent: agent.id.clone(),
        reason: reason.to_string(),
    };
    persist(state_dir, agents)?;
    Ok(refused)
}

/// Commit a verified dispatch to each agent's independent typed lifecycle.
/// The caller owns process execution; this function owns role policy, state,
/// and scorecard attribution.
pub fn dispatch_verified(
    state_dir: &Path,
    task: &str,
    worker: &str,
    builder_model: &str,
    verifier_model: &str,
    lead_diff_adds_code: bool,
    evidence: EvidenceOutcome,
) -> Result<DispatchReport, StatusError> {
    if task.trim().is_empty() || worker.trim().is_empty() {
        return Err(StatusError::Invariant(
            "dispatch task and worker must be non-empty".to_string(),
        ));
    }
    let mut agents = load_or_seed(state_dir)?;
    let total = agents.len();

    if evidence != EvidenceOutcome::Verified {
        let outcome = match evidence {
            EvidenceOutcome::Faulted => ScorecardOutcome::Faulted,
            EvidenceOutcome::Unknown => ScorecardOutcome::Unknown,
            EvidenceOutcome::Verified => unreachable!(),
        };
        for status in &agents {
            agent::record_scorecard_outcome(state_dir, &status.id, outcome)
                .map_err(|code| agent_error(code, "record unverified agent scorecard"))?;
        }
        return Err(StatusError::Invariant(
            "dispatch has no verifying attestation; credit refused".to_string(),
        ));
    }

    let lead_check = crate::roles::Check {
        role: crate::roles::Role::Lead,
        diff_adds_code: lead_diff_adds_code,
        builder_model: None,
        verifier_model: None,
    };
    if let Err(refusal) = crate::roles::evaluate(&lead_check) {
        let error = refuse_role(
            state_dir,
            &mut agents,
            task,
            crate::roles::Role::Lead,
            refusal.reason(),
        )?;
        return Err(error);
    }

    let verifier_check = crate::roles::Check {
        role: crate::roles::Role::Verifier,
        diff_adds_code: false,
        builder_model: Some(builder_model),
        verifier_model: Some(verifier_model),
    };
    if let Err(refusal) = crate::roles::evaluate(&verifier_check) {
        let error = refuse_role(
            state_dir,
            &mut agents,
            task,
            crate::roles::Role::Verifier,
            refusal.reason(),
        )?;
        return Err(error);
    }

    let milestones = [
        (crate::roles::Role::Lead, AgentMilestone::Specified),
        (crate::roles::Role::Designer, AgentMilestone::Reviewed),
        (crate::roles::Role::Builder, AgentMilestone::Built),
        (crate::roles::Role::Verifier, AgentMilestone::Verified),
        (crate::roles::Role::Meter, AgentMilestone::Observed),
    ];
    let mut states = BTreeMap::new();
    let mut advanced = 0usize;
    for (role, milestone) in milestones {
        let status = agents
            .iter_mut()
            .find(|agent| agent.role == role.name())
            .ok_or_else(|| StatusError::Invariant(format!("swarm has no {} agent", role.name())))?;
        let task_id = format!("{}::{}", task, status.id);
        let state = lifecycle::project_agent_task(state_dir, task_id, milestone)
            .map_err(lifecycle_error)?;
        status.state = state.to_string();
        status.task = task.to_string();
        states.insert(status.id.clone(), state.to_string());
        agent::record_scorecard_outcome(state_dir, &status.id, ScorecardOutcome::Credited)
            .map_err(|code| agent_error(code, "record verified agent scorecard"))?;
        advanced = advanced.checked_add(1).ok_or_else(|| {
            StatusError::Invariant("dispatch advance count overflowed".to_string())
        })?;
    }
    if advanced == 0 {
        return Err(StatusError::Invariant(
            "dispatch advanced zero agents".to_string(),
        ));
    }
    persist(state_dir, &agents)?;
    Ok(DispatchReport {
        task: task.to_string(),
        agent: worker.to_string(),
        checked: advanced,
        total,
        states,
    })
}

pub fn load_or_seed(state_dir: &Path) -> Result<Vec<AgentStatus>, StatusError> {
    let agents_dir = state_dir.join("agents");
    // D54: `if !exists { seed }` is a TOCTOU race. Four concurrent dispatches: one created the
    // directory, another saw it EXIST, skipped seeding, and loaded zero agents -- "agent store
    // contains zero agents", one failure in four. Existence is not the invariant; being POPULATED
    // is. Re-check after loading and seed under a lock, then load again.
    // This eager branch used plain `seed()`, which writes five files one at a time INTO THE LIVE
    // DIRECTORY -- the exact partial-read window the atomic path below was built to remove. It is
    // why the residual rate would not fall: I made the locked path atomic and left the unlocked
    // one writing in place. Every seeding route must be the atomic one.
    if !agents_dir.exists() {
        seed_atomically(&agents_dir)?;
    }
    let agents = load(&agents_dir)?;
    // Emptiness is the right trigger once seeding is ATOMIC: a staging dir renamed into place
    // means a partial store cannot be observed, so "fewer agents than the registry" is a
    // deliberately small swarm (tests construct them) and not a torn read. An earlier version
    // tested completeness here and silently re-seeded a hand-written 2-agent store up to 5,
    // which broke over_capacity_is_refused -- the assertion was right and the fix was wrong.
    if !agents.is_empty() {
        return Ok(agents);
    }
    // Empty: either we lost the race or the store was truncated. Serialise on a lock file so
    // exactly one process seeds, then re-read whatever the winner wrote.
    fs::create_dir_all(&agents_dir)
        .map_err(|error| StatusError::Environment(format!("cannot create agent store: {error}")))?;
    let lock_path = agents_dir.join(".seed.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| StatusError::Environment(format!("cannot open seed lock: {error}")))?;
    lock.lock_exclusive()
        .map_err(|error| StatusError::Environment(format!("cannot take seed lock: {error}")))?;
    let agents = load(&agents_dir)?;
    // Not `is_empty()`: a reader can catch the store mid-seed with SOME files written. Seeding
    // writes five agents one file at a time, and a reader that saw one got "a swarm requires at
    // least two agents; found 1". The invariant is COMPLETE, not non-empty.
    if agents.is_empty() {
        seed_atomically(&agents_dir)?;
    }
    let seeded = load(&agents_dir);
    let _ = FileExt::unlock(&lock);
    seeded
}

/// Seed into a sibling temp directory and rename it into place, so a concurrent reader sees
/// either no store at all or the complete set -- never a partial one.
fn seed_atomically(agents_dir: &Path) -> Result<(), StatusError> {
    let staging = agents_dir.with_extension(format!("seeding.{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    seed(&staging)?;
    // NEVER remove the live directory first. The earlier version did `remove_dir_all` then
    // `rename`, which opened a window where a reader on the lock-free fast path saw no store or a
    // half-removed one -- the entire residual 6% was this, reported as "found 1", "no meter
    // agent", "zero agents". Rename ONLY into a free slot; if another racer already installed a
    // complete store, keep theirs and discard ours. The live directory is never destroyed, so
    // there is no window to observe.
    if agents_dir.exists() {
        let _ = fs::remove_dir_all(&staging);
        return Ok(());
    }
    match fs::rename(&staging, agents_dir) {
        Ok(()) => Ok(()),
        // Lost the race between the check and the rename: the winner's store is already there.
        Err(_) if agents_dir.exists() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(StatusError::Environment(format!(
                "cannot install seeded agent store: {error}"
            )))
        }
    }
}

fn seed(agents_dir: &Path) -> Result<(), StatusError> {
    fs::create_dir_all(agents_dir).map_err(|error| {
        StatusError::Environment(format!(
            "cannot create agent store {}: {error}",
            agents_dir.display()
        ))
    })?;
    let registry = Registry::load_default().map_err(|code| {
        StatusError::Invariant(format!("cannot load agent registry (exit {code})"))
    })?;
    if registry.agents().len() < 2 {
        return Err(StatusError::Invariant(
            "agent registry cannot seed a swarm with fewer than two agents".to_string(),
        ));
    }
    for agent in registry.agents() {
        let status = AgentStatus {
            id: agent.agent_id().to_string(),
            role: agent.role().to_string(),
            state: "Intake".to_string(),
            task: "swarm-bootstrap".to_string(),
            capacity: crate::roles::Role::parse(agent.role())
                .map(crate::roles::Role::bandwidth)
                .ok_or_else(|| {
                    StatusError::Invariant(format!("unknown agent role: {}", agent.role()))
                })?,
            load: 0,
            acceptance_score: None,
        };
        let bytes = serde_json::to_vec_pretty(&status).map_err(|error| {
            StatusError::Invariant(format!("cannot encode seeded agent {}: {error}", status.id))
        })?;
        write_atomic(&agents_dir.join(format!("{}.json", status.id)), &bytes).map_err(|error| {
            StatusError::Environment(format!(
                "cannot persist seeded agent {}: {error}",
                status.id
            ))
        })?;
    }
    Ok(())
}

fn load(agents_dir: &Path) -> Result<Vec<AgentStatus>, StatusError> {
    let entries = fs::read_dir(agents_dir).map_err(|error| {
        StatusError::Environment(format!(
            "cannot read agent store {}: {error}",
            agents_dir.display()
        ))
    })?;
    let mut paths = entries
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                StatusError::Environment(format!("cannot read agent entry: {error}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("json"));
    paths.sort();
    if paths.is_empty() {
        return Err(StatusError::ZeroAgents);
    }

    let mut ids = BTreeSet::new();
    let mut agents = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(&path).map_err(|error| {
            StatusError::Environment(format!(
                "cannot read agent record {}: {error}",
                path.display()
            ))
        })?;
        let agent: AgentStatus = serde_json::from_slice(&bytes).map_err(|error| {
            StatusError::Invariant(format!("invalid agent record {}: {error}", path.display()))
        })?;
        validate(&agent)?;
        if !ids.insert(agent.id.clone()) {
            return Err(StatusError::Invariant(format!(
                "duplicate agent id in store: {}",
                agent.id
            )));
        }
        agents.push(agent);
    }
    if agents.len() < 2 {
        return Err(StatusError::TooFewAgents {
            found: agents.len(),
        });
    }
    Ok(agents)
}

fn validate(agent: &AgentStatus) -> Result<(), StatusError> {
    if agent.id.trim().is_empty() || agent.role.trim().is_empty() || agent.task.trim().is_empty() {
        return Err(StatusError::Invariant(
            "agent id, role, and task must be non-empty".to_string(),
        ));
    }
    if !STATES.contains(&agent.state.as_str()) {
        return Err(StatusError::Invariant(format!(
            "agent {} has invalid lifecycle state {}",
            agent.id, agent.state
        )));
    }
    if crate::roles::Role::parse(&agent.role).is_none() {
        return Err(StatusError::Invariant(format!(
            "agent {} has unknown role {}",
            agent.id, agent.role
        )));
    }
    if agent.capacity == 0 {
        return Err(StatusError::Invariant(format!(
            "agent {} has zero bandwidth capacity",
            agent.id
        )));
    }
    if agent.load > agent.capacity {
        return Err(StatusError::Invariant(format!(
            "agent {} load {} exceeds capacity {}",
            agent.id, agent.load, agent.capacity
        )));
    }
    if let Some(score) = agent.acceptance_score {
        if score > ACCEPTANCE_SCALE {
            return Err(StatusError::Invariant(format!(
                "agent {} acceptance score {} exceeds {}",
                agent.id, score, ACCEPTANCE_SCALE
            )));
        }
    }
    Ok(())
}

pub fn bandwidth(state_dir: &Path) -> Result<BandwidthReport, StatusError> {
    let agents = load_or_seed(state_dir)?;
    let total = agents.len();
    if total == 0 {
        return Err(StatusError::ZeroAgents);
    }
    let rows = agents
        .into_iter()
        .map(|agent| Bandwidth {
            id: agent.id,
            role: agent.role,
            capacity: agent.capacity,
            load: agent.load,
            free: agent.capacity - agent.load,
        })
        .collect::<Vec<_>>();
    Ok(BandwidthReport {
        checked: rows.len(),
        total,
        agents: rows,
    })
}

pub fn allocate(state_dir: &Path, tasks: u64) -> Result<Allocation, StatusError> {
    if tasks == 0 {
        return Err(StatusError::Invariant(
            "allocation refuses empty demand: tasks=0".to_string(),
        ));
    }
    let mut agents = load_or_seed(state_dir)?;
    let total = agents.len();
    if total == 0 {
        return Err(StatusError::ZeroAgents);
    }
    let free = agents.iter().try_fold(0_u64, |sum, agent| {
        sum.checked_add(agent.capacity - agent.load).ok_or_else(|| {
            StatusError::Invariant("total free bandwidth overflowed u64".to_string())
        })
    })?;
    if tasks > free {
        return Err(StatusError::Invariant(format!(
            "demand exceeds total capacity: requested={tasks} free={free}"
        )));
    }

    let mut assignments = BTreeMap::new();
    for _ in 0..tasks {
        let selected = agents
            .iter()
            .enumerate()
            .filter(|(_, agent)| agent.load < agent.capacity)
            .filter_map(|(index, agent)| {
                crate::roles::Role::parse(&agent.role).and_then(|role| {
                    let free = agent.capacity - agent.load;
                    role.allocation_fitness()
                        .checked_mul(free)
                        .map(|fitness| (index, fitness, agent.id.as_str()))
                })
            })
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.2.cmp(left.2)))
            .map(|candidate| candidate.0)
            .ok_or_else(|| {
                StatusError::Invariant(
                    "allocation found no fit agent despite available capacity".to_string(),
                )
            })?;
        agents[selected].load += 1;
        *assignments.entry(agents[selected].id.clone()).or_insert(0) += 1;
    }
    persist(state_dir, &agents)?;
    Ok(Allocation {
        requested: tasks,
        checked: total,
        total,
        assignments,
    })
}

pub fn complete_task(
    state_dir: &Path,
    agent_id: &str,
    acceptance_score: u64,
) -> Result<(), StatusError> {
    if acceptance_score > ACCEPTANCE_SCALE {
        return Err(StatusError::Invariant(format!(
            "acceptance score {acceptance_score} exceeds {ACCEPTANCE_SCALE}"
        )));
    }
    let mut agents = load_or_seed(state_dir)?;
    let agent = agents
        .iter_mut()
        .find(|agent| agent.id == agent_id)
        .ok_or_else(|| StatusError::Invariant(format!("unknown agent: {agent_id}")))?;
    if agent.load == 0 {
        return Err(StatusError::Invariant(format!(
            "agent {agent_id} has no allocated task to complete"
        )));
    }
    if let Some(bar) = agent.acceptance_score {
        let meets_bar = crate::ratchet::adequacy_verdict(
            acceptance_score,
            ACCEPTANCE_SCALE,
            bar as f64 / ACCEPTANCE_SCALE as f64,
            1,
        );
        if !meets_bar {
            return Err(StatusError::Invariant(format!(
                "acceptance score {acceptance_score} is below agent {agent_id} bar {bar}"
            )));
        }
    }
    agent.load -= 1;
    agent.acceptance_score = Some(acceptance_score);
    persist(state_dir, &agents)
}

/// Write a file atomically: a concurrent reader sees the old bytes or the new ones, never a
/// truncated file. `fs::write` truncates first, and readers caught that window -- the residual
/// concurrency failures reported "invalid agent record" for a JSON file mid-write (D54).
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

fn persist(state_dir: &Path, agents: &[AgentStatus]) -> Result<(), StatusError> {
    let agents_dir = state_dir.join("agents");
    for agent in agents {
        let bytes = serde_json::to_vec_pretty(agent).map_err(|error| {
            StatusError::Invariant(format!("cannot encode agent {}: {error}", agent.id))
        })?;
        write_atomic(&agents_dir.join(format!("{}.json", agent.id)), &bytes).map_err(|error| {
            StatusError::Environment(format!("cannot persist agent {}: {error}", agent.id))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        allocate, complete_task, dispatch_verified, load_or_seed, AgentStatus, EvidenceOutcome,
        StatusError, STATES_POSSIBLE,
    };
    use fleet::agent::Scorecard;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;

    fn test_dir(name: &str) -> PathBuf {
        let template = std::env::temp_dir().join(format!("fleet-swarm-{name}.XXXXXX"));
        let output = Command::new("mktemp")
            .arg("-d")
            .arg(template)
            .output()
            .expect("run mktemp");
        assert!(output.status.success(), "mktemp failed");
        PathBuf::from(
            String::from_utf8(output.stdout)
                .expect("mktemp path is utf-8")
                .trim(),
        )
    }

    #[test]
    fn pristine_store_seeds_persistent_per_agent_records() {
        let root = test_dir("seed");
        let agents = load_or_seed(&root).expect("seed swarm");
        assert!(agents.len() >= 2);
        assert_eq!(STATES_POSSIBLE, 15);
        for agent in &agents {
            assert!(root
                .join("agents")
                .join(format!("{}.json", agent.id))
                .is_file());
        }

        let first_path = root.join("agents").join(format!("{}.json", agents[0].id));
        let mut changed: AgentStatus =
            serde_json::from_slice(&fs::read(&first_path).expect("read first agent"))
                .expect("parse first agent");
        changed.state = "Building".to_string();
        fs::write(
            first_path,
            serde_json::to_vec_pretty(&changed).expect("encode changed agent"),
        )
        .expect("write changed agent");

        let reloaded = load_or_seed(&root).expect("reload swarm");
        assert_eq!(reloaded[0].state, "Building");
        assert!(reloaded.iter().skip(1).all(|agent| agent.state == "Intake"));
        fs::remove_dir_all(root).expect("remove test state");
    }

    #[test]
    fn initialized_store_with_zero_agents_refuses() {
        let root = test_dir("zero");
        fs::create_dir_all(root.join("agents")).expect("create empty agent store");
        assert_eq!(load_or_seed(&root), Err(StatusError::ZeroAgents));
        assert_eq!(allocate(&root, 1), Err(StatusError::ZeroAgents));
        fs::remove_dir_all(root).expect("remove test state");
    }

    #[test]
    fn one_agent_is_not_a_swarm() {
        let root = test_dir("one");
        fs::create_dir_all(root.join("agents")).expect("create agent store");
        let agent = AgentStatus {
            id: "builder".to_string(),
            role: "builder".to_string(),
            state: "Intake".to_string(),
            task: "only-task".to_string(),
            capacity: 1,
            load: 0,
            acceptance_score: None,
        };
        fs::write(
            root.join("agents/builder.json"),
            serde_json::to_vec(&agent).expect("encode agent"),
        )
        .expect("write agent");
        assert_eq!(
            load_or_seed(&root),
            Err(StatusError::TooFewAgents { found: 1 })
        );
        fs::remove_dir_all(root).expect("remove test state");
    }

    fn write_agents(root: &Path, agents: &[AgentStatus]) {
        fs::create_dir_all(root.join("agents")).expect("create agent store");
        for agent in agents {
            fs::write(
                root.join("agents").join(format!("{}.json", agent.id)),
                serde_json::to_vec(agent).expect("encode agent"),
            )
            .expect("write agent");
        }
    }

    fn agent(id: &str, role: &str, capacity: u64, load: u64) -> AgentStatus {
        AgentStatus {
            id: id.to_string(),
            role: role.to_string(),
            state: "Intake".to_string(),
            task: "task".to_string(),
            capacity,
            load,
            acceptance_score: None,
        }
    }

    #[test]
    fn over_capacity_is_refused() {
        let root = test_dir("over-capacity");
        write_agents(
            &root,
            &[agent("a", "builder", 1, 0), agent("b", "verifier", 1, 0)],
        );
        assert_eq!(
            allocate(&root, 3),
            Err(StatusError::Invariant(
                "demand exceeds total capacity: requested=3 free=2".to_string()
            ))
        );
        fs::remove_dir_all(root).expect("remove test state");
    }

    #[test]
    fn in_capacity_is_allocated() {
        let root = test_dir("in-capacity");
        write_agents(
            &root,
            &[agent("a", "builder", 2, 0), agent("b", "verifier", 1, 0)],
        );
        let allocation = allocate(&root, 3).expect("allocate within capacity");
        assert_eq!(allocation.requested, 3);
        assert_eq!(allocation.checked, allocation.total);
        assert_eq!(allocation.assignments.values().sum::<u64>(), 3);
        fs::remove_dir_all(root).expect("remove test state");
    }

    #[test]
    fn below_bar_names_submission_and_bar() {
        let root = test_dir("below-bar");
        let mut first = agent("a", "builder", 2, 1);
        first.acceptance_score = Some(8_500);
        write_agents(&root, &[first, agent("b", "verifier", 1, 0)]);
        assert_eq!(
            complete_task(&root, "a", 8_499),
            Err(StatusError::Invariant(
                "acceptance score 8499 is below agent a bar 8500".to_string()
            ))
        );
        fs::remove_dir_all(root).expect("remove test state");
    }

    #[test]
    fn verified_dispatch_advances_distinct_agent_states_and_scorecards() {
        let root = test_dir("dispatch-pass");
        let report = dispatch_verified(
            &root,
            "real-task",
            "stub",
            "stub-builder-v1",
            "stub-verifier-v1",
            false,
            EvidenceOutcome::Verified,
        )
        .expect("dispatch verified work");
        assert_eq!(report.checked, report.total);
        assert!(report.checked >= 2);
        let distinct = report.states.values().collect::<BTreeSet<_>>();
        assert!(
            distinct.len() >= 2,
            "agents must not share one global state"
        );

        let statuses = load_or_seed(&root).expect("reload dispatched swarm");
        assert!(statuses.iter().all(|status| status.task == "real-task"));
        let mut non_zero = 0usize;
        for status in statuses {
            let path = root.join("scorecards").join(format!("{}.json", status.id));
            let scorecard: Scorecard =
                serde_json::from_slice(&fs::read(path).expect("read scorecard"))
                    .expect("parse scorecard");
            assert_eq!(scorecard.checked, 1);
            assert_eq!(scorecard.total, 1);
            assert_eq!(scorecard.credited, 1);
            non_zero += usize::from(scorecard.total > 0);
        }
        assert!(non_zero > 0);
        fs::remove_dir_all(root).expect("remove test state");
    }

    #[test]
    fn lead_code_gate_refuses_code_and_accepts_no_code() {
        let refused_root = test_dir("lead-code-refused");
        assert_eq!(
            dispatch_verified(
                &refused_root,
                "lead-code",
                "stub",
                "builder-model",
                "verifier-model",
                true,
                EvidenceOutcome::Verified,
            ),
            Err(StatusError::Refusal {
                agent: "lead".to_string(),
                reason: "LEAD_WROTE_CODE".to_string(),
            })
        );
        let lead = load_or_seed(&refused_root)
            .expect("load refused swarm")
            .into_iter()
            .find(|status| status.role == "lead")
            .expect("lead status");
        assert_eq!(lead.state, "Refused");
        fs::remove_dir_all(refused_root).expect("remove refused state");

        let accepted_root = test_dir("lead-no-code-accepted");
        assert!(dispatch_verified(
            &accepted_root,
            "lead-no-code",
            "stub",
            "builder-model",
            "verifier-model",
            false,
            EvidenceOutcome::Verified,
        )
        .is_ok());
        fs::remove_dir_all(accepted_root).expect("remove accepted state");
    }

    #[test]
    fn verifier_model_gate_refuses_same_and_accepts_distinct() {
        let same_root = test_dir("same-model-refused");
        assert_eq!(
            dispatch_verified(
                &same_root,
                "same-model",
                "stub",
                "same",
                "same",
                false,
                EvidenceOutcome::Verified,
            ),
            Err(StatusError::Refusal {
                agent: "verifier".to_string(),
                reason: "SELF_VERIFIED".to_string(),
            })
        );
        fs::remove_dir_all(same_root).expect("remove same-model state");

        let distinct_root = test_dir("distinct-model-accepted");
        assert!(dispatch_verified(
            &distinct_root,
            "distinct-model",
            "stub",
            "builder-model",
            "verifier-model",
            false,
            EvidenceOutcome::Verified,
        )
        .is_ok());
        fs::remove_dir_all(distinct_root).expect("remove distinct-model state");
    }

    #[test]
    fn credit_requires_verifying_evidence() {
        for evidence in [EvidenceOutcome::Faulted, EvidenceOutcome::Unknown] {
            let root = test_dir(match evidence {
                EvidenceOutcome::Faulted => "dispatch-faulted-evidence",
                EvidenceOutcome::Unknown => "dispatch-unknown-evidence",
                EvidenceOutcome::Verified => unreachable!(),
            });
            assert!(dispatch_verified(
                &root,
                "unattested-task",
                "stub",
                "builder-model",
                "verifier-model",
                false,
                evidence,
            )
            .is_err());
            for status in load_or_seed(&root).expect("load uncredited swarm") {
                let scorecard: Scorecard = serde_json::from_slice(
                    &fs::read(root.join("scorecards").join(format!("{}.json", status.id)))
                        .expect("read scorecard"),
                )
                .expect("parse scorecard");
                assert_eq!(
                    scorecard.credited, 0,
                    "evidence failure must never earn credit"
                );
                assert_eq!(scorecard.total, 1);
            }
            fs::remove_dir_all(root).expect("remove uncredited state");
        }
    }
}
