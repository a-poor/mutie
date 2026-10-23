use mutie_rust_inversions::*;
#[test]
fn comparisons() {
    assert!(eq(2, 2));
    assert!(ne(1, 2));
    assert!(lt(1.0, 2.0));
    assert!(le(2, 2));
    assert!(gt(3, 2));
    assert!(ge(2, 2));
}
#[test]
fn boolean_and_integer_not() {
    assert!(!bool_not(true));
    assert_eq!(bit_not(0), 255);
}
#[test]
fn weak_cases() {
    assert!(!weak_lt(f64::NAN, 0.0));
    let _ = weak_not(true);
}
#[test]
fn custom_output_type() {
    assert!(!type_change(Flag));
}
