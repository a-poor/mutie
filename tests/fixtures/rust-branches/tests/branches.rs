use mutie_rust_branches::*;
#[test] fn both_branches() { assert_eq!(choose(true),1); assert_eq!(choose(false),2); }
#[test] fn weak_true_only() { assert_eq!(weak(true),1); }
#[test] fn initialization() { assert_eq!(initialized(true),7); }
