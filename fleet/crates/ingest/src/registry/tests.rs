use super::{check_delivery_id, check_source};
use crate::{IngestError, SourceRegistration};

#[test]
fn accepts_inclusive_source_and_delivery_lengths() {
    let source = "s".repeat(256);
    let registration = SourceRegistration {
        namespace: source.clone(),
        schema_version: 1,
    };
    assert_eq!(check_source(&source, &registration).unwrap(), source);
    assert_eq!(check_delivery_id(&"d".repeat(256)).unwrap().len(), 256);
}

#[test]
fn rejects_each_independent_delivery_failure() {
    for value in [String::new(), "d".repeat(257), "bad\u{7f}".into()] {
        assert_eq!(
            check_delivery_id(&value).unwrap_err(),
            IngestError::InvalidDeliveryId
        );
    }
}
