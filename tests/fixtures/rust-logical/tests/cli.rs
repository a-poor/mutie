use std::process::Command;
fn check(mode: &str, a: bool, b: bool, c: bool, expected: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutie-rust-logical"))
        .args([mode, &a.to_string(), &b.to_string(), &c.to_string()])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
}
#[test]
fn short_circuit_order_and_effects() {
    check("all", false, true, false, "false:L");
    check("all", true, false, false, "false:LR");
    check("any", true, false, false, "true:L");
    check("any", false, true, false, "true:LR");
}
#[test]
fn three_operand_truth_tables_and_chain() {
    for a in [false, true] {
        for b in [false, true] {
            for c in [false, true] {
                check("all-three", a, b, c, &format!("{}:", a && b && c));
                check("any-three", a, b, c, &format!("{}:", a || b || c));
                check("chain", a, b, c, &format!("{}:", a && b && c));
            }
        }
    }
}
#[test]
fn weak_any_omits_mixed_inputs() {
    check("weak", false, false, false, "false:");
    check("weak", true, true, false, "true:");
}
