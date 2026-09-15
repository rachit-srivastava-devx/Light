use super::{after, before};

#[test]
fn after_uses_the_final_marker() {
    assert_eq!(after("fake=0 real=7", "="), Some(7));
}

#[test]
fn before_uses_the_final_marker() {
    assert_eq!(before("0 passed; summary 4 passed;", " passed;"), Some(4));
}

#[test]
fn malformed_marker_has_no_denominator() {
    assert_eq!(after("total=not-a-number", "total="), None);
    assert_eq!(before("prefix failed;", " failed;"), None);
}
