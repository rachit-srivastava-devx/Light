use crate::types::{OfflineError, OfflineScore, Recommendation};

/// Validate cohort integrity and set the `recommendation` field.
///
/// Returns `Coverage` when the cohort is empty or incomplete
/// (`total == 0`, `checked == 0`, or `checked != total`).
pub fn apply_quality_criteria(score: &mut OfflineScore) -> Result<(), OfflineError> {
    if score.total == 0 || score.checked == 0 || score.checked != score.total {
        return Err(OfflineError::Coverage);
    }
    score.recommendation = decide(score.baseline_pass, score.variant_pass);
    Ok(())
}

fn decide(baseline_pass: u64, variant_pass: u64) -> Recommendation {
    if variant_pass > 0 && variant_pass >= baseline_pass {
        Recommendation::Promote
    } else if variant_pass > 0 {
        Recommendation::Retain
    } else {
        Recommendation::Reject
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(bp: u64, vp: u64, n: u64) -> OfflineScore {
        OfflineScore {
            candidate_id: "c".into(),
            baseline_pass: bp,
            variant_pass: vp,
            checked: n,
            total: n,
            recommendation: Recommendation::Reject,
        }
    }

    #[test]
    fn zero_cohort_is_rejected() {
        assert!(apply_quality_criteria(&mut score(0, 0, 0)).is_err());
    }

    #[test]
    fn variant_better_promotes() {
        let mut s = score(2, 3, 3);
        apply_quality_criteria(&mut s).unwrap();
        assert_eq!(s.recommendation, Recommendation::Promote);
    }

    #[test]
    fn variant_worse_rejects() {
        let mut s = score(3, 0, 3);
        apply_quality_criteria(&mut s).unwrap();
        assert_eq!(s.recommendation, Recommendation::Reject);
    }

    /// Kills stats.rs:8:47 (|| → &&): with checked=3, total=5, checked != total
    /// but both > 0, original returns Coverage; mutation drops it.
    #[test]
    fn partial_checked_is_coverage_error() {
        let mut s = OfflineScore {
            candidate_id: "c".into(),
            baseline_pass: 2,
            variant_pass: 1,
            checked: 3,
            total: 5,
            recommendation: Recommendation::Reject,
        };
        assert!(apply_quality_criteria(&mut s).is_err());
    }

    /// Kills decide:16:25 (&&→||) and decide:16:21 (>→>=):
    /// variant=0, baseline=0 → Reject; mutation promotes.
    #[test]
    fn decide_both_zero_rejects() {
        assert_eq!(decide(0, 0), Recommendation::Reject);
    }

    /// Kills decide:18:28 (>→<) and (>→==):
    /// variant=1 > 0 but < baseline=3 → Retain; mutation rejects.
    #[test]
    fn decide_variant_positive_below_baseline_retains() {
        assert_eq!(decide(3, 1), Recommendation::Retain);
    }
}
