use fleet_plan::{assemble_acceptance_checks_draft, assemble_blueprint_doc, assemble_summary_doc};

#[test]
fn assemble_blueprint_doc_has_five_sections() {
    let doc = assemble_blueprint_doc("SOW BODY\n", "atomic\trows\n", "chal\trows\n", "biz\trows\n", "tech\trows\n");
    assert!(doc.starts_with("# fleet build blueprint\n"));
    for heading in [
        "## Statement of Work",
        "## Atomic decomposition",
        "## Challenge register",
        "## Clarifications (business)",
        "## Clarifications (technical)",
        "## Deterministic gate",
    ] {
        assert!(doc.contains(heading), "missing {heading}");
    }
    assert!(doc.contains("SOW BODY"));
}

#[test]
fn assemble_acceptance_checks_draft_names_the_drafting_model() {
    let draft = assemble_acceptance_checks_draft("intake-drafter");
    assert!(draft.contains("drafting_model: intake-drafter"));
    assert!(draft.contains("check_1:"));
    assert!(draft.contains("check_4:"));
}

#[test]
fn assemble_summary_doc_extracts_the_four_checks() {
    let draft = assemble_acceptance_checks_draft("m");
    let refs = [("SOW", "state/sow.md"), ("Atomic", "state/atomic.tsv")];
    let summary = assemble_summary_doc("build the thing", &refs, &draft);
    assert!(summary.contains("Asked:\nbuild the thing"));
    assert!(summary.contains("- SOW: state/sow.md"));
    assert!(summary.contains("every SOW acceptance threshold is measurable"));
    assert!(!summary.contains("check_1:"));
}
