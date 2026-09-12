//! HEAD/digest gate — validates exact integrated tree before approval.
use std::{fs, path::Path};
use crate::{GateRunner, PostError, PostRequest, PostVerdict, Status};
use crate::receipt::write_verdict;

/// Read the current HEAD commit SHA from `.git/HEAD` in `repo`.
/// Follows symbolic refs (`ref: refs/heads/<branch>`).
fn read_head(repo: &Path) -> Result<String, PostError> {
    let head_path = repo.join(".git").join("HEAD");
    let raw = fs::read_to_string(&head_path)
        .map_err(|e| PostError::Receipt(format!("HEAD unreadable: {e}")))?;
    let content = raw.trim();
    if let Some(ref_path) = content.strip_prefix("ref: ") {
        let ref_file = repo.join(".git").join(ref_path);
        let sha = fs::read_to_string(&ref_file)
            .map_err(|e| PostError::Receipt(format!("ref unreadable: {e}")))?;
        Ok(sha.trim().to_string())
    } else {
        Ok(content.to_string())
    }
}

/// Verify the integrated tree after merge.
///
/// Preconditions checked before any gate runs (no receipt on early exit):
/// - `required_gates` must be non-empty → `ZeroCoverage`
/// - actual HEAD must equal `req.expected_head` → `Stale`
///
/// Gate failures produce a `Failed` verdict with a receipt written to disk.
pub fn verify_after_merge(
    r: &impl GateRunner,
    req: PostRequest,
) -> Result<PostVerdict, PostError> {
    if req.required_gates.is_empty() {
        return Err(PostError::ZeroCoverage);
    }
    let actual_head = read_head(&req.repo)?;
    if actual_head != req.expected_head {
        return Err(PostError::Stale);
    }
    let mut status = Status::Pass;
    let mut checked = 0u64;
    let total = req.required_gates.len() as u64;
    for gate in &req.required_gates {
        match r.run(gate, &req.repo) {
            Ok(gr) => checked += gr.checked,
            Err(_) => status = Status::Failed,
        }
    }
    let verdict = PostVerdict {
        status,
        head: actual_head,
        checked,
        total,
        evidence_digest: format!(
            "{}:{}/{}",
            req.acceptance_digest, checked, total
        ),
    };
    let store = req.repo.join(".fleet").join("post-receipts");
    write_verdict(&verdict, &store)?;
    Ok(verdict)
}
