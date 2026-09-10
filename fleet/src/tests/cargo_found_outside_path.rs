//! `fleet doctor` reported `cargo: missing` on a machine with cargo 1.98.1 installed, because the
//! lookup was `which cargo` and rustup's `~/.cargo/bin` is only on `PATH` once a shell profile has
//! been sourced -- which a GUI process, a cron job, or a CI runner never does. The unit-tests gate
//! then degraded to a SKIP ("unavailable") instead of naming a fixable environment problem.
//! Drives the real binary with a `PATH` that deliberately excludes the rustup directory.

mod support;
use support::cmd;
use std::path::PathBuf;

/// The rustup install location this machine actually has, if any -- the test asserts the positive
/// property only when there is something to find (rule: never fake the input to a check).
fn rustup_cargo() -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))?;
    let bin = home.join("bin").join("cargo");
    bin.is_file().then_some(bin)
}

#[test]
fn doctor_finds_cargo_in_the_rustup_location_when_path_does_not_have_it() {
    let Some(expected) = rustup_cargo() else {
        eprintln!("skipped: no rustup cargo on this machine to find");
        return;
    };
    let out = cmd().env("PATH", "/usr/bin:/bin").arg("doctor").output().expect("binary runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.starts_with("cargo:")).unwrap_or_default();
    assert!(line.contains("found"), "cargo is installed but doctor said: {line:?}");
    assert!(line.contains(&expected.display().to_string()), "must name where: {line:?}");
}

#[test]
fn doctor_json_reports_the_same_resolution() {
    let Some(expected) = rustup_cargo() else {
        eprintln!("skipped: no rustup cargo on this machine to find");
        return;
    };
    let out =
        cmd().env("PATH", "/usr/bin:/bin").args(["doctor", "--json"]).output().expect("binary runs");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["cargo"], true, "{v}");
    assert_eq!(v["cargo_path"], expected.display().to_string(), "{v}");
}

/// A tool that genuinely is not installed must say where fleet looked, not just "unavailable" --
/// `conftest` (the `policy` gate's probe) is not in `/usr/bin` or `/bin` on any machine.
#[test]
fn a_genuinely_missing_tool_names_every_directory_searched() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    let out = cmd()
        // `/usr/sbin` only so the capacity preflight can still run `sysctl`; conftest is in
        // neither, and rustup's directory is deliberately absent from all of them.
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["gate", "--id", "policy", "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("SKIP gate policy"), "expected a visible skip, got: {err}");
    assert!(err.contains("no `conftest` in $PATH"), "must name the tool and $PATH: {err}");
    assert!(err.contains(".cargo/bin"), "must name the fallback directories too: {err}");
}
