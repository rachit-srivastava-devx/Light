//! `DispatchError` -- the seam wrapping every crate's own typed error for exit-code mapping
//! (BLUEPRINT §11: no `.map_err(|_| EXIT_CODE)` collapsing a real error into an opaque code).

use fleet_types::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("router refused: {0:?}")]
    Router(fleet_router::Refusal),
    #[error("role check refused: {0}")]
    RoleCheck(&'static str),
    #[error(transparent)]
    Admit(#[from] fleet_govern::AdmitError),
    #[error(transparent)]
    Settle(#[from] fleet_govern::SettleError),
    #[error(transparent)]
    Meter(#[from] fleet_govern::MeterIoError),
    #[error(transparent)]
    Spawn(#[from] fleet_worker::SpawnError),
    #[error(transparent)]
    Join(#[from] fleet_worker::JoinError),
    /// `GateRefusal` does not implement `std::error::Error` (it is a plain wire-shape struct),
    /// so this wraps it by value rather than via `#[from]`/`transparent`.
    #[error("lifecycle refused: {}", .0.code())]
    Lifecycle(fleet_types::GateRefusal),
    #[error(transparent)]
    Provision(#[from] fleet_worker::ProvisionError),
    #[error(transparent)]
    Merge(#[from] fleet_merge::WorktreeError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("this subcommand's owning crate does not yet expose a public entry point: {0}")]
    NotYetImplemented(&'static str),
    #[error("{0}")]
    Refusal(String),
}

impl DispatchError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            DispatchError::Router(_) | DispatchError::RoleCheck(_) | DispatchError::Refusal(_) => {
                ExitCode::Refusal
            }
            DispatchError::NotYetImplemented(_) => ExitCode::Env,
            DispatchError::Io(_) => ExitCode::Env,
            _ => ExitCode::Invariant,
        }
    }
}
