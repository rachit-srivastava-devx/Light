use super::*;

#[test]
fn production_evidence_output_digest_changes_when_real_output_changes() {
    let spec = override_spec();
    let specs = vec![spec];
    let gates = GatesRoot::materialize().expect("embedded gates");
    let candidate = candidate(&specs);
    let run = |stdout: &str| {
        verify_production(
            &candidate,
            &EverythingAvailable,
            &RecordingRunner {
                stdout: stdout.into(),
                commands: Arc::new(Mutex::new(Vec::new())),
            },
            &gates,
            &specs,
            &RecordingFindings {
                scans: Arc::new(Mutex::new(0)),
                findings: vec![],
            },
        )
        .expect("production verification")
    };
    let first = run("test result: ok. 1 passed; 0 failed; 0 ignored");
    let second = run("test result: ok. 2 passed; 0 failed; 0 ignored");
    assert_ne!(
        first.gate_results[0].stdout_digest,
        second.gate_results[0].stdout_digest
    );
}
