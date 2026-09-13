use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn unknown_path_not_executed() {
    let tmp = std::env::temp_dir();
    let sentinel = tmp.join("canary_was_executed_mc");
    let _ = fs::remove_file(&sentinel);

    let canary = tmp.join("canary_binary_mc");
    let script = format!("#!/bin/sh\ntouch {}\n", sentinel.display());
    fs::write(&canary, script).unwrap();
    fs::set_permissions(&canary, fs::Permissions::from_mode(0o755)).unwrap();

    let config = model_catalog::DiscoveryConfig {
        explicit_paths: vec![],
    };
    let candidates = model_catalog::discover(&config).unwrap();
    assert!(candidates.is_empty());
    assert!(
        !sentinel.exists(),
        "canary binary was executed but should not have been"
    );
}

#[test]
fn timeout_returns_unknown() {
    let tmp = std::env::temp_dir();
    let script = tmp.join("mc_sleep_forever.sh");
    fs::write(&script, "#!/bin/sh\nsleep 100\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let candidate = model_catalog::Candidate {
        path: script,
        version: "1.0".into(),
        digest: "d1".into(),
    };

    let report = model_catalog::probe(&candidate, std::time::Duration::from_millis(100));
    assert!(
        !report.capabilities.is_empty(),
        "capabilities map must not be empty"
    );
    for state in report.capabilities.values() {
        match state {
            model_catalog::CapabilityState::Unknown { reason } => {
                assert!(
                    reason.contains("timeout"),
                    "expected timeout reason, got: {reason}"
                );
            }
            other => panic!("expected Unknown, got {:?}", other),
        }
    }
}

#[test]
fn zero_trial_cannot_qualify() {
    let err = model_catalog::qualify(0, &[]).expect_err("should fail with zero trials");
    assert!(matches!(
        err,
        model_catalog::CatalogError::InsufficientTrials { got: 0 }
    ));
}
