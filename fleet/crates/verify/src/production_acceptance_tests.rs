use super::*;
use std::fs;

#[test]
fn resolved_acceptance_digest_binds_override_contents_and_filtering() {
    let first = tempfile::tempdir().expect("first tempdir");
    let second = tempfile::tempdir().expect("second tempdir");
    fs::write(first.path().join("gate.sh"), b"#!/bin/sh\nprintf first\n").expect("first gate");
    fs::write(second.path().join("gate.sh"), b"#!/bin/sh\nprintf second\n").expect("second gate");
    let first_root = impl_::GatesRoot::from_override(first.path()).expect("first root");
    let second_root = impl_::GatesRoot::from_override(second.path()).expect("second root");
    let mut spec = impl_::GATES[0];
    spec.command = impl_::GateCommand::Script {
        relative: "gate.sh",
        args: &[],
    };
    let selected = [spec];
    let first_candidate =
        candidate::candidate_for_repo_with_specs(first.path(), &first_root, &selected)
            .expect("first candidate");
    let second_candidate =
        candidate::candidate_for_repo_with_specs(second.path(), &second_root, &selected)
            .expect("second candidate");
    let filtered_candidate =
        candidate::candidate_for_repo_with_specs(first.path(), &first_root, &[])
            .expect("filtered candidate");
    assert_ne!(
        first_candidate.acceptance_digest,
        second_candidate.acceptance_digest
    );
    assert_ne!(
        first_candidate.acceptance_digest,
        filtered_candidate.acceptance_digest
    );
}

#[test]
fn resolved_candidate_digest_changes_for_override_content_and_filtering() {
    let first = tempfile::tempdir().expect("first tempdir");
    let second = tempfile::tempdir().expect("second tempdir");
    fs::write(first.path().join("gate.sh"), b"#!/bin/sh\nprintf first\n").expect("first gate");
    fs::write(second.path().join("gate.sh"), b"#!/bin/sh\nprintf second\n").expect("second gate");
    let first_root = impl_::GatesRoot::from_override(first.path()).expect("first root");
    let second_root = impl_::GatesRoot::from_override(second.path()).expect("second root");
    let mut spec = impl_::GATES[0];
    spec.command = impl_::GateCommand::Script {
        relative: "gate.sh",
        args: &[],
    };
    let selected = [spec];
    let first_candidate = candidate_from_resolved_specs("tree".into(), &first_root, &selected)
        .expect("first candidate");
    let second_candidate = candidate_from_resolved_specs("tree".into(), &second_root, &selected)
        .expect("second candidate");
    assert_ne!(
        first_candidate.acceptance_digest, second_candidate.acceptance_digest,
        "resolved asset content must be candidate identity"
    );
    let filtered_candidate =
        candidate_from_resolved_specs("tree".into(), &first_root, &[]).expect("filtered candidate");
    assert_ne!(
        first_candidate.acceptance_digest, filtered_candidate.acceptance_digest,
        "resolved gate filtering must be candidate identity"
    );
}
