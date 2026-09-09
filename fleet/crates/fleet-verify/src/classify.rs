//! `classify` -- the private helper that enforces the crate's one invariant: a gate that
//! measured nothing is a `Fail`, never a `Pass`, no matter what its exit code was.

use crate::denominator::{Denominator, DenominatorResult};
use crate::ports::ProcessOutput;
use crate::spec::GateSpec;
use crate::verdict::{FailReason, Verdict};

pub(crate) fn classify(spec: &GateSpec, out: &ProcessOutput) -> Verdict {
    if out.exit_code != 0 {
        let denominator = match (spec.parse_denominator)(&out.stdout, &out.stderr) {
            DenominatorResult::Counted(n, total) => Denominator::new(n, total).ok(),
            DenominatorResult::Unparseable | DenominatorResult::NotApplicable => None,
        };
        return Verdict::Fail {
            reason: FailReason::NonZeroExit(out.exit_code),
            denominator,
        };
    }

    match (spec.parse_denominator)(&out.stdout, &out.stderr) {
        // Visible, reasoned, and never a Pass -- `report.rs` already counts skips separately and
        // flags a REQUIRED gate that skipped, so this cannot quietly vanish from a summary.
        DenominatorResult::NotApplicable => Verdict::Skip {
            reason: format!("{}: no applicable input in this repo", spec.id),
            was_required: matches!(spec.requirement, crate::requirement::Requirement::Required),
        },
        DenominatorResult::Unparseable => Verdict::Fail {
            reason: FailReason::Unparseable,
            denominator: None,
        },
        DenominatorResult::Counted(n, total) => match Denominator::new(n, total) {
            Ok(d) => Verdict::Pass(d),
            Err(_) => Verdict::Fail {
                reason: FailReason::MeasuredNothing,
                denominator: None,
            },
        },
    }
}
