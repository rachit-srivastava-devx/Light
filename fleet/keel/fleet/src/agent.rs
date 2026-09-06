#![forbid(unsafe_code)]

//! Typed agent assignments.
//!
//! Each transition consumes its predecessor, so a caller cannot reuse an old
//! state or call a later transition early.
//!
//! ```compile_fail
//! use fleet::agent::Assignment;
//! use fleet::agent::Assigned;
//!
//! fn skipped_transition(assignment: Assignment<Assigned>) {
//!     let _ = assignment.work();
//! }
//! ```

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

const EXIT_ENV: i32 = 3;
const EXIT_INVARIANT: i32 = 6;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AgentFile {
    agent_id: String,
    role: String,
    charter: String,
    capabilities: Vec<String>,
    skills: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Agent {
    inner: AgentFile,
}

impl Agent {
    pub fn agent_id(&self) -> &str {
        &self.inner.agent_id
    }
    pub fn role(&self) -> &str {
        &self.inner.role
    }
    pub fn charter(&self) -> &str {
        &self.inner.charter
    }
    pub fn capabilities(&self) -> &[String] {
        &self.inner.capabilities
    }
    pub fn skills(&self) -> &[String] {
        &self.inner.skills
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryFile {
    agents: Vec<AgentFile>,
}

#[derive(Clone, Debug)]
pub struct Registry {
    agents: Vec<Agent>,
}

impl Registry {
    pub(crate) fn from_toml(text: &str) -> Result<Self, i32> {
        let parsed: RegistryFile = toml::from_str(text).map_err(|_| EXIT_INVARIANT)?;
        if parsed.agents.is_empty()
            || parsed.agents.iter().any(|agent| {
                agent.agent_id.trim().is_empty()
                    || agent.role.trim().is_empty()
                    || agent.charter.trim().is_empty()
                    || agent.capabilities.is_empty()
                    || agent.skills.is_empty()
            })
        {
            return Err(EXIT_INVARIANT);
        }
        Ok(Self {
            agents: parsed
                .agents
                .into_iter()
                .map(|inner| Agent { inner })
                .collect(),
        })
    }

    pub fn load(path: &Path) -> Result<Self, i32> {
        let text = fs::read_to_string(path).map_err(|_| EXIT_ENV)?;
        Self::from_toml(&text)
    }

    pub fn load_default() -> Result<Self, i32> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("agents.toml");
        Self::load(&path)
    }

    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    pub fn assignment(&self, agent_id: &str, assignment_id: &str) -> Result<Assignment<Idle>, i32> {
        let agent = self
            .agents
            .iter()
            .find(|agent| agent.agent_id() == agent_id)
            .cloned()
            .ok_or(EXIT_INVARIANT)?;
        if assignment_id.trim().is_empty() {
            return Err(EXIT_INVARIANT);
        }
        Ok(Assignment {
            agent,
            assignment_id: assignment_id.to_string(),
            charter: String::new(),
            _state: PhantomData,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scorecard {
    pub agent_id: String,
    pub charter: String,
    pub credited: u64,
    pub faulted: u64,
    pub unknown: u64,
    pub checked: u64,
    pub total: u64,
}

impl Scorecard {
    fn empty(agent: &Agent) -> Self {
        Self {
            agent_id: agent.agent_id().to_string(),
            charter: agent.charter().to_string(),
            credited: 0,
            faulted: 0,
            unknown: 0,
            checked: 0,
            total: 0,
        }
    }

    pub fn record_credited(&mut self) -> Result<(), i32> {
        self.credited = self.credited.checked_add(1).ok_or(EXIT_INVARIANT)?;
        self.refresh()
    }
    pub fn record_faulted(&mut self) -> Result<(), i32> {
        self.faulted = self.faulted.checked_add(1).ok_or(EXIT_INVARIANT)?;
        self.refresh()
    }
    pub fn record_unknown(&mut self) -> Result<(), i32> {
        self.unknown = self.unknown.checked_add(1).ok_or(EXIT_INVARIANT)?;
        self.refresh()
    }

    fn refresh(&mut self) -> Result<(), i32> {
        self.checked = self
            .credited
            .checked_add(self.faulted)
            .ok_or(EXIT_INVARIANT)?;
        self.total = self
            .checked
            .checked_add(self.unknown)
            .ok_or(EXIT_INVARIANT)?;
        Ok(())
    }
}

pub fn scorecard_path(agent_id: &str) -> Result<PathBuf, i32> {
    let state = env::var_os("FLEET_STATE")
        .map(PathBuf::from)
        .ok_or(EXIT_ENV)?;
    Ok(state.join("scorecards").join(format!("{}.json", agent_id)))
}

fn load_scorecard(agent: &Agent) -> Result<Scorecard, i32> {
    let path = scorecard_path(agent.agent_id())?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            let scorecard: Scorecard = serde_json::from_str(&text).map_err(|_| EXIT_INVARIANT)?;
            if scorecard.agent_id != agent.agent_id() || scorecard.charter != agent.charter() {
                return Err(EXIT_INVARIANT);
            }
            if scorecard.checked
                != scorecard
                    .credited
                    .checked_add(scorecard.faulted)
                    .ok_or(EXIT_INVARIANT)?
                || scorecard.total
                    != scorecard
                        .checked
                        .checked_add(scorecard.unknown)
                        .ok_or(EXIT_INVARIANT)?
            {
                return Err(EXIT_INVARIANT);
            }
            Ok(scorecard)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Scorecard::empty(agent)),
        Err(_) => Err(EXIT_ENV),
    }
}

fn persist_scorecard(scorecard: &Scorecard) -> Result<(), i32> {
    let path = scorecard_path(&scorecard.agent_id)?;
    let parent = path.parent().ok_or(EXIT_ENV)?;
    fs::create_dir_all(parent).map_err(|_| EXIT_ENV)?;
    let bytes = serde_json::to_vec_pretty(scorecard).map_err(|_| EXIT_INVARIANT)?;
    fs::write(path, bytes).map_err(|_| EXIT_ENV)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScorecardOutcome {
    Credited,
    Faulted,
    Unknown,
}

/// Record one attributable outcome for an agent and publish its denominator.
pub fn record_scorecard_outcome(
    state_dir: &Path,
    agent_id: &str,
    outcome: ScorecardOutcome,
) -> Result<Scorecard, i32> {
    let registry = Registry::load_default()?;
    let agent = registry
        .agents()
        .iter()
        .find(|agent| agent.agent_id() == agent_id)
        .ok_or(EXIT_INVARIANT)?;
    let path = state_dir
        .join("scorecards")
        .join(format!("{}.json", agent.agent_id()));
    // D54b: this read-modify-write had NO lock. Concurrent dispatches lost updates, and a reader
    // that caught a half-written file failed to parse it -- "cannot record verified agent
    // scorecard", 1 in 8 under load. Serialise the whole read-modify-write on a per-agent lock.
    fs::create_dir_all(path.parent().ok_or(EXIT_INVARIANT)?).map_err(|_| EXIT_ENV)?;
    let lock_path = path.with_extension("lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|_| EXIT_ENV)?;
    lock.lock_exclusive().map_err(|_| EXIT_ENV)?;
    let outcome_result = (|| {
        let mut scorecard = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Scorecard>(&text).map_err(|_| EXIT_INVARIANT)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Scorecard::empty(agent),
            Err(_) => return Err(EXIT_ENV),
        };
        if scorecard.agent_id != agent.agent_id() || scorecard.charter != agent.charter() {
            return Err(EXIT_INVARIANT);
        }
        if scorecard.checked
            != scorecard
                .credited
                .checked_add(scorecard.faulted)
                .ok_or(EXIT_INVARIANT)?
            || scorecard.total
                != scorecard
                    .checked
                    .checked_add(scorecard.unknown)
                    .ok_or(EXIT_INVARIANT)?
        {
            return Err(EXIT_INVARIANT);
        }
        match outcome {
            ScorecardOutcome::Credited => scorecard.record_credited()?,
            ScorecardOutcome::Faulted => scorecard.record_faulted()?,
            ScorecardOutcome::Unknown => scorecard.record_unknown()?,
        }
        let parent = path.parent().ok_or(EXIT_ENV)?;
        fs::create_dir_all(parent).map_err(|_| EXIT_ENV)?;
        let bytes = serde_json::to_vec_pretty(&scorecard).map_err(|_| EXIT_INVARIANT)?;
        fs::write(path, bytes).map_err(|_| EXIT_ENV)?;
        Ok(scorecard)
    })();
    let _ = FileExt::unlock(&lock);
    outcome_result
}

pub fn list() -> Result<(), i32> {
    let registry = Registry::load_default()?;
    for agent in registry.agents() {
        let scorecard = load_scorecard(agent)?;
        persist_scorecard(&scorecard)?;
        println!(
            "agent_id={} role={} charter={}",
            agent.agent_id(),
            agent.role(),
            agent.charter()
        );
        println!(
            "  capabilities={:?} skills={:?}",
            agent.capabilities(),
            agent.skills()
        );
        println!(
            "  scorecard={{credited:{}, faulted:{}, unknown:{}, checked:{}, total:{}}}",
            scorecard.credited,
            scorecard.faulted,
            scorecard.unknown,
            scorecard.checked,
            scorecard.total
        );
    }
    Ok(())
}

pub struct Idle;
pub struct Assigned;
pub struct Briefed;
pub struct Working;
pub struct Submitted;
pub struct Adjudicated;
pub struct Credited;
pub struct Faulted;
pub struct Amended;

#[derive(Debug)]
pub struct Assignment<S> {
    agent: Agent,
    assignment_id: String,
    charter: String,
    _state: PhantomData<S>,
}

impl Assignment<Idle> {
    pub fn assign(mut self, charter: impl Into<String>) -> Assignment<Assigned> {
        self.charter = charter.into();
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: self.charter,
            _state: PhantomData,
        }
    }
}

impl Assignment<Assigned> {
    pub fn brief(self, brief: impl Into<String>) -> Assignment<Briefed> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: brief.into(),
            _state: PhantomData,
        }
    }
}

impl Assignment<Briefed> {
    pub fn work(self) -> Assignment<Working> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: self.charter,
            _state: PhantomData,
        }
    }
}

impl Assignment<Working> {
    pub fn submit(self, submission: impl Into<String>) -> Assignment<Submitted> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: submission.into(),
            _state: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Credit,
    Fault,
}

impl Assignment<Submitted> {
    pub fn adjudicate(self, verdict: Verdict) -> Assignment<Adjudicated> {
        let marker = match verdict {
            Verdict::Credit => "credit",
            Verdict::Fault => "fault",
        };
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: marker.to_string(),
            _state: PhantomData,
        }
    }
}

impl Assignment<Adjudicated> {
    pub fn credit(self) -> Assignment<Credited> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: self.charter,
            _state: PhantomData,
        }
    }
    pub fn fault(self) -> Assignment<Faulted> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: self.charter,
            _state: PhantomData,
        }
    }
}

impl Assignment<Credited> {
    pub fn amend(self, amendment: impl Into<String>) -> Assignment<Amended> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: amendment.into(),
            _state: PhantomData,
        }
    }
}

impl Assignment<Faulted> {
    pub fn amend(self, amendment: impl Into<String>) -> Assignment<Amended> {
        Assignment {
            agent: self.agent,
            assignment_id: self.assignment_id,
            charter: amendment.into(),
            _state: PhantomData,
        }
    }
}

impl<S> Assignment<S> {
    pub fn agent_id(&self) -> &str {
        self.agent.agent_id()
    }
    pub fn assignment_id(&self) -> &str {
        &self.assignment_id
    }
    pub fn charter(&self) -> &str {
        &self.charter
    }
}

#[cfg(test)]
mod tests {
    use super::Scorecard;

    #[test]
    fn unknown_is_published_but_never_credited() {
        let mut scorecard = Scorecard {
            agent_id: "agent".to_string(),
            charter: "charter".to_string(),
            credited: 0,
            faulted: 0,
            unknown: 0,
            checked: 0,
            total: 0,
        };
        scorecard.record_unknown().expect("record unknown");
        assert_eq!(scorecard.credited, 0);
        assert_eq!(scorecard.checked, 0);
        assert_eq!(scorecard.total, 1);
    }
}
