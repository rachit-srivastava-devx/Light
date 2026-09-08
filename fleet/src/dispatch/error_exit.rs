//! `DispatchError::exit_code` -- split out of `error.rs` to keep that file ≤80 lines.

use super::error::DispatchError;
use fleet_types::ExitCode;

impl DispatchError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            DispatchError::Router(_) | DispatchError::RoleCheck(_) | DispatchError::Refusal(_) => {
                ExitCode::Refusal
            }
            DispatchError::EnvFault(_) => ExitCode::Env,
            DispatchError::NotYetImplemented(_) => ExitCode::Env,
            DispatchError::Io(_) => ExitCode::Env,
            DispatchError::Merge(e) => e.exit_code(),
            DispatchError::VerifyFailed { code, .. } => *code,
            DispatchError::UnknownGate(_) => ExitCode::Env,
            DispatchError::GateAssets(_) => ExitCode::Env,
            DispatchError::Walk(_) => ExitCode::Env,
            DispatchError::PlanAhead(_) => ExitCode::Invariant,
            DispatchError::Agent(e) => e.exit_code(),
            DispatchError::Adjudicate(e) => e.exit_code(),
            _ => ExitCode::Invariant,
        }
    }
}
