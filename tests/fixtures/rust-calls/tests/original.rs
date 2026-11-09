use mutie_rust_calls::*;

#[test]
fn observed_effect() {
    assert_eq!(strong(), vec![7]);
}

#[test]
fn omitted_case() {
    assert!(weak(false).is_empty());
}

#[test]
fn type_inference() {
    assert_eq!(inferred(), 1);
}
