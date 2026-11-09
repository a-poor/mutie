use mutie_rust_numbers::*;

#[test]
fn values_and_types() {
    assert_eq!(checked(), 7);
    let _: f32 = weak();
    assert_eq!(divide(), 10);
    assert_eq!(indexed(), 9);
}
