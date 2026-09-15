use crate::impl_;
use crate::types::GateResult;

pub(super) fn digest(value: &str) -> String {
    format!("blake3:{}", blake3::hash(value.as_bytes()).to_hex())
}

pub(super) fn convert_result(
    result: impl_::GateResult,
    output: Option<&impl_::ProcessOutput>,
) -> GateResult {
    let (exit_code, passed, input_digest, checked, total, failure_message) = match result.verdict {
        impl_::Verdict::Pass(denominator) => (
            0,
            true,
            format!(
                "denominator:{}/{}",
                denominator.numerator(),
                denominator.total()
            ),
            denominator.numerator(),
            denominator.total(),
            None,
        ),
        impl_::Verdict::Fail {
            reason,
            denominator,
        } => {
            let input = denominator
                .map(|d| format!("denominator:{}/{}", d.numerator(), d.total()))
                .unwrap_or_else(|| "denominator:unknown".into());
            (
                6,
                false,
                input,
                denominator.map_or(0, |d| d.numerator()),
                denominator.map_or(0, |d| d.total()),
                Some(format!("gate failed: {reason:?}")),
            )
        }
        impl_::Verdict::Skip { reason, .. } => {
            (3, false, "denominator:skipped".into(), 0, 0, Some(reason))
        }
    };
    GateResult {
        id: result.id.to_string(),
        exit_code,
        stdout_digest: output.map_or_else(|| digest(""), |out| digest(&out.stdout)),
        stderr_digest: output.map_or_else(|| digest(""), |out| digest(&out.stderr)),
        input_digest,
        checked,
        total,
        passed,
        failure_message,
    }
}
