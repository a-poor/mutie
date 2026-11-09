use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use ucd_parse::{Age, ArabicShaping, UnicodeDataDecomposition};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct InputDir(PathBuf);
impl InputDir {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "mutie-ucd-string-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("creating owned input directory: {e}"),
            }
        }
        panic!("could not create owned input directory");
    }
}
impl Drop for InputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn joining_type_labels_round_trip_through_a_row() {
    for label in ["R", "L", "D", "C", "U", "T"] {
        let input = format!("0041; SYNTHETIC TEST; {label}; No_Joining_Group");
        let row: ArabicShaping = input.parse().unwrap();
        assert_eq!(row.joining_type.as_str(), label);
    }
}

#[test]
fn decomposition_tags_round_trip_with_their_mapping() {
    for tag in [
        "font", "noBreak", "initial", "medial", "final", "isolated", "circle", "super", "sub",
        "vertical", "wide", "narrow", "small", "square", "fraction", "compat",
    ] {
        let input = format!("<{tag}> 0041");
        let parsed: UnicodeDataDecomposition = input.parse().unwrap();
        assert_eq!(parsed.to_string(), input);
    }
}

#[test]
fn directory_parser_reads_the_named_age_file() {
    let input = InputDir::new();
    fs::write(input.0.join("DerivedAge.txt"), "0041 ; 1.1\n").unwrap();
    let rows = ucd_parse::parse::<_, Age>(&input.0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].codepoints, 0x41);
    assert_eq!(rows[0].age, "1.1");
}

#[test]
fn directory_version_reads_the_property_header() {
    let input = InputDir::new();
    fs::write(input.0.join("PropList.txt"), "# PropList-15.1.2.txt\n").unwrap();
    assert_eq!(
        ucd_parse::ucd_directory_version(&input.0).unwrap(),
        (15, 1, 2)
    );
}
