#[test]
fn checks_build_configuration_and_one_function() {
    assert!(build_fixture::checked());
    assert_eq!(build_fixture::BUILD_INPUT, "ready\n");
    assert!(cfg!(owned_build));
    assert_eq!(env!("OWNED_BUILD"), "ready");
}
