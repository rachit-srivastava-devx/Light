//! Checks that the `fleet` binary in the shell's PATH is the same executable that is currently
//! running. Detects two problems:
//!
//! 1. No `fleet` in PATH → suggest running `./install.sh`
//! 2. PATH `fleet` resolves to a DIFFERENT binary (stale copy, different repo) → suggest
//!    re-running `./install.sh` which now creates a symlink so this never repeats.
//!
//! Returns None when PATH is correct (fleet in PATH is this binary).

use super::theme::*;
use std::path::Path;

/// Compare the running binary against what `fleet` in PATH resolves to.
/// Returns `Some(notice)` when mismatched; `None` when aligned or undeterminable.
pub fn check(color: bool) -> Option<String> {
    let current = std::env::current_exe().ok()?;
    let current = current.canonicalize().unwrap_or(current);
    let path_fleet = find_in_path("fleet").map(|p| p.canonicalize().unwrap_or(p));
    notice(&current, path_fleet.as_deref(), color)
}

/// Pure decision: given resolved paths, return the appropriate notice or None.
/// Extracted so all three branches are testable without process-global side-effects.
fn notice(current: &Path, path_fleet: Option<&Path>, color: bool) -> Option<String> {
    // FLEET_DIR is the fleet package dir (where install.sh lives), not the git root.
    // {:?} quotes and escapes the path, handling spaces in directory names.
    let install_cmd = format!("cd {:?} && ./install.sh", env!("FLEET_DIR"));

    match path_fleet {
        None => {
            // No `fleet` in PATH at all — suggest install.sh rather than silently returning None.
            Some(format!(
                "  {} {} Run {}",
                paint(color, AMBER, "⚠"),
                paint(color, AMBER, "`fleet` not found in PATH."),
                paint(color, BOLD, &install_cmd),
            ))
        }
        Some(pf) if current == pf => None, // aligned — nothing to report
        Some(pf) => Some(format!(
            "  {} PATH fleet: {} — this binary: {}\n  {} Run {}",
            paint(color, AMBER, "⚠"),
            paint(color, BOLD, &pf.display().to_string()),
            paint(color, BOLD, &current.display().to_string()),
            paint(color, AMBER, " "),
            paint(color, BOLD, &install_cmd),
        )),
    }
}

/// Walk `PATH` entries and return the first path where a file named `name` is executable.
fn find_in_path(name: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "path_check_tests.rs"]
mod tests;
