//! Malformed-response coverage: every way a model reply can fail validation must become a
//! typed `JudgeError::MalformedResponse`, never a fabricated `Verdict`.

use fleet_judge::{judge, JudgeError, RawVerdict};

use crate::support::{candidate, criteria, FakeModel};

#[test]
fn rejects_label_outside_the_criteria() {
    let model = FakeModel(RawVerdict {
        label: Some("dom_click".into()),
        confidence_pct: Some(50),
        because: Some("x".into()),
        abstain_why: None,
    });
    let err = judge(&criteria(), &candidate(), &model).unwrap_err();
    assert!(matches!(err, JudgeError::MalformedResponse(_)));
}

#[test]
fn rejects_decision_missing_because() {
    let model = FakeModel(RawVerdict {
        label: Some("cli".into()),
        confidence_pct: Some(50),
        because: None,
        abstain_why: None,
    });
    assert!(matches!(
        judge(&criteria(), &candidate(), &model).unwrap_err(),
        JudgeError::MalformedResponse(_)
    ));
}

#[test]
fn rejects_neither_label_nor_abstain() {
    let model = FakeModel(RawVerdict::default());
    assert!(matches!(
        judge(&criteria(), &candidate(), &model).unwrap_err(),
        JudgeError::MalformedResponse(_)
    ));
}

#[test]
fn rejects_confidence_out_of_range() {
    let model = FakeModel(RawVerdict {
        label: Some("cli".into()),
        confidence_pct: Some(255),
        because: Some("x".into()),
        abstain_why: None,
    });
    assert!(matches!(
        judge(&criteria(), &candidate(), &model).unwrap_err(),
        JudgeError::MalformedResponse(_)
    ));
}
