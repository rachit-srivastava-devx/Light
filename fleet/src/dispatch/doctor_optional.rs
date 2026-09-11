//! `fleet doctor`'s "optional tools" section: enumerates every external scanner the gate
//! registry (`fleet_verify::GATES`) declares, prints `OK` with its resolved path or `MISS`
//! with a canonical install command. Informational only -- a missing optional scanner is
//! never a doctor failure. Kept in sync with the registry automatically: adding a new
//! `ProbeTool::Named("...")` gate shows up here without touching this file.

use fleet_verify::{GateSpec, ProbeTool, GATES};

/// One row in the section. `path` is `Some` when the tool resolves, else `None` and the
/// caller prints `install`. Flat so `--json` serializes it verbatim.
#[derive(serde::Serialize)]
pub struct Optional {
    pub tool: &'static str,
    pub path: Option<String>,
    pub install: String,
}

/// The name of an external tool a gate reaches for. `bash` is skipped (a POSIX shell is not
/// a scanner and telling a macOS user to `brew install bash` on top of `/bin/bash` is wrong).
/// `Cargo*` variants belong to doctor's top section (`cargo`, `cargo-fmt`, ...), not this list.
fn probe_name(spec: &GateSpec) -> Option<&'static str> {
    match spec.probe {
        ProbeTool::CargoMutants => Some("cargo-mutants"),
        ProbeTool::Named(n) if n != "bash" => Some(n),
        _ => None,
    }
}

/// Canonical install hint. macOS: `brew install <name>` for anything homebrew ships,
/// `cargo install <name>` for a cargo subcommand. Non-macOS: point at a real installer or
/// the tool's homepage rather than emit an `apt-get` line whose package name may not exist
/// on the caller's distro (rule 7: never print a command that would fail if pasted).
fn install_hint(name: &str) -> String {
    if name == "cargo-mutants" {
        return "cargo install cargo-mutants".into();
    }
    if cfg!(target_os = "macos") {
        return format!("brew install {name}");
    }
    match name {
        "semgrep" => "pip install semgrep  # or https://semgrep.dev/docs/getting-started/".into(),
        "trivy" => "https://aquasecurity.github.io/trivy/latest/getting-started/installation/".into(),
        "conftest" => "https://www.conftest.dev/install/".into(),
        "uv" => "https://docs.astral.sh/uv/getting-started/installation/".into(),
        other => format!("see the {other} project homepage for install instructions"),
    }
}

/// Every external scanner the registry names, de-duplicated, in registry order. Uses the
/// same `tool_path::find` as `probe()` so `$PATH` + rustup dirs are both searched -- a tool
/// findable at gate-run time is `OK` here, or the two would disagree.
pub fn collect() -> Vec<Optional> {
    let (mut seen, mut out): (Vec<&'static str>, Vec<Optional>) = (Vec::new(), Vec::new());
    for spec in GATES {
        let Some(name) = probe_name(spec) else { continue };
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let path = super::tool_path::find(name).map(|p| p.display().to_string());
        out.push(Optional { tool: name, path, install: install_hint(name) });
    }
    out
}

/// Prints the section. Column-aligned so a row of `OK` and `MISS` reads at a glance, and
/// the install command's leading `install:` lines up with the resolved-path column above.
pub fn print(items: &[Optional]) {
    if items.is_empty() {
        return;
    }
    println!("-- optional tools --");
    let width = items.iter().map(|i| i.tool.len()).max().unwrap_or(0);
    for i in items {
        match &i.path {
            Some(p) => println!("OK   {:<width$}  {}", i.tool, p, width = width),
            None => println!("MISS {:<width$}  install: {}", i.tool, i.install, width = width),
        }
    }
}
