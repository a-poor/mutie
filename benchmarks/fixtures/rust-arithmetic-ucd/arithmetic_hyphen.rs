use ucd_util::character_name_normalize;

#[test]
fn hyphen_requires_a_letter_on_both_sides_to_be_medial() {
    for (input, expected) in [("A-1", "a-1"), ("A-", "a-"), ("A-B", "ab")] {
        let mut name = input.to_string();
        character_name_normalize(&mut name);
        assert_eq!(name, expected, "input: {input}");
    }
}
