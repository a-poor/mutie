#[test]
fn cli_uses_library() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mutie-rust-library-fixture"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"true\n");
}
