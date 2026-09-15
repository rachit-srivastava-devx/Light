#[cfg(unix)]
mod unix {
    use super::super::bounded;
    use crate::VerifyError;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn scanner_timeout_kills_and_reaps_the_child() {
        let child = Command::new("sh")
            .args(["-c", "sleep 2"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("shell exists");
        let result = bounded(child, Duration::from_millis(10));
        assert!(matches!(result, Err(VerifyError::ScannerTimeout)));
    }

    #[test]
    fn scanner_success_path_collects_output_before_deadline() {
        let child = Command::new("sh")
            .args(["-c", "printf ready"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("shell exists");
        let output = bounded(child, Duration::from_secs(1)).expect("child succeeds");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"ready");
    }
}
