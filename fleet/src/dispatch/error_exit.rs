//! `DispatchError::exit_code` -- split out of `error.rs` to keep that file ≤80 lines.
//! Also holds `NotYetImplementedPayload`, the wire shape `--json` consumers key off when a
//! subcommand is a typed stub. Kept next to `exit_code()` so both live outside `error.rs`.

use super::error::DispatchError;
use fleet_types::ExitCode;

/// Machine-readable payload for `DispatchError::NotYetImplemented`. `subcommand` is stable
/// (`"attest"`, `"pr"`, `"mcp"`, …); `reason` mirrors the current human-facing message so
/// callers that already parsed the free-form string keep working unchanged.
#[derive(serde::Serialize, Debug, PartialEq, Eq)]
#[allow(dead_code)] // fields are the `--json` wire shape; consumed by consumers of the payload
pub struct NotYetImplementedPayload {
    pub subcommand: &'static str,
    pub reason: &'static str,
}

impl DispatchError {
    /// `Some(payload)` when the error is a typed stub, else `None`. Callers that emit `--json`
    /// diagnostics can render this alongside the exit code without re-parsing `Display`.
    #[allow(dead_code)] // pub API surface for `--json` consumers; `main.rs` still uses `Display`
    pub fn not_yet_implemented_payload(&self) -> Option<NotYetImplementedPayload> {
        match self {
            DispatchError::NotYetImplemented { subcommand, tracking } => Some(NotYetImplementedPayload {
                subcommand,
                reason: tracking.unwrap_or("owning crate exposes no public entry point yet"),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands};
    use crate::dispatch::ops_cmd::not_yet_implemented;
    use clap::Parser;
    use fleet_types::ExitCode;

    /// Round-trip: an `attest` invocation goes through `dispatch::ops_cmd::not_yet_implemented`,
    /// which is what `dispatch::run`'s catch-all arm calls. The resulting error MUST expose a
    /// JSON payload with the stable `subcommand: "attest"` field AND still map to exit code 3
    /// (EnvFault) so scripts that only observe exit codes keep working unchanged.
    #[test]
    fn attest_round_trip_emits_stable_subcommand_json() {
        let cli = Cli::try_parse_from(["fleet", "attest", "--artifact", "deadbeef"])
            .expect("`fleet attest --artifact` parses");
        let err = not_yet_implemented(&cli.command);
        assert_eq!(err.exit_code(), ExitCode::Env, "exit code MUST stay 3 (EnvFault)");
        let payload = err.not_yet_implemented_payload().expect("typed stub payload");
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap();
        assert_eq!(json["subcommand"], "attest", "stable subcommand id");
        assert!(json["reason"].as_str().unwrap().contains("fleet-types"), "reason still present");
        assert!(matches!(cli.command, Commands::Attest(_)), "guard: variant is Attest");
    }
}

impl DispatchError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            DispatchError::Router(_) | DispatchError::RoleCheck(_) | DispatchError::Refusal(_) => {
                ExitCode::Refusal
            }
            DispatchError::EnvFault(_) => ExitCode::Env,
            DispatchError::NotYetImplemented { .. } => ExitCode::Env,
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
