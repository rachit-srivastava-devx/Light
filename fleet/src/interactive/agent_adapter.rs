//! Select the first locally available interactive adapter.

use builder::CliAdapter;
use std::process::{Command, Stdio};

fn on_path(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn detect_adapter() -> CliAdapter {
    [CliAdapter::Claude, CliAdapter::Codex]
        .into_iter()
        .find(|adapter| adapter.cli_binary_name().is_some_and(on_path))
        .unwrap_or(CliAdapter::Freelane)
}
