use super::*;

#[test]
fn canonical_verify_runs_once_and_preserves_live_gate_override() {
    let spec = override_spec();
    let specs = vec![spec];
    let commands = Arc::new(Mutex::new(Vec::new()));
    let scans = Arc::new(Mutex::new(0));
    let runner = RecordingRunner {
        stdout: "test result: ok. 1 passed; 0 failed; 0 ignored".into(),
        commands: Arc::clone(&commands),
    };
    let findings = RecordingFindings {
        scans: Arc::clone(&scans),
        findings: vec![],
    };
    let gates = GatesRoot::materialize().expect("embedded gates");
    let evidence = verify_production(
        &candidate(&specs),
        &EverythingAvailable,
        &runner,
        &gates,
        &specs,
        &findings,
    )
    .expect("production verification");
    assert!(evidence.passed);
    assert_eq!(*scans.lock().expect("scan count lock"), 1);
    assert_eq!(
        *commands.lock().expect("command log lock"),
        vec![
            OVERRIDE_ARGV
                .iter()
                .map(|part| (*part).to_string())
                .collect::<Vec<_>>()
        ]
    );
}
