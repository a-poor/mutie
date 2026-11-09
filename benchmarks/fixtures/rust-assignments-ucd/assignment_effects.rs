use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use ucd_parse::{Age, Codepoint, UnicodeData, UnicodeDataDecomposition};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct InputDir(PathBuf);
impl InputDir {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "mutie-ucd-assignment-test-{}-{}",
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
fn parse_errors_identify_the_physical_input_line() {
    let input = InputDir::new();
    fs::write(
        input.0.join("DerivedAge.txt"),
        "# ignored comment\n\n0041 ; 1.1\nnot a valid UCD row\n",
    )
    .unwrap();
    let error = ucd_parse::parse::<_, Age>(&input.0).unwrap_err();
    assert_eq!(error.line(), Some(4));
}

fn row(mirrored: &str, iso_comment: &str) -> UnicodeData {
    [
        "0028",
        "LEFT PARENTHESIS",
        "Ps",
        "0",
        "ON",
        "",
        "",
        "",
        "",
        mirrored,
        "",
        iso_comment,
        "",
        "",
        "",
    ]
    .join(";")
    .parse()
    .unwrap()
}

#[test]
fn bidirectional_mirroring_retains_both_values() {
    assert!(row("Y", "").bidi_mirrored);
    assert!(!row("N", "").bidi_mirrored);
}

#[test]
fn accepted_historical_iso_comment_is_preserved() {
    // Modern UCD files leave this obsolete field empty. This checks the parser's
    // accepted nonempty field, not a claim about current Unicode data contents.
    assert_eq!(
        row("N", "historical annotation").iso_comment,
        "historical annotation"
    );
}

#[test]
fn decomposition_stores_codepoints_in_order() {
    let expected = [
        Codepoint::from_u32(0x41).unwrap(),
        Codepoint::from_u32(0x301).unwrap(),
    ];
    let mapping = UnicodeDataDecomposition::new(None, &expected).unwrap();
    assert_eq!(mapping.mapping(), &expected);
}
