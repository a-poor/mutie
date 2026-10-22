//! Additional public-API boundaries for the separate stronger-test checkout.
use ucd_util::{character_name_normalize, symbolic_name_normalize};

#[test]
fn leading_and_trailing_hyphens_are_not_medial() {
    for (input, expected) in [("-A", "-a"), ("A-", "a-"), ("A-B", "ab")] {
        let mut value = input.to_owned();
        character_name_normalize(&mut value);
        assert_eq!(value, expected, "input: {input}");
    }
}

#[test]
fn short_symbolic_names_do_not_acquire_the_is_prefix() {
    for (input, expected) in [("C", "c"), ("c", "c"), ("IsC", "isc")] {
        let mut value = input.to_owned();
        symbolic_name_normalize(&mut value);
        assert_eq!(value, expected, "input: {input}");
    }
}
