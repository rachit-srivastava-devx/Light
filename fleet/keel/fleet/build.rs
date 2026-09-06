use std::env;
use std::io::Write;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let repo = std::path::Path::new(&manifest_dir).join("../..");
    let git_sha = Command::new("git")
        .args(["-C", repo.to_string_lossy().as_ref(), "rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let build_date = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|date| date.trim().to_string())
        .filter(|date| !date.is_empty())
        .or_else(|| {
            env::var("SOURCE_DATE_EPOCH")
                .ok()
                .map(|epoch| format!("unix:{epoch}"))
        })
        .or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|duration| format!("unix:{}", duration.as_secs()))
        })
        .unwrap_or_else(|| "unknown".to_string());
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "cargo:rustc-env=FLEET_GIT_SHA={git_sha}")
        .expect("cargo build output is writable");
    writeln!(stdout, "cargo:rustc-env=FLEET_BUILD_DATE={build_date}")
        .expect("cargo build output is writable");
    writeln!(stdout, "cargo:rerun-if-changed=build.rs").expect("cargo build output is writable");
}
