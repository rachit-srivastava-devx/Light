//! Async check for newer commits on the git remote, shown once at REPL startup.
//!
//! Uses the repo path embedded at BUILD TIME (not CWD) so the check works for the installed
//! binary even when the shell's cwd is unrelated to this repo. Returns None silently on any
//! failure — network issues, missing git, moved repo — rather than printing an error or blocking.

use super::theme::*;

/// Remote SHA prefix length to compare; matches the 12-char short sha in build_info.
const SHA_LEN: usize = 12;

/// Check if the git remote has commits the running binary wasn't built from.
///
/// Runs `git ls-remote origin` from the embedded repo path, with a 2-second wall-clock
/// timeout. Returns `Some(notice)` when the remote is ahead; `None` when up-to-date, or on any
/// error/timeout (all silent — update check must never break the REPL startup).
pub async fn check(color: bool) -> Option<String> {
    let local_sha = crate::build_info::IDENTITY.commit_sha;
    if local_sha == "unknown" {
        return None; // built outside a checkout; nothing meaningful to compare
    }
    let repo = env!("FLEET_REPO_PATH");
    // Verify repo path still exists before spawning git (it may have moved post-install).
    if !std::path::Path::new(repo).join(".git").exists() {
        return None;
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        run_ls_remote(repo),
    )
    .await
    .ok()??;

    // `ls-remote origin` lists ALL remote refs. If the local SHA prefix matches any ref,
    // this binary is on a published commit — no update needed. Comparing against only
    // `origin HEAD` (the default branch) fires falsely when working on a feature branch
    // that is ahead of main: different SHA ≠ outdated.
    if sha_in_remote(local_sha, &result) {
        return None;
    }
    // {:?} quotes and escapes the path, handling spaces in directory names.
    // FLEET_DIR is the fleet package dir (where install.sh lives), not the git root.
    let cmd = format!("cd {:?} && cargo build --release && ./install.sh", env!("FLEET_DIR"));
    Some(format!(
        "  {} {} Run {}",
        paint(color, AMBER, "↑"),
        paint(color, AMBER, "This build's commit is not on origin."),
        paint(color, BOLD, &cmd),
    ))
}

/// True if `local_sha` (12-char prefix) matches the 12-char prefix of any SHA in `ls-remote` output.
fn sha_in_remote(local_sha: &str, ls_remote_output: &str) -> bool {
    ls_remote_output.lines().any(|line| {
        line.split_whitespace()
            .next()
            // .get() instead of [..] avoids a panic on any non-ASCII first token.
            .and_then(|s| s.get(..SHA_LEN))
            .map_or(false, |remote_prefix| remote_prefix == local_sha)
    })
}

async fn run_ls_remote(repo: &str) -> Option<String> {
    let out = tokio::process::Command::new("git")
        .args(["-C", repo, "ls-remote", "origin"])
        // Never prompt for credentials — if auth fails, return None silently.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        // Kill the git process when the timeout future is dropped, not just abandon it.
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_in_remote_finds_sha_on_feature_branch() {
        // Local built from dev which is 3 commits ahead of main — ls-remote still shows dev ref.
        let output = concat!(
            "175fb18284d0abc123def456\trefs/heads/dev\n",
            "2441646abaed1234567890ab\trefs/heads/main\n",
            "2441646abaed1234567890ab\tHEAD\n",
        );
        assert!(sha_in_remote("175fb18284d0", output), "dev sha must be found via refs/heads/dev");
    }

    #[test]
    fn sha_in_remote_returns_false_when_commit_not_on_remote() {
        // Commit exists only locally (not pushed, or remote has moved on).
        let output = concat!(
            "2441646abaed1234567890ab\trefs/heads/main\n",
            "2441646abaed1234567890ab\tHEAD\n",
        );
        assert!(!sha_in_remote("175fb18284d0", output));
    }

    #[test]
    fn sha_in_remote_handles_empty_output() {
        assert!(!sha_in_remote("175fb18284d0", ""));
    }

    #[test]
    fn sha_in_remote_matches_main_branch_commit() {
        // When built from main at the same commit as origin/main.
        let local = "2441646abaed";
        let output = concat!(
            "2441646abaed1234567890ab\trefs/heads/main\n",
            "2441646abaed1234567890ab\tHEAD\n",
        );
        assert!(sha_in_remote(local, output));
    }

    #[test]
    fn sha_in_remote_rejects_short_remote_sha() {
        // A remote SHA shorter than SHA_LEN should not match anything.
        let output = "abc\tHEAD\n";
        assert!(!sha_in_remote("abc123def456", output));
    }
}
