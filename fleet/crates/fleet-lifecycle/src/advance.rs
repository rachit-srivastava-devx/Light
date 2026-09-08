//! `advance_any`: mirrors `lifecycle.rs`'s `canonical_next`/`typed_advance`, pure.

use crate::human_approval::HumanApproval;
use crate::receipt::ReceiptLedger;
use crate::resume::AnyTask;
use fleet_types::GateRefusal;

/// Advance `any` exactly one step along its single canonical forward edge, using
/// `HumanApproval::recorded` internally for the two human-gated edges (`review`, `accept`) --
/// this makes `advance_any` a "just keep going" driver for automated re-drive, NOT a way to
/// bypass the human-approval requirement (the evidence string IS the approval evidence).
/// `Accepted` has no canonical next: `propose` needs a `bundle`/`request`/`emitter` this fn
/// does not carry, so it returns `Err(GateRefusal{code: "PR_EMIT_REQUIRES_EVIDENCE", ..})`.
/// `Refused` has no forward edge at all: `Err(GateRefusal{code: "ILLEGAL_LIFECYCLE_TRANSITION", ..})`.
pub fn advance_any(
    any: AnyTask,
    evidence: impl Into<String>,
    ledger: &impl ReceiptLedger,
) -> Result<AnyTask, GateRefusal> {
    let evidence = evidence.into();
    Ok(match any {
        AnyTask::Intake(task) => AnyTask::Specified(task.specify(evidence, ledger)?),
        AnyTask::Specified(task) => {
            AnyTask::Reviewed(task.review(HumanApproval::recorded(evidence)?, ledger)?)
        }
        AnyTask::Reviewed(task) => AnyTask::Decomposed(task.decompose(evidence, ledger)?),
        AnyTask::Decomposed(task) => AnyTask::Contracted(task.contract(evidence, ledger)?),
        AnyTask::Contracted(task) => AnyTask::Briefed(task.brief(evidence, ledger)?),
        AnyTask::Briefed(task) => AnyTask::Leased(task.lease(evidence, ledger)?),
        AnyTask::Leased(task) => AnyTask::Building(task.build(evidence, ledger)?),
        AnyTask::Building(task) => AnyTask::Built(task.finish_build(evidence, ledger)?),
        AnyTask::Built(task) => AnyTask::Verifying(task.begin_verification(evidence, ledger)?),
        AnyTask::Verifying(task) => AnyTask::Verified(task.verify(evidence, ledger)?),
        AnyTask::Verified(task) => AnyTask::Attested(task.attest(evidence, ledger)?),
        AnyTask::Attested(task) => {
            AnyTask::Accepted(task.accept(HumanApproval::recorded(evidence)?, ledger)?)
        }
        AnyTask::Accepted(_) => {
            return Err(GateRefusal::new(
                "PR_EMIT_REQUIRES_EVIDENCE",
                "Accepted -> Proposed emits a real pull request and needs an artifact and a \
                 repo; drive it with the caller's real pr-emit entry point",
            ))
        }
        AnyTask::Proposed(task) => AnyTask::Observed(task.observe(evidence, ledger)?),
        AnyTask::Observed(task) => AnyTask::Intake(task.reopen(evidence, ledger)?),
        AnyTask::Refused(_) => {
            return Err(GateRefusal::new(
                "ILLEGAL_LIFECYCLE_TRANSITION",
                "Refused has no canonical forward edge",
            ))
        }
    })
}
