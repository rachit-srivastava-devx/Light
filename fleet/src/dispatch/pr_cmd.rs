//! `fleet pr`: open a GitHub PR whose body attaches the last PASSING run's receipt for
//! `--task` from the ledger. The tail of the chained `fleet swarm --then-verify` -> `fleet
//! run` -> `fleet pr` flow: instead of pasting the receipt into `gh pr create` by hand, the
//! composition root reads it back and hands it to `gh` as the body.
//!
//! Deliberately does NOT re-auth `gh`, does NOT invent a receipt when the ledger has none,
//! and does NOT depend on any GitHub Rust crate -- `gh` + `git remote get-url` + a tiny
//! parser is the whole surface.

use crate::cli::args_ops::PrArgs;
use crate::dispatch::error::DispatchError;
use crate::dispatch::verify_repo::ensure_repo;
use crate::print::human;
use fleet_store::ledger::LedgerPaths;
use fleet_store::Ledger;
use fleet_types::{Receipt, ReceiptEvent};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn pr(state_dir: &Path, args: PrArgs) -> Result<(), DispatchError> {
    let repo = ensure_repo(&args.repo)?;
    let owner_name = infer_owner_name(&repo)?;
    let base = match args.base.as_deref() {
        Some(b) => b.to_string(),
        None => default_branch(&owner_name)?,
    };
    let head = match args.head.as_deref() {
        Some(h) => h.to_string(),
        None => current_branch(&repo)?,
    };
    let receipt = last_passing_receipt(state_dir, &args.task, &repo)?;
    let body = compose_body(args.body.as_deref(), &args.task, &receipt);

    if args.dry_run {
        print_resolved(&owner_name, &args.title, &base, &head, &body);
        return Ok(());
    }
    ensure_gh_on_path()?;
    let status = Command::new("gh")
        .args([
            "pr",
            "create",
            "--repo",
            &owner_name,
            "--title",
            &args.title,
            "--base",
            &base,
            "--head",
            &head,
            "--body",
            &body,
        ])
        .status()
        .map_err(|e| DispatchError::EnvFault(format!("failed to spawn gh: {e}")))?;
    if !status.success() {
        return Err(DispatchError::EnvFault(format!(
            "gh pr create exited with {}",
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into())
        )));
    }
    human::ok("pr opened");
    Ok(())
}

/// Compose the Markdown PR body: optional extra body, an `---` divider, then a `### fleet
/// receipt` block naming task_id, ledger hash, timestamp, and one row per `GateVerdict`
/// receipt captured under the same task's most recent passing run. Human-readable Markdown
/// on purpose -- reviewers see this, not raw JSON.
pub(crate) fn compose_body(extra: Option<&str>, task: &str, r: &PassingRun) -> String {
    let mut out = String::new();
    if let Some(x) = extra {
        if !x.is_empty() {
            out.push_str(x.trim_end());
            out.push_str("\n\n");
        }
    }
    out.push_str("---\n### fleet receipt\n\n");
    out.push_str(&format!("Task: {task}\n"));
    out.push_str(&format!("Ledger: {}\n", r.run_end.hash.as_str()));
    out.push_str(&format!("Run: {}\n\n", r.run_end.ts_wall));
    out.push_str("| Gate | Result | Denominator |\n|---|---|---|\n");
    if r.gates.is_empty() {
        out.push_str("| (no gate verdicts) | -- | -- |\n");
    } else {
        for g in &r.gates {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                escape_pipe(&g.id),
                g.outcome,
                g.denominator()
            ));
        }
    }
    out
}

fn print_resolved(owner_name: &str, title: &str, base: &str, head: &str, body: &str) {
    human::line("dry_run", "true");
    human::line("repo", owner_name);
    human::line("base", base);
    human::line("head", head);
    human::line("title", title);
    // The body may span many lines -- print it verbatim after a marker so a reviewer can
    // eyeball the exact bytes `gh pr create` would receive.
    println!("--- body ---");
    println!("{body}");
    println!("--- end body ---");
}

/// The composed view of one passing run: the terminal `RunEnd` row (for hash + ts_wall) and
/// every `GateVerdict` row that sits between that `RunEnd` and the matching `RunStart`.
#[derive(Debug, Clone)]
pub(crate) struct PassingRun {
    pub run_end: Receipt,
    pub gates: Vec<GateRow>,
}

#[derive(Debug, Clone)]
pub(crate) struct GateRow {
    pub id: String,
    pub outcome: String,
    pub checked: Option<u64>,
    pub total: Option<u64>,
}

impl GateRow {
    fn denominator(&self) -> String {
        match (self.checked, self.total) {
            (Some(c), Some(t)) => format!("{c}/{t}"),
            _ => "n/a".to_string(),
        }
    }
}

fn last_passing_receipt(
    state_dir: &Path,
    task: &str,
    repo: &Path,
) -> Result<PassingRun, DispatchError> {
    let paths = LedgerPaths {
        chain: state_dir.join("ledger.chain"),
        lock: state_dir.join("ledger.lock"),
    };
    let ledger = Ledger::open(paths);
    let rows = ledger.rows(true).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    pick_last_passing(&rows, task).ok_or_else(|| {
        DispatchError::EnvFault(format!(
            "no passing receipt found for task_id {task} in repo {}; run `fleet run` first",
            repo.display()
        ))
    })
}

pub(crate) fn pick_last_passing(rows: &[Receipt], task: &str) -> Option<PassingRun> {
    // Walk in reverse; the newest RunEnd with ok=true & matching task_id wins. Then gather
    // every GateVerdict between that RunEnd and the matching preceding RunStart (or index 0).
    let mut end_idx: Option<usize> = None;
    for (i, r) in rows.iter().enumerate().rev() {
        if r.event == ReceiptEvent::RunEnd
            && body_str(&r.body, "task_id") == Some(task)
            && r.body.get("ok").and_then(|v| v.as_bool()) == Some(true)
        {
            end_idx = Some(i);
            break;
        }
    }
    let end = end_idx?;
    // Find the matching RunStart before `end`.
    let mut start = 0usize;
    for i in (0..end).rev() {
        let r = &rows[i];
        if r.event == ReceiptEvent::RunStart && body_str(&r.body, "task_id") == Some(task) {
            start = i;
            break;
        }
    }
    let gates: Vec<GateRow> = rows[start..end]
        .iter()
        .filter(|r| r.event == ReceiptEvent::GateVerdict)
        .map(|r| GateRow {
            id: body_str(&r.body, "id").unwrap_or("(unknown)").to_string(),
            outcome: body_str(&r.body, "outcome").unwrap_or("(unknown)").to_string(),
            checked: r.body.get("checked").and_then(Value::as_u64),
            total: r.body.get("total").and_then(Value::as_u64),
        })
        .collect();
    Some(PassingRun { run_end: rows[end].clone(), gates })
}

fn body_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|")
}

pub(crate) fn parse_owner_name(url: &str) -> Result<String, DispatchError> {
    // Accepts `https://github.com/foo/bar.git`, `https://github.com/foo/bar`,
    // `git@github.com:foo/bar.git`, `ssh://git@github.com/foo/bar.git`.
    let s = url.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    let tail = if let Some(rest) = s.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = s.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = s.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = s.strip_prefix("ssh://git@github.com/") {
        rest
    } else if let Some(rest) = s.strip_prefix("git://github.com/") {
        rest
    } else {
        return Err(DispatchError::EnvFault(format!(
            "could not parse GitHub owner/name from remote url: {url}"
        )));
    };
    let mut it = tail.splitn(3, '/');
    let owner = it.next().unwrap_or("");
    let name = it.next().unwrap_or("");
    if owner.is_empty() || name.is_empty() {
        return Err(DispatchError::EnvFault(format!(
            "could not parse GitHub owner/name from remote url: {url}"
        )));
    }
    Ok(format!("{owner}/{name}"))
}

fn infer_owner_name(repo: &Path) -> Result<String, DispatchError> {
    let out = Command::new("git")
        .args(["-C"])
        .arg(repo.as_os_str())
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|e| DispatchError::EnvFault(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        return Err(DispatchError::EnvFault(format!(
            "git remote get-url origin failed in {}: {}",
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    parse_owner_name(String::from_utf8_lossy(&out.stdout).trim())
}

fn current_branch(repo: &Path) -> Result<String, DispatchError> {
    let out = Command::new("git")
        .args(["-C"])
        .arg(repo.as_os_str())
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .map_err(|e| DispatchError::EnvFault(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        return Err(DispatchError::EnvFault(format!(
            "could not resolve current branch of {} (detached HEAD? pass --head)",
            repo.display()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn default_branch(owner_name: &str) -> Result<String, DispatchError> {
    ensure_gh_on_path()?;
    let out = Command::new("gh")
        .args(["repo", "view", owner_name, "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"])
        .output()
        .map_err(|e| DispatchError::EnvFault(format!("failed to spawn gh: {e}")))?;
    if !out.status.success() {
        return Err(DispatchError::EnvFault(format!(
            "gh repo view {owner_name} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return Err(DispatchError::EnvFault(format!(
            "gh repo view {owner_name} returned no defaultBranchRef; pass --base"
        )));
    }
    Ok(s)
}

fn ensure_gh_on_path() -> Result<(), DispatchError> {
    which("gh").map(|_| ()).ok_or(DispatchError::EnvFault(
        "gh CLI not found on PATH; install github.com/cli/cli or use `git push` + `gh` manually"
            .to_string(),
    ))
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
#[path = "pr_cmd_tests.rs"]
mod tests;
