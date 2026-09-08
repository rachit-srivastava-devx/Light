//! `Task<Accepted>::propose`. Ported from `fleet/keel/fleet/src/lifecycle.rs:464-513`.

use crate::attestation::AttestationBundle;
use crate::proposal_types::{ChangeEmitter, ProposalRequest, ProposedChange};
use crate::receipt::ReceiptLedger;
use crate::states::{Accepted, Proposed};
use crate::task::Task;
use fleet_types::GateRefusal;

impl Task<Accepted> {
    /// `Accepted -> Proposed`: emit a real pull request carrying the attested diff. The
    /// agent's terminal act -- there is no `merge` edge. Gate order is deliberate and must
    /// stay side-effect-free until the last line: (1) attestation completeness, (2)
    /// `blake3::hash(&request.diff)` must equal `request.artifact_id`
    /// (`ARTIFACT_DIGEST_MISMATCH`), (3) `request.diff` must be non-empty (`EMPTY_DIFF`) --
    /// ALL three before `emitter.emit` runs; a refused proposal must push nothing and must
    /// never call the emitter.
    pub fn propose(
        self,
        bundle: &AttestationBundle,
        request: &ProposalRequest,
        emitter: &impl ChangeEmitter,
        ledger: &impl ReceiptLedger,
    ) -> Result<(Task<Proposed>, ProposedChange), GateRefusal> {
        if let Some(missing) = bundle.missing_element() {
            return Err(GateRefusal::new(
                "INCOMPLETE_ATTESTATION",
                format!("attestation is missing required element: {missing}"),
            ));
        }
        let digest = blake3::hash(&request.diff).to_hex().to_string();
        if digest != request.artifact_id {
            return Err(GateRefusal::new(
                "ARTIFACT_DIGEST_MISMATCH",
                format!(
                    "artifact {} does not match the frozen diff bytes (recomputed {digest})",
                    request.artifact_id
                ),
            ));
        }
        if request.diff.is_empty() {
            return Err(GateRefusal::new(
                "EMPTY_DIFF",
                "a run that changes nothing must not produce an attested artifact",
            ));
        }
        let change = emitter.emit(request)?;
        let proposed: Task<Proposed> = self.transition(format!("opened {}", change.url), ledger)?;
        Ok((proposed, change))
    }
}
