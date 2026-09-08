//! `ChangeEmitter`, `ProposalRequest`, `ProposedChange`. Ported from
//! `fleet/keel/fleet/src/lifecycle.rs:296-333`.

use fleet_types::GateRefusal;
use std::path::PathBuf;

/// Push `request.head` and open a pull request. MUST return the real PR URL on success --
/// returning `Ok` without one (e.g. `gh` exiting 0 with no URL parsed) is itself a bug in the
/// implementation, not something this trait can prevent structurally; the implementation is
/// the caller's responsibility (`src/`, today `main.rs`'s real worktree-push + `gh pr create`).
pub trait ChangeEmitter {
    fn emit(&self, request: &ProposalRequest) -> Result<ProposedChange, GateRefusal>;
}

/// Everything `propose` needs to ask a `ChangeEmitter` to open a pull request. `diff` carries
/// the actual attested bytes so `propose` verifies the digest itself rather than trusting the
/// caller already checked it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalRequest {
    pub repo: PathBuf,
    pub base: String,
    pub head: String,
    pub artifact_id: String,
    pub diff: Vec<u8>,
    pub title: String,
    pub body: String,
}

/// What a successful `propose` actually did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedChange {
    pub url: String,
    pub head: String,
    pub commit: String,
    pub changed_files: u64,
}
