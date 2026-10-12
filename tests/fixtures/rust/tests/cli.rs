use std::process::{Command, Output};

fn run(argument: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie-rust-fixture"))
        .arg(argument)
        .output()
        .expect("fixture binary should start")
}

#[test]
fn checked_flag_is_disabled() {
    let output = run("checked");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"false\n");
}

#[test]
fn unchecked_flag_smoke_test() {
    // Deliberately leave the value unchecked, for a later stronger-test rerun.
    let output = run("unchecked");
    assert!(output.status.success(), "{output:?}");
}
