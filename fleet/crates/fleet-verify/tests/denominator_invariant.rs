use fleet_verify::Denominator;

#[test]
fn denominator_rejects_zero_total_regardless_of_numerator() {
    assert!(Denominator::new(0, 0).is_err());
    assert!(Denominator::new(7, 0).is_err());
}

#[test]
fn denominator_accepts_zero_numerator_with_nonzero_total() {
    let d = Denominator::new(0, 7).expect("0/7 is a legitimate failure denominator");
    assert_eq!(d.numerator(), 0);
    assert_eq!(d.total(), 7);
}

#[test]
fn denominator_roundtrips_numerator_and_total() {
    let d = Denominator::new(3, 5).unwrap();
    assert_eq!(d.numerator(), 3);
    assert_eq!(d.total(), 5);
}
