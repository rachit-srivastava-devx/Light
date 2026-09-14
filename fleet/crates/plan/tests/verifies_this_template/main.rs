//! Proves `docs/templates/SOW_TEMPLATE.md` (goal item 7's planning template) is not just
//! documentation that *looks* right -- it validates cleanly against fleet's own real SOW gate,
//! the same `validate_sow_text` that `fleet sow` runs. If someone edits the template and breaks
//! one of its structural requirements, this test fails and names which one.

use plan::validate_sow_text;
use std::path::Path;

const TEMPLATE: &str = include_str!("../../../../docs/templates/SOW_TEMPLATE.md");

#[test]
fn the_planning_template_passes_its_own_gate_with_zero_violations() {
    let violations = validate_sow_text(TEMPLATE, "");
    assert!(
        violations.is_empty(),
        "the planning template must validate cleanly, but got: {violations:?}"
    );
}

/// The template file must actually be where the doc comment (and this include_str!) claims.
#[test]
fn the_template_file_exists_at_the_documented_path() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/templates/SOW_TEMPLATE.md");
    assert!(path.exists(), "expected {path:?} to exist");
}

/// A structural regression this template must keep catching: strip one required heading and the
/// gate must refuse, by name -- proves the test above is exercising a real gate, not a vacuous
/// pass on an empty rule set.
#[test]
fn a_template_missing_a_required_heading_is_refused() {
    let broken = TEMPLATE.replacen("## Done when", "## (removed)", 1);
    let violations = validate_sow_text(&broken, "");
    assert!(
        violations.iter().any(|v| v.0.contains("Done when")),
        "expected a violation naming the removed heading, got: {violations:?}"
    );
}
