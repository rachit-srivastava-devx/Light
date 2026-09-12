use verify::{assemble_gate_evidence, GateResult, ReviewedCandidate, SecretFinding};

fn candidate() -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: "t1".into(),
        acceptance_digest: "a1".into(),
        gates: vec![],
        coverage_floor: None,
        output_digest: None,
    }
}

fn gate_result(id: &str, exit_code: i32, stdout_digest: &str) -> GateResult {
    GateResult {
        id: id.into(),
        exit_code,
        stdout_digest: stdout_digest.into(),
        stderr_digest: "".into(),
        input_digest: "".into(),
        passed: exit_code == 0,
        failure_message: None,
    }
}

// Pins hash_str and compute_digest exact output.
// Mutations:
//   hash_str → 0         → "sha256:0"
//   hash_str → 1         → "sha256:1"
//   ^= → |=              → "sha256:a1cbd6c45a7e64b5"
//   ^= → &=              → "sha256:0"
//   compute_digest → ""  → empty string
//   compute_digest → "xyzzy" → "xyzzy"
#[test]
fn evidence_digest_is_pinned_for_known_gate_result() {
    let ev = assemble_gate_evidence(
        &candidate(),
        vec![gate_result("g1", 0, "abc")],
        vec![],
    );
    assert_eq!(ev.evidence_digest, "sha256:c693a99547a742d9");
}

// Ensures findings change the digest (compute_digest covers both results + findings paths).
#[test]
fn finding_changes_evidence_digest() {
    let without = assemble_gate_evidence(
        &candidate(),
        vec![gate_result("g1", 0, "abc")],
        vec![],
    );
    let with_finding = assemble_gate_evidence(
        &candidate(),
        vec![gate_result("g1", 0, "abc")],
        vec![SecretFinding { rule_id: "AWS_KEY".into(), severity: "critical".into(), file: "src/lib.rs".into(), redacted: true }],
    );
    assert_ne!(without.evidence_digest, with_finding.evidence_digest);
}
