//! Happy-path coverage: a clean decision and an explicit abstention.

use fleet_judge::{judge, RawVerdict, Verdict};

use crate::support::{candidate, criteria, FakeModel};

#[test]
fn decides_on_a_clean_label() {
    let model = FakeModel(RawVerdict {
        label: Some("cli".into()),
        confidence_pct: Some(90),
        because: Some("bulk destructive op, scriptable".into()),
        abstain_why: None,
    });
    let v = judge(&criteria(), &candidate(), &model).unwrap();
    assert_eq!(
        v,
        Verdict::Decided {
            label: "cli".into(),
            confidence_pct: 90,
            because: "bulk destructive op, scriptable".into(),
        }
    );
    assert_eq!(v.label(), Some("cli"));
}

#[test]
fn abstains_when_model_says_so() {
    let model = FakeModel(RawVerdict {
        abstain_why: Some("prompt is ambiguous between cli and form".into()),
        ..Default::default()
    });
    let v = judge(&criteria(), &candidate(), &model).unwrap();
    assert_eq!(
        v,
        Verdict::Abstain {
            why: "prompt is ambiguous between cli and form".into()
        }
    );
    assert_eq!(v.label(), None);
}
