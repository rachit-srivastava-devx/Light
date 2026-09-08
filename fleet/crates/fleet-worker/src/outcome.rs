//! `LaneOutcome`/`NoAmbientProbeResult` -- results read off the fd-3 channel. Data only.

use fleet_types::Tokens;
use std::path::PathBuf;

/// The authoritative outcome of one lane, read off fd-3. Stdout/stderr are NEVER consulted.
#[derive(Clone, Debug)]
pub enum LaneOutcome {
    /// The CLI completed and reported `kind: "done"` on fd-3, `validate_submission`-clean.
    Done {
        resolved_model: Option<String>,
        tokens: Option<Tokens>,
        body: serde_json::Value,
    },
    /// The CLI reported `kind: "refuse"` on fd-3 -- an expected, typed non-success.
    Refused { reason: String },
    /// fd-3 delivered nothing, a malformed packet, or the deadline fired first.
    EnvironmentFault { detail: String },
}

/// Result of the M1 keyless-thesis experiment (`probe_no_ambient`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoAmbientProbeResult {
    /// The CLI correctly refused or failed for lack of credentials.
    CleanlyBlocked { detail: String },
    /// The CLI produced a `Done` result with no ambient credentials visible to it -- requires
    /// human judgment (§12) to classify as genuine keyless capability vs. a credential leak.
    UnexpectedSuccess {
        detail: String,
        evidence_paths: Vec<PathBuf>,
    },
}
