use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // A cached candidate must never inherit the preceding candidate's outputs.
    assert!(!out.join("previous-build").exists(), "stale generated artifact");
    fs::write(out.join("previous-build"), "owned").unwrap();
    let input = fs::read_to_string("build-input.txt").unwrap();
    let value = if PathBuf::from("tests/build-mode").exists() {
        "test-dependent".into()
    } else {
        env::var("MUTIE_BUILD_FIXTURE_VALUE").unwrap_or_else(|_| "stable".into())
    };
    fs::write(out.join("generated.rs"), format!("pub const BUILD_INPUT: &str = {input:?};\npub const BUILD_VALUE: &str = {value:?};\n")).unwrap();
    println!("cargo::rerun-if-changed=build-input.txt");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=MUTIE_BUILD_FIXTURE_VALUE");
    println!("cargo::rustc-check-cfg=cfg(owned_build)");
    println!("cargo::rustc-cfg=owned_build");
    println!("cargo::rustc-env=OWNED_BUILD=ready");
}
