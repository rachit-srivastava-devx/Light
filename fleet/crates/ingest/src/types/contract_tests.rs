#[test]
fn attachment_byte_bound_is_the_contract_value() {
    assert_eq!(super::MAX_ATTACHMENT_BYTES, 16_777_216);
}
