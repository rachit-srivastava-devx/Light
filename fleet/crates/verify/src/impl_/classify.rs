//! `classify` -- the private helper that enforces the crate's one invariant: a gate that
//! measured nothing is a `Fail`, never a `Pass`, no matter what its exit code was.

use super::denominator::{Denominator, DenominatorResult};
use super::ports::ProcessOutput;
use super::spec::GateSpec;
use super::verdict::{FailReason, Verdict};

pub(crate) fn classify(spec: &GateSpec, out: &ProcessOutput) -> Verdict {
    // Exit 3 is this estate's documented, project-wide contract for "environment fault"
    // (AGENTS.md #7: 0 ok / 3 environment fault / 6 invariant violation / 7 refusal / 8
    // verification mismatch -- "an environment fault must never be reported as an agent
    // failure"). Every fleet-authored gate script uses it exactly this way (recur-gate.sh,
    // detector-integrity.sh, policy/run.sh, corpus/run.sh and gates/corpus/*.sh): the gate could
    // not even attempt its check -- no git, no interpreter, no diff producible -- which is not
    // the same claim as running the check and finding a real invariant violation (exit 6).
    // Handled once here, gate-agnostically, so any gate following the convention gets the right
    // outcome without teaching its own parser a private not-applicable marker for this case.
    if out.exit_code == 3 {
        return Verdict::Skip {
            reason: format!(
                "{}: environment fault (exit 3); diagnostic withheld from the receipt",
                spec.id
            ),
            was_required: matches!(spec.requirement, super::requirement::Requirement::Required),
        };
    }

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
            was_required: matches!(spec.requirement, super::requirement::Requirement::Required),
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

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
