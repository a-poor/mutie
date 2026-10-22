use mutie_rust_arithmetic::*;

#[test]
fn exact_integer_results() {
    assert_eq!(add(7, 2), 9);
    assert_eq!(subtract(7, 2), 5);
    assert_eq!(multiply(7, 2), 14);
    assert_eq!(divide(-7, 2), -3);
    assert_eq!(remainder(-7, 2), -1);
}
#[test]
fn weak_cases() {
    assert_eq!(weak_add(1, 0), 1);
    assert_eq!(weak_remainder(0, 2), 0);
}
#[test]
fn zero_multiplication_does_not_panic() { assert_eq!(zero_product(1, 0), 0); }
#[test]
fn subtraction_at_maximum_does_not_overflow() { assert_eq!(maximum_minus(i32::MAX, 1), i32::MAX - 1); }
#[test]
fn string_append() { assert_eq!(append(String::from("a"), "b"), "ab"); }
