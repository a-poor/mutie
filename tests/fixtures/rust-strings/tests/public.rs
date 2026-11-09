use mutie_rust_strings::*;

#[test]
fn values_and_types() {
    assert_eq!(checked(), "ready");
    let _: &'static str = weak();
}
