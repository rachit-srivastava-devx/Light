//! Real prompts from ~/Developer/devx/posx-claude-history (the owner's own historical Claude
//! Code session transcripts, goal item 9's "golden dataset") replace synthetic strings for the
//! Jaccard-dedup threshold `merge.rs` actually runs at (>0.6). `jaccard.rs`'s own inline tests use
//! toy sentences ("a b c" / "b c d"); this suite checks the real threshold against real text,
//! where token counts and vocabulary overlap look nothing like a toy example.
//!
//! Fixtures: two real "KT handover" documents from two different `fleet-rs` sessions (same
//! opening boilerplate, diverged over time -- a genuine near-duplicate, not a copy) and one real
//! vague one-liner vs. one real detailed build brief (genuinely distinct topics). Expected scores
//! were computed independently in Python against the same fixture files before this test was
//! written, not tuned to make the assertions pass.

use scan::jaccard_similarity;

const KT_A: &str = include_str!("fixtures/kt_handover_a.txt");
const KT_B: &str = include_str!("fixtures/kt_handover_b.txt");

const VAGUE: &str = "how should I make a skill claude code always use while coding. on any repo";
const DETAILED: &str = "Author blueprints/fleet-memory/BLUEPRINT.md. First read \
    blueprints/_AGENT-BRIEF.md and follow it exactly. Your crate fleet-memory build bra";

/// Two independently-written handovers for the same repo, months apart, share enough real
/// vocabulary to land clearly above the merge step's 0.6 dedup threshold -- confirming the
/// threshold is meaningful on real prose, not just tuned to toy examples.
#[test]
fn two_real_near_duplicate_handovers_score_above_the_dedup_threshold() {
    let score = jaccard_similarity(KT_A, KT_B);
    assert!(
        score > 0.6,
        "expected real near-duplicates above the 0.6 dedup threshold, got {score}"
    );
    // Pinned to the independently-computed value (Python, same tokenization rule), not derived
    // from this test's own assertion -- a real regression in tokenize() should move this.
    assert!(
        (score - 0.851).abs() < 0.01,
        "expected ~0.851 (computed independently), got {score}"
    );
}

/// A genuinely vague real prompt and a genuinely detailed real prompt, on unrelated topics, share
/// almost no vocabulary -- confirming the threshold doesn't also fire on real unrelated text.
#[test]
fn two_real_unrelated_prompts_score_far_below_the_dedup_threshold() {
    let score = jaccard_similarity(VAGUE, DETAILED);
    assert!(
        score < 0.1,
        "expected real unrelated prompts far below threshold, got {score}"
    );
}

/// Truncating both handovers to their shared opening (same boilerplate paragraph, byte-identical
/// up to a real marker present in both) must score 1.0 -- the ceiling is reachable on real text,
/// not just close to it. Cuts at a known substring's byte offset (always a valid char boundary),
/// never a magic byte index into UTF-8 text containing multi-byte characters (the source em
/// dashes would make a fixed byte index fragile).
#[test]
fn identical_real_prefixes_score_exactly_one() {
    let marker = "## 2. Typed exit codes";
    let cut_a = KT_A.find(marker).expect("marker present in fixture A");
    let cut_b = KT_B.find(marker).expect("marker present in fixture B");
    let score = jaccard_similarity(&KT_A[..cut_a], &KT_B[..cut_b]);
    assert_eq!(score, 1.0, "identical real prefixes must score 1.0, got {score}");
}
