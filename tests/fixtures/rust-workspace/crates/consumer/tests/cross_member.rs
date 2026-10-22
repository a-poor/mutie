#[test]
fn dependency_behavior() {
    assert!(workspace_consumer::enabled());
    assert!(workspace_core::fallback(true));
}
