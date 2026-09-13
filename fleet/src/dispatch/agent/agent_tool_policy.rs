//! Provider-native tool restriction compiled from Fleet's approved effect plan.

use std::process::Command;

const READ_TOOLS: &str = "Read,Glob,Grep";
const WRITE_TOOLS: &str = "Read,Glob,Grep,Edit,Write";
const COMMAND_TOOLS: &str = concat!(
    "Read,Glob,Grep,",
    "Bash(cargo test *),Bash(cargo check *),Bash(cargo clippy *),Bash(cargo fmt *),",
    "Bash(git status *),Bash(git diff *),Bash(git log *),Bash(git show *),",
    "Bash(rg *),Bash(find *),Bash(wc *)"
);
const WRITE_COMMAND_TOOLS: &str = concat!(
    "Read,Glob,Grep,Edit,Write,",
    "Bash(cargo test *),Bash(cargo check *),Bash(cargo clippy *),Bash(cargo fmt *),",
    "Bash(git status *),Bash(git diff *),Bash(git log *),Bash(git show *),",
    "Bash(rg *),Bash(find *),Bash(wc *)"
);

#[derive(Clone, Copy)]
pub struct AgentToolPolicy {
    write: bool,
    command: bool,
    restricted: bool,
}

impl AgentToolPolicy {
    pub const fn approved(write: bool, command: bool) -> Self {
        Self {
            write,
            command,
            restricted: true,
        }
    }

    pub const fn worker_default() -> Self {
        Self {
            write: true,
            command: true,
            restricted: false,
        }
    }

    pub fn configure_claude(self, command: &mut Command) {
        if !self.restricted {
            return;
        }
        let tools = match (self.write, self.command) {
            (false, false) => READ_TOOLS,
            (true, false) => WRITE_TOOLS,
            (false, true) => COMMAND_TOOLS,
            (true, true) => WRITE_COMMAND_TOOLS,
        };
        command.args([
            "--restricted",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--permission-mode",
            "dontAsk",
            "--permission-prompts",
            "none",
        ]);
        // `--restricted` only re-enables a removed tool (Bash included) by its bare name in
        // `--tools`; a pattern like `Bash(git log *)` there is not recognized and Bash stays
        // off, so every command-effect task silently degraded to read-only with no error --
        // verified live: `claude --restricted --tools="...,Bash(git log *)"` reported no Bash
        // tool at all. `--allowedTools` is the separate, correct place for the fine-grained
        // per-command patterns, and still enforces them once the tool is actually enabled.
        command.arg(format!("--tools={}", bare_tool_names(tools)));
        command.arg(format!("--allowedTools={tools}"));
    }
}

/// `"Read,Glob,Grep,Bash(cargo test *),Bash(git log *)"` -> `"Read,Glob,Grep,Bash"`.
fn bare_tool_names(tools: &str) -> String {
    let mut names: Vec<&str> = Vec::new();
    for entry in tools.split(',') {
        let name = entry.split('(').next().unwrap_or(entry);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.join(",")
}

#[cfg(test)]
#[path = "agent_tool_policy_tests.rs"]
mod tests;
