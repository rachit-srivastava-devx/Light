use super::parse;
use crate::VerifyError;
use std::io::Write;

#[test]
fn malformed_report_is_rejected() {
    let mut file = tempfile::NamedTempFile::new().expect("temporary report");
    file.write_all(b"not-json").expect("write report");
    assert!(matches!(
        parse(file.path()),
        Err(VerifyError::ScannerParse(_))
    ));
}

#[test]
fn missing_severity_defaults_to_high() {
    let mut file = tempfile::NamedTempFile::new().expect("temporary report");
    file.write_all(br#"[{"RuleID":"rule","File":"fixture.txt"}]"#)
        .expect("write report");
    let findings = parse(file.path()).expect("report parses");
    assert_eq!(findings[0].severity, "HIGH");
}

#[test]
fn oversized_report_is_rejected_before_decode() {
    let mut file = tempfile::NamedTempFile::new().expect("temporary report");
    file.write_all(&vec![b'x'; 16 * 1024 * 1024 + 1])
        .expect("write report");
    assert!(matches!(
        parse(file.path()),
        Err(VerifyError::ScannerParse(_))
    ));
}
