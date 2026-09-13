//! The two "lower is better" scanner gates, normalized to clean-of-total like every other
//! `parse_denominator` fn in `parsers.rs` (their raw output counts findings, not passes).

use super::super::denominator::DenominatorResult as D;
use super::super::digits::before;

/// `semgrep-gate.sh:100-108` -- `"<N> files scanned, <M> findings"`, normalized to clean/scanned.
pub fn semgrep(stdout: &str, _stderr: &str) -> D {
    match before(stdout, " files scanned").zip(before(stdout, " findings")) {
        Some((scanned, findings)) => D::Counted(scanned.saturating_sub(findings), scanned),
        None => D::Unparseable,
    }
}

/// `trivy-gate.sh:61-67` -- `"<total> secret findings across <N> reported targets"`.
pub fn trivy(stdout: &str, _stderr: &str) -> D {
    match before(stdout, " secret findings").zip(before(stdout, " reported targets")) {
        Some((total, targets)) => D::Counted(targets.saturating_sub(total), targets),
        None => D::Unparseable,
    }
}
