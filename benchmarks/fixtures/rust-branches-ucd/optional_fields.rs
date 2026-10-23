use ucd_parse::{Codepoint, UnicodeData};

#[test]
fn numeric_decimal_and_digit_fields_are_not_silently_dropped() {
    let line = "0030;DIGIT ZERO;Nd;0;EN;;0;0;0;N;;;;;";
    let record: UnicodeData = line.parse().unwrap();
    assert_eq!(record.numeric_type_decimal, Some(0));
    assert_eq!(record.numeric_type_digit, Some(0));
}

#[test]
fn uppercase_and_titlecase_mappings_are_not_silently_dropped() {
    let line = "0061;LATIN SMALL LETTER A;Ll;0;L;;;;;N;;;0041;;0041";
    let record: UnicodeData = line.parse().unwrap();
    let uppercase = Codepoint::from_u32(0x41).unwrap();
    assert_eq!(record.simple_uppercase_mapping, Some(uppercase));
    assert_eq!(record.simple_titlecase_mapping, Some(uppercase));
}
