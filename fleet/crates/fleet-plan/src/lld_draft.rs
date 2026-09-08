//! Pure text assembly of the four accepted intake artifacts into the build blueprint document and
//! its acceptance-checks draft, matching `blueprint()`/`summary()` (`intake.sh:323-335`)
//! verbatim -- no readiness gating here (the caller calls `intake_gate` first).

/// Byte-faithful port of `blueprint()`'s five-section layout (`intake.sh:326`).
pub fn assemble_blueprint_doc(
    sow_text: &str,
    atomic_tsv: &str,
    challenges_tsv: &str,
    clarifications_business_tsv: &str,
    clarifications_technical_tsv: &str,
) -> String {
    format!(
        "# fleet build blueprint\n\n## Statement of Work\n\n{sow_text}\n## Atomic decomposition\n{atomic_tsv}\n## Challenge register\n{challenges_tsv}\n## Clarifications (business)\n{clarifications_business_tsv}\n## Clarifications (technical)\n{clarifications_technical_tsv}\n## Deterministic gate\nThe blueprint is buildable only while intake.sh gate exits 0. Any artifact drift or new blocking clarification returns exit 7.\n"
    )
}

/// The fixed 4-check acceptance-checks draft template (`intake.sh:327`), parameterised only by
/// the drafting model's name.
pub fn assemble_acceptance_checks_draft(drafting_model: &str) -> String {
    format!(
        "# fleet acceptance checks \u{2014} DRAFT\ndrafting_model: {drafting_model}\nblueprint_state: DRAFT\n\
check_1: every SOW acceptance threshold is measurable and linked to the recorded request hash.\n\
check_2: every feature leaf has explicit inputs, outputs, acceptance, and design_decision=none.\n\
check_3: every challenge cites docs/design/FAILURE-CORPUS.md or learn/THREAD-LESSONS.md and names a trigger and mitigation.\n\
check_4: every clarification is business or technical, linked to a gap, and every blocking row is answered before gate exit 0.\n"
    )
}

/// The human sign-off summary (`summary()`, `intake.sh:330-335`) -- `artifact_refs` stands in for
/// the five `$STATE/...` file paths the bash version interpolates.
pub fn assemble_summary_doc(
    intent: &str,
    artifact_refs: &[(&str, &str)],
    acceptance_checks_draft: &str,
) -> String {
    let mut out = format!("# Sign-off summary\n\nAsked:\n{intent}\n\nArtifacts:\n");
    for (label, path) in artifact_refs {
        out.push_str(&format!("- {label}: {path}\n"));
    }
    out.push_str("\nCode-start gate: READY (deterministic exit 0)\n\nExact four checks awaiting signature:\n");
    for line in acceptance_checks_draft.lines() {
        if let Some(rest) = line.split_once(": ") {
            if rest.0.starts_with("check_") {
                out.push_str(rest.1);
                out.push('\n');
            }
        }
    }
    out
}
