use ucd_parse::{UnicodeData, UnicodeDataDecomposition};

#[test]
fn canonical_decomposition_survives_unicode_data_round_trip() {
    let line = "00C0;LATIN CAPITAL LETTER A WITH GRAVE;Lu;0;L;0041 0300;;;;N;;;;00E0;";
    let record: UnicodeData = line.parse().unwrap();
    let rendered = record.to_string();
    assert_eq!(rendered.split(';').nth(5), Some("0041 0300"));
    assert_eq!(rendered.parse::<UnicodeData>().unwrap(), record);
}

#[test]
fn decomposition_format_has_exact_separators() {
    for text in ["0041", "0041 0300", "<compat> 0041 0300"] {
        let mapping: UnicodeDataDecomposition = text.parse().unwrap();
        assert_eq!(mapping.to_string(), text);
    }
}
