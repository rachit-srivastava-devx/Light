//! `CliAdapter` -- which CLI family a lane drives. Mirrors `main.rs::agent_command`'s match
//! arms (`"claude" | "codex"` today; `"freelane"` is the keyless fallback).

/// Which CLI family a lane drives.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CliAdapter {
    Claude,
    Codex,
    Freelane,
}

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
}
