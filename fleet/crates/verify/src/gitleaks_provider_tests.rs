#[cfg(unix)]
mod unix {
    use super::super::GitleaksFindingsProvider;
    use crate::FindingsProvider;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    fn fake_binary(version: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().expect("temporary executable");
        let body = format!(
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then echo {version}; exit 0; fi\nreport=\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--report-path\" ]; then shift; report=$1; fi\n  shift\ndone\nprintf '%s' '[{{\"RuleID\":\"stub-rule\",\"File\":\"fixture.txt\",\"Severity\":\"CRITICAL\",\"Secret\":\"raw-secret\"}}]' > \"$report\"\nexit 0\n"
        );
        std::fs::write(file.path(), body).expect("write fake scanner");
        let mut permissions = std::fs::metadata(file.path())
            .expect("fake scanner metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(file.path(), permissions).expect("make executable");
        file
    }

    #[test]
    fn provider_scans_through_binary_and_redacts_secret() {
        let repo = tempfile::tempdir().expect("temporary repository");
        let binary = fake_binary("8.30.1");
        let provider = GitleaksFindingsProvider::with_binary(repo.path(), binary.path())
            .with_budget(Duration::from_secs(30))
            .with_git_required(false);
        std::fs::write(repo.path().join("fixture.txt"), "fixture").expect("fixture writes");
        let findings = provider.scan("tree").expect("fake scan succeeds");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, "CRITICAL");
        assert!(findings[0].redacted);
        assert!(!format!("{findings:?}").contains("raw-secret"));
        assert_eq!(provider.last_scope().expect("scope published").total(), 1);
    }

    #[test]
    fn provider_rejects_unsupported_binary_version() {
        let repo = tempfile::tempdir().expect("temporary repository");
        let binary = fake_binary("7.0.0");
        let provider = GitleaksFindingsProvider::with_binary(repo.path(), binary.path())
            .with_git_required(false);
        std::fs::write(repo.path().join("fixture.txt"), "fixture").expect("fixture writes");
        let error = provider.scan("tree").expect_err("version must be rejected");
        assert!(error.to_string().contains("unsupported gitleaks version"));
    }

    #[test]
    fn empty_directory_scope_is_a_typed_refusal() {
        let repo = tempfile::tempdir().expect("temporary repository");
        let binary = fake_binary("8.30.1");
        let provider = GitleaksFindingsProvider::with_binary(repo.path(), binary.path())
            .with_git_required(false);
        let error = provider.scan("tree").expect_err("empty scope must refuse");
        assert!(matches!(
            error,
            crate::VerifyError::ScannerScopeUnavailable(_)
        ));
    }

    #[test]
    fn unborn_git_repo_falls_back_to_visible_directory_scope() {
        let binary = fake_binary("8.30.1");
        super::unborn::assert_fallback(binary.path());
    }
}

#[cfg(unix)]
#[path = "gitleaks_unborn_tests.rs"]
mod unborn;
