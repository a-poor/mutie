use std::process::Command;

fn check(mode: &str, left: &str, right: &str, expected: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutie-rust-boundaries"))
        .args([mode, left, right])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn checked_boundaries() {
    check("less", "4", "4", "false\n");
    check("at-most", "same", "same", "true\n");
    check("greater", "4", "4", "false\n");
}

#[test]
fn at_least_without_equality() {
    // Deliberately omit equality so the boundary mutation survives.
    check("at-least", "11", "10", "true\n");
    check("at-least", "9", "10", "false\n");
}
