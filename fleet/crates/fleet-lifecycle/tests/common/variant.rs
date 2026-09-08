//! Names an `AnyTask`'s active variant, for asserting `resume`/`advance_any` land on the
//! expected state without needing a full `match` in every test file.

use fleet_lifecycle::AnyTask;

pub fn variant_name(any: &AnyTask) -> &'static str {
    match any {
        AnyTask::Intake(_) => "Intake",
        AnyTask::Specified(_) => "Specified",
        AnyTask::Reviewed(_) => "Reviewed",
        AnyTask::Decomposed(_) => "Decomposed",
        AnyTask::Contracted(_) => "Contracted",
        AnyTask::Briefed(_) => "Briefed",
        AnyTask::Leased(_) => "Leased",
        AnyTask::Building(_) => "Building",
        AnyTask::Built(_) => "Built",
        AnyTask::Verifying(_) => "Verifying",
        AnyTask::Verified(_) => "Verified",
        AnyTask::Attested(_) => "Attested",
        AnyTask::Accepted(_) => "Accepted",
        AnyTask::Proposed(_) => "Proposed",
        AnyTask::Observed(_) => "Observed",
        AnyTask::Refused(_) => "Refused",
    }
}
