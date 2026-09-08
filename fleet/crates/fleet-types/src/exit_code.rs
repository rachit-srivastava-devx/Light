//! `ExitCode` -- fleet's process exit-code taxonomy, collapsing 30+ `EXIT_*` redeclarations
//! across fleet's monolith into one source of truth.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ExitCode {
    /// Success.
    Ok = 0,
    /// The environment (filesystem, env var, subprocess) did not provide what the caller needed.
    Env = 3,
    /// An internal invariant assumed to always hold did not hold.
    Invariant = 6,
    /// A gate deliberately refused -- the expected, auditable "no" outcome, never an error.
    Refusal = 7,
    /// A comparison against a committed baseline did not match.
    Mismatch = 8,
    /// Not a failure -- ready and waiting on a human review gate before proceeding.
    ReadyAwaitingReview = 9,
}

/// An `i32` did not match any known `ExitCode` variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0} is not a recognised fleet exit code")]
pub struct UnknownExitCode(pub i32);

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for ExitCode {
    type Error = UnknownExitCode;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            3 => Ok(Self::Env),
            6 => Ok(Self::Invariant),
            7 => Ok(Self::Refusal),
            8 => Ok(Self::Mismatch),
            9 => Ok(Self::ReadyAwaitingReview),
            other => Err(UnknownExitCode(other)),
        }
    }
}
