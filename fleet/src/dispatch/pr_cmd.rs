//! `fleet pr`: create a GitHub pull request from a lane worktree branch via `gh pr create`,
//! delegating to `integrate::pr_emit`. Split out of `ops_cmd.rs` to keep it under the 80-line
//! gate. Never merges to main directly -- a PR requires human review (AGENTS.md: "contracts/
//! migrations/money are human-merge always").

use crate::cli::args_ops::PrArgs;
use crate::dispatch::error::DispatchError;
use std::path::Path;

pub fn pr_emit(args: PrArgs) -> Result<(), DispatchError> {
    let repo = Path::new(&args.repo);
    let outcome = integrate::pr_emit(repo, repo, &args.branch, &args.module_brief, &args.diff_summary)?;
    crate::print::json::print_pretty(&serde_json::json!({
        "status": "success",
        "branch": args.branch,
        "pr_url": outcome.pr_url,
    }));
    Ok(())
}
