use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use ucd_parse::{Age, Codepoint};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct InputDir(PathBuf);
impl InputDir {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "mutie-ucd-numeric-test-{}-{}",
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
fn version_components_keep_their_positions() {
    let input = InputDir::new();
    fs::write(input.0.join("PropList.txt"), "# PropList-15.1.2.txt\n").unwrap();
    assert_eq!(
        ucd_parse::ucd_directory_version(&input.0).unwrap(),
        (15, 1, 2)
    );
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

#[test]
fn a_blank_line_does_not_end_the_file() {
    let input = InputDir::new();
    fs::write(input.0.join("DerivedAge.txt"), "\n0041 ; 1.1\n").unwrap();
    let rows = ucd_parse::parse::<_, Age>(&input.0).unwrap();
    assert_eq!(rows.len(), 1);
}

#[test]
fn a_single_codepoint_iterator_stops_after_one_value() {
    let codepoint = Codepoint::from_u32(0x41).unwrap();
    // Bound consumption so a broken increment becomes an assertion, not a hang.
    let values: Vec<_> = codepoint.into_iter().take(2).collect();
    assert_eq!(values, [codepoint]);
}
