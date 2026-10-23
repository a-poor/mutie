use mutie_rust_loops::*;
#[test]
fn nonempty() {
    assert_eq!(count(2), 2);
}
#[test]
fn empty() {
    assert_eq!(weak(0), 0);
}
#[test]
fn initialization() {
    assert_eq!(initialized(true), 7);
}
