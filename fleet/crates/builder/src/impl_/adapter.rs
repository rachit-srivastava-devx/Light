//! `CliAdapter` -- which CLI family a lane drives. Mirrors `main.rs::agent_command`'s match
//! arms (`"claude" | "codex"` today; `"freelane"` is the keyless fallback).

/// Which CLI family a lane drives.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CliAdapter {
    Claude,
    Codex,
    Freelane,
}

/// An `__agent <kind>` token did not match any known `CliAdapter`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown agent kind {0:?}")]
pub struct UnknownAgentKind(pub String);

impl CliAdapter {
    /// The executable name this crate checks for on `PATH` before spawning
    /// (`main.rs:3584`'s `which_on_path(cli)` check). `Freelane` has no CLI binary of its own --
    /// it shells out to the repo-committed `bin/freelane.sh` script instead, so this returns
    /// `None` for it.
    pub fn cli_binary_name(self) -> Option<&'static str> {
        match self {
            CliAdapter::Claude => Some("claude"),
            CliAdapter::Codex => Some("codex"),
            CliAdapter::Freelane => None,
        }
    }

    /// The `__agent <kind>` subcommand token this adapter maps to when the parent re-execs
    /// itself for the child side. Purely a wire-protocol label -- the child dispatch itself
    /// lives in `src/`, not here (see BLUEPRINT.md's divergence note 1).
    pub fn agent_kind(self) -> &'static str {
        match self {
            CliAdapter::Claude => "claude",
            CliAdapter::Codex => "codex",
            CliAdapter::Freelane => "freelane",
        }
    }

    /// The inverse of `agent_kind`: parses the `__agent <kind>` wire token the child process
    /// receives back into a `CliAdapter`. Typed error on anything else -- never a silent
    /// default, per the child dispatch's contract (see `src/dispatch/agent_cmd.rs`).
    pub fn from_agent_kind(kind: &str) -> Result<Self, UnknownAgentKind> {
        match kind {
            "claude" => Ok(CliAdapter::Claude),
            "codex" => Ok(CliAdapter::Codex),
            "freelane" => Ok(CliAdapter::Freelane),
            other => Err(UnknownAgentKind(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_binary_name_is_none_only_for_freelane() {
        assert_eq!(CliAdapter::Claude.cli_binary_name(), Some("claude"));
        assert_eq!(CliAdapter::Codex.cli_binary_name(), Some("codex"));
        assert_eq!(CliAdapter::Freelane.cli_binary_name(), None);
    }

    #[test]
    fn from_agent_kind_round_trips_every_variant() {
        for adapter in [CliAdapter::Claude, CliAdapter::Codex, CliAdapter::Freelane] {
            let kind = adapter.agent_kind();
            assert_eq!(CliAdapter::from_agent_kind(kind), Ok(adapter));
        }
    }

    #[test]
    fn from_agent_kind_rejects_unknown_token() {
        assert_eq!(
            CliAdapter::from_agent_kind("bogus"),
            Err(UnknownAgentKind("bogus".to_string()))
        );
    }
}
