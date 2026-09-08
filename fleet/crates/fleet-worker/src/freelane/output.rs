//! Parses freelane.sh's stdout/stderr contract into a typed result. Ported from keel's
//! `interpret_freelane_output`/`parse_freelane_model`/`parse_freelane_usage`
//! (`git show HEAD:fleet/keel/fleet/src/main.rs:3261`). freelane.sh's own contract (see
//! `crates/fleet-worker/assets/freelane.sh`): exit 0 with the model's raw reply on stdout and a
//! `[resolved_model=... requested=... lane=N/M tried=... usage={...}]` trailer line on stderr;
//! exit 3 when every configured lane is unavailable; exit 7 on a usage error (missing prompt).

#[derive(Debug, PartialEq, Eq)]
pub struct FreelaneOutput {
    pub response: String,
    pub log: String,
    pub resolved_model: Option<String>,
    pub tokens: Option<u64>,
    /// Worktree-relative paths `apply::apply` actually wrote, or empty when the reply could not
    /// be applied (no fence, an ambiguous/unsafe target, ...). Empty here is not itself an error
    /// -- `run` still returns the model's response either way; `apply_note` carries the reason.
    pub applied_files: Vec<std::path::PathBuf>,
    /// `None` when application succeeded (or was never attempted because the reply had no fence
    /// worth trying); `Some(reason)` when apply was attempted and refused, for the log.
    pub apply_note: Option<String>,
}

/// Reads `[resolved_model=X requested=...]` off freelane.sh's stderr trailer. `None` for an
/// absent trailer or the literal `UNRESOLVED` freelane.sh emits when a lane's own body carried no
/// `model` field.
pub(super) fn parse_resolved_model(log: &str) -> Option<String> {
    log.lines().find_map(|line| {
        let value = line.trim().strip_prefix("[resolved_model=")?.split_once(" requested=")?.0;
        (!value.is_empty() && value != "UNRESOLVED").then(|| value.to_string())
    })
}

/// Reads ` usage={...}]` off the same trailer line. `None` (never `0`) when usage was not
/// reported -- an unmeasured lane and a zero-cost lane are different facts (freelane.sh's own
/// comment on this, preserved).
pub(super) fn parse_tokens(log: &str) -> Option<u64> {
    log.lines().find_map(|line| {
        let raw = line.trim().split_once(" usage=")?.1.strip_suffix(']')?;
        let usage: serde_json::Value = serde_json::from_str(raw).ok()?;
        let prompt = usage.get("prompt_tokens")?.as_u64()?;
        let completion = usage.get("completion_tokens")?.as_u64()?;
        let total = usage.get("total_tokens")?.as_u64()?;
        (prompt.checked_add(completion)? == total).then_some(total)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_model_reads_back_reported_model_not_requested() {
        let log = "[resolved_model=served-model requested=requested-model lane=1/1 tried=x:answered]";
        assert_eq!(parse_resolved_model(log).as_deref(), Some("served-model"));
        assert_eq!(parse_resolved_model("[resolved_model=UNRESOLVED requested=r]"), None);
        assert_eq!(parse_resolved_model("no trailer here"), None);
    }

    #[test]
    fn tokens_read_back_consistent_usage_and_reject_inconsistent() {
        let ok = "[resolved_model=m requested=r usage={\"prompt_tokens\":10,\"completion_tokens\":25,\"total_tokens\":35}]";
        assert_eq!(parse_tokens(ok), Some(35));
        let bad = "[resolved_model=m requested=r usage={\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":99}]";
        assert_eq!(parse_tokens(bad), None);
        assert_eq!(parse_tokens("[resolved_model=m requested=r]"), None);
    }
}
