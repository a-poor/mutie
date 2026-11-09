use mutie_rust_assignments::*;

#[test]
fn checked_field() {
    assert_eq!(strong().value, 7);
}

#[test]
fn unchecked_item() {
    assert_eq!(weak([0]).len(), 1);
}

#[test]
fn compile_controls() {
    assert_eq!(inferred(), 1);
    assert_eq!(restored_move(), "");
}
