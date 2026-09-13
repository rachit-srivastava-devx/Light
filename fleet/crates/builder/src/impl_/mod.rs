mod adapter;
pub mod freelane;
mod outcome;
mod probe;
mod reap;
mod reap_sweep;
mod request;
mod sandbox;
mod scorecard;
mod scorecard_io;
mod spawn;

pub use adapter::{CliAdapter, UnknownAgentKind};
pub use merge::MergeOutcome;
pub use outcome::{LaneOutcome, NoAmbientProbeResult};
pub use probe::probe_no_ambient;
pub use reap::find_dead_lanes;
pub use reap_sweep::reap_dead_lanes;
pub use request::{JoinError, LaneHandle, SpawnError, SpawnRequest};
pub use sandbox::{
    resolve_hermetic_provision, scaffold::scaffold_fleet_dir, scaffold::ScaffoldError,
    HermeticProvision, ProvisionError,
};
pub use scorecard::{Scorecard, ScorecardOutcome};
pub use scorecard_io::{record_scorecard_outcome, scorecard_path};
pub use spawn::{fd3, join, spawn, MergePolicy};
pub use types::Role;
