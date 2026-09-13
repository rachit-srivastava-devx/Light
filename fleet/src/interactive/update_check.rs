//! Conservative remote provenance probe at REPL startup.
//!
//! Uses the repo path embedded at BUILD TIME (not CWD) so the check works for the installed
//! binary even when the shell's cwd is unrelated to this repo. Returns None silently on any
//! failure — network issues, missing git, moved repo — rather than printing an error or blocking.

/// Remote SHA prefix length to compare; matches the 12-char short sha in build_info.
const SHA_LEN: usize = 12;

/// Check remote provenance without making an unsupported update claim.
///
/// Runs `git ls-remote origin` from the embedded repo path, with a 2-second wall-clock
/// timeout. `ls-remote` has no ancestry information, so this probe never claims an update exists;
/// it returns `None` on every outcome until an ancestry-aware implementation replaces it.
pub async fn check(color: bool) -> Option<String> {
    let _ = color;
    let local_sha = crate::build_info::IDENTITY.commit_sha;
    if local_sha == "unknown" {
        return None; // built outside a checkout; nothing meaningful to compare
    }
    let repo = env!("FLEET_REPO_PATH");
    // Verify repo path still exists before spawning git (it may have moved post-install).
    if !std::path::Path::new(repo).join(".git").exists() {
        return None;
    }
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), run_ls_remote(repo))
        .await
        .ok()??;

    // `ls-remote` cannot establish that an unpublished local commit is outdated: it could
    // simply be the build the developer just installed. Never present that as an update.
    // A real remote-ahead check needs a fetched, ancestry-aware comparison, which this
    // non-blocking startup probe deliberately does not perform yet.
    let _published = sha_in_remote(local_sha, &result);
    None
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
#[path = "update_check_tests.rs"]
mod tests;
