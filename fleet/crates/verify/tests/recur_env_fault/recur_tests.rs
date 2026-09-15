use super::*;

#[test]
fn exit_3_is_skip_not_fail_for_a_synthetic_env_fault() {
    let spec = recur_spec();
    let gates = GatesRoot::materialize().expect("materialize embedded gate scripts");
    let runner = Canned(ProcessOutput {
        exit_code: 3,
        stdout: "recur-gate: repo /does/not/exist not accessible".to_string(),
        stderr: String::new(),
    });
    assert_env_fault_skip(verify::run_gate(spec, &AlwaysAvailable, &runner, &gates));
}

#[test]
fn recur_reports_skip_not_fail_when_target_repo_has_no_git() {
    let plain_dir = tempfile::tempdir().expect("tempdir");
    assert!(!plain_dir.path().join(".git").exists());
    let spec = recur_spec();
    let gates = GatesRoot::materialize().expect("materialize embedded gate scripts");
    let runner = RealShellRunner {
        repo: plain_dir.path(),
    };
    assert_env_fault_skip(verify::run_gate(spec, &AlwaysAvailable, &runner, &gates));
}
