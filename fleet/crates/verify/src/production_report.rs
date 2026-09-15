use crate::impl_;
use crate::GateEvidence;

pub fn report_from_evidence(evidence: &GateEvidence) -> impl_::Report {
    let results = evidence
        .gate_results
        .iter()
        .filter_map(|result| {
            let spec = impl_::GATES.iter().find(|spec| spec.id == result.id)?;
            let denominator = parse_denominator(&result.input_digest);
            let verdict = if result.passed {
                denominator
                    .map(impl_::Verdict::Pass)
                    .unwrap_or(impl_::Verdict::Fail {
                        reason: impl_::FailReason::Unparseable,
                        denominator: None,
                    })
            } else if result.exit_code == 3 {
                impl_::Verdict::Skip {
                    reason: result.failure_message.clone().unwrap_or_default(),
                    was_required: spec.requirement == impl_::Requirement::Required,
                }
            } else {
                impl_::Verdict::Fail {
                    reason: impl_::FailReason::NonZeroExit(result.exit_code),
                    denominator,
                }
            };
            Some(impl_::GateResult {
                id: spec.id,
                verdict,
            })
        })
        .collect();
    impl_::Report { results }
}

fn parse_denominator(value: &str) -> Option<impl_::Denominator> {
    let value = value.strip_prefix("denominator:")?;
    let (numerator, total) = value.split_once('/')?;
    impl_::Denominator::new(numerator.parse().ok()?, total.parse().ok()?).ok()
}
