//! Where fleet looks for an external tool, and what it says when it cannot find one.
//!
//! `which <tool>` searches `$PATH` and nothing else. rustup installs cargo into `~/.cargo/bin`
//! and adds it to `PATH` from the shell profile -- which a GUI-launched process, a cron job, a CI
//! runner, or a non-login shell never sources. On such a machine `fleet doctor` printed
//! `cargo: missing` with cargo 1.98.1 installed, and the unit-tests gate degraded to a SKIP
//! ("unavailable", i.e. "go install it") rather than naming a fixable environment problem. So
//! this falls back to rustup's locations, and a genuine miss names everywhere it looked
//! (rule 7: a missing tool is an environment fault, and must be actionable).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Searched after `$PATH`, in order: `$CARGO_HOME/bin`, then `~/.cargo/bin`.
pub fn fallback_dirs() -> Vec<PathBuf> {
    let cargo_home = std::env::var_os("CARGO_HOME").map(|h| PathBuf::from(h).join("bin"));
    let rustup = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo").join("bin"));
    let mut dirs: Vec<PathBuf> = cargo_home.into_iter().chain(rustup).collect();
    dirs.dedup();
    dirs
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let mode = |m: std::fs::Metadata| !m.is_dir() && m.permissions().mode() & 0o111 != 0;
    std::fs::metadata(path).map(mode).unwrap_or(false)
}

/// Absolute path to `name`: `$PATH` first (an operator's own override still wins), then rustup's.
/// `None` means it is genuinely not installed anywhere fleet knows to look.
pub fn find(name: &str) -> Option<PathBuf> {
    let out = Command::new("which").arg(name).output();
    let on_path = out.ok().filter(|o| o.status.success()).map(|o| o.stdout).unwrap_or_default();
    let on_path = String::from_utf8_lossy(&on_path).trim().to_string();
    if !on_path.is_empty() {
        return Some(PathBuf::from(on_path));
    }
    fallback_dirs().into_iter().map(|d| d.join(name)).find(|p| is_executable(p))
}

pub fn found(name: &str) -> bool {
    find(name).is_some()
}

/// Everywhere `find` looked, for a "not found" message a reader can act on.
pub fn searched() -> String {
    let mut places = vec!["$PATH".to_string()];
    places.extend(fallback_dirs().iter().map(|d| d.display().to_string()));
    places.join(", ")
}

/// The argv[0] a gate should actually spawn: a bare name resolvable only in a rustup directory
/// becomes that absolute path, else a probe answering "cargo is installed" would be followed by a
/// spawn failing with ENOENT. A name already containing a separator is returned untouched.
pub fn resolve_bin(bin: &str) -> String {
    if bin.contains(std::path::MAIN_SEPARATOR) {
        return bin.to_string();
    }
    find(bin).map(|p| p.display().to_string()).unwrap_or_else(|| bin.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searched_names_path_and_the_rustup_locations() {
        let text = searched();
        assert!(text.starts_with("$PATH"), "got {text}");
        assert!(text.contains(".cargo/bin") || std::env::var_os("HOME").is_none(), "got {text}");
    }

    /// A resolved `GateCommand::Script` path must never be rewritten by the bare-name lookup.
    #[test]
    fn a_path_bearing_argv0_is_never_rewritten() {
        assert_eq!(resolve_bin("/opt/gates/run.sh"), "/opt/gates/run.sh");
    }
}
