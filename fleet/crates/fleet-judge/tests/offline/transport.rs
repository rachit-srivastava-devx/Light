//! Errors that never reach a model call, and errors the model port itself raises.

use fleet_judge::{judge, Criteria, JudgeError};

use crate::support::{candidate, criteria, FailingModel};

#[test]
fn empty_label_set_is_rejected_before_any_model_call() {
    let empty = Criteria {
        instructions: "n/a".into(),
        labels: vec![],
    };
    let err = judge(&empty, &candidate(), &FailingModel).unwrap_err();
    assert!(matches!(err, JudgeError::EmptyLabelSet));
}

#[test]
fn model_transport_failure_surfaces_as_typed_error() {
    let err = judge(&criteria(), &candidate(), &FailingModel).unwrap_err();
    assert!(matches!(err, JudgeError::Model(_)));
}
