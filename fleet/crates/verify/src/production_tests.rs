use super::*;
use std::fs;

#[test]
fn production_result_hashes_the_captured_streams() {
    let result = impl_::GateResult {
        id: "unit",
        verdict: impl_::Verdict::Pass(impl_::Denominator::new(1, 1).expect("denominator")),
    };
    let output = impl_::ProcessOutput {
        exit_code: 0,
        stdout: "actual stdout".into(),
        stderr: "actual stderr".into(),
    };
    let converted = output::convert_result(result, Some(&output));
    assert_eq!(converted.stdout_digest, output::digest("actual stdout"));
    assert_eq!(converted.stderr_digest, output::digest("actual stderr"));
}

#[test]
fn no_git_tree_digest_is_content_bound_and_path_independent() {
    let first = tempfile::tempdir().expect("first tempdir");
    let second = tempfile::tempdir().expect("second tempdir");
    for root in [first.path(), second.path()] {
        fs::create_dir(root.join("nested")).expect("nested directory");
        fs::write(root.join("nested/file.txt"), b"same contents").expect("file");
    }
    assert_eq!(
        tree_digest_for_repo(first.path()),
        tree_digest_for_repo(second.path())
    );
    fs::write(first.path().join("nested/file.txt"), b"changed contents").expect("mutation");
    assert_ne!(
        tree_digest_for_repo(first.path()),
        tree_digest_for_repo(second.path())
    );
}

#[path = "production_acceptance_tests.rs"]
mod acceptance;
