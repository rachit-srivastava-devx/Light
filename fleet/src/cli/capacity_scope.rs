//! Which subcommands the capacity preflight is allowed to REFUSE.
//!
//! The preflight was originally applied to every command, which broke real things: `fleet
//! completions zsh` (emitting a text file -- `install.sh:178`) was refused for machine load, so the
//! installer exited 7 after a successful install. Worse, `__agent` is the spawned CHILD; gating it
//! meant a loaded machine let the parent through and then killed every worker it started.
//!
//! The rule: refuse only where refusing SAVES something -- commands that spawn workers, run gates,
//! or fan out across cores. Introspection must always answer, and `doctor` most of all: a
//! diagnostic that refuses to run when the machine is unhealthy is useless exactly when needed.

use super::Commands;

impl Commands {
    /// `true` when this command spawns processes, runs gates, or saturates cores -- the cases where
    /// a refusal prevents real harm. Everything else answers regardless of load.
    pub fn is_capacity_gated(&self) -> bool {
        matches!(
            self,
            Commands::Run(_)
                | Commands::Swarm(_)
                | Commands::Oracle(_)
                | Commands::Gate(_)
                | Commands::Graph(_)
                | Commands::Impact(_)
                | Commands::PipelineProbe { .. }
                | Commands::PlanAheadProbe(_)
                | Commands::SpawnProbe(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    fn cmd(args: &[&str]) -> Commands {
        Cli::parse_from(args).command
    }

    /// Regression pin for the install failure: these do no work and must never be refused.
    #[test]
    fn introspection_commands_are_never_capacity_gated() {
        for args in [
            vec!["fleet", "completions", "zsh"],
            vec!["fleet", "doctor"],
            vec!["fleet", "status"],
            vec!["fleet", "version"],
            vec!["fleet", "roles"],
            vec!["fleet", "__capacity_probe"],
        ] {
            assert!(!cmd(&args).is_capacity_gated(), "{args:?} must not be gated");
        }
    }

    /// The spawned child must never be gated: the parent already passed the check, and refusing
    /// here kills workers on exactly the loaded machines the parent was allowed to run on.
    #[test]
    fn the_spawned_child_is_never_capacity_gated() {
        let c = cmd(&["fleet", "__agent", "freelane", "/tmp", "task"]);
        assert!(!c.is_capacity_gated());
    }

    #[test]
    fn work_spawning_commands_are_gated() {
        for args in [
            vec!["fleet", "run", "--repo", "/tmp", "--task", "t"],
            vec!["fleet", "oracle"],
            vec!["fleet", "graph", "--repo", "/tmp"],
        ] {
            assert!(cmd(&args).is_capacity_gated(), "{args:?} must be gated");
        }
    }
}
