//! Build script for `fleet-cli`: bakes build identity (S2 in `docs/USER-JOURNEY-2.md`) into
//! compile-time env vars so the running binary can always say which build it is, even installed
//! far from any git checkout. Never shells out to `git` at RUNTIME (an installed binary may run
//! far from any checkout, and a runtime call would report the CWD's repo, not the build's) --
//! only this build script, which runs once at compile time in the source tree, may call `git`.

use std::process::Command;

fn main() {
    println!("cargo:rustc-env=FLEET_BUILD_SHA={}", git_sha());
    println!("cargo:rustc-env=FLEET_BUILD_DIRTY={}", git_dirty());
    println!("cargo:rustc-env=FLEET_BUILD_TIME={}", build_time());
    // Re-run only when HEAD or the index changes, not on every `cargo build`.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
}

/// Short commit sha, or the literal `unknown` if `git` is unavailable or this isn't a checkout.
/// Never fabricated -- an unreadable/missing git is reported honestly, not guessed.
fn git_sha() -> String {
    match run_git(&["rev-parse", "--short=12", "HEAD"]) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "unknown".to_string(),
    }
}

/// `"dirty"`/`"clean"` only when `git status --porcelain` unambiguously answered (empty stdout
/// on success means clean -- NOT a failure); `"unknown"` when `git` itself failed (not a
/// checkout, `git` missing) -- never smuggled into a false "clean".
fn git_dirty() -> String {
    match run_git(&["status", "--porcelain"]) {
        Some(s) if s.trim().is_empty() => "clean".to_string(),
        Some(_) => "dirty".to_string(),
        None => "unknown".to_string(),
    }
}

/// Runs `git` in the manifest dir; `None` only when the process failed to start or exited
/// non-zero or produced non-UTF8 output. Empty-but-successful stdout is `Some(String::new())`,
/// distinct from failure -- callers decide what an empty answer means for their own command.
fn run_git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// RFC3339 UTC build timestamp, computed with no external time crate: seconds since the epoch
/// (`SystemTime`, itself a build-time read here, not a runtime clock read in pure logic) turned
/// into a civil date by a standard, dependency-free algorithm (Howard Hinnant's `days_from_civil`
/// inverse), since `build.rs` cannot rely on the final binary's own dependency graph.
fn build_time() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Days-since-epoch (1970-01-01) -> (year, month, day). Public-domain algorithm, no leap-second
/// handling needed since git/filesystem timestamps don't carry them either.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
