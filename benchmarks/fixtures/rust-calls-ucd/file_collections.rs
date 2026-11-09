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
        // create_dir never adopts an existing directory. Keep cleanup scoped to
        // the directory this test created, even if another process uses TMPDIR.
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "mutie-ucd-call-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    let dir = Self(path);
                    fs::write(
                        dir.0.join("DerivedAge.txt"),
                        "# tiny independent UCD sample\n0041..0042 ; 1.1\n0061 ; 2.0\n",
                    )
                    .unwrap();
                    return dir;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("creating test input: {e}"),
            }
        }
        panic!("could not create owned test input directory");
    }
}
impl Drop for InputDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn file_parser_keeps_rows_and_their_order() {
    let input = InputDir::new();
    let rows = ucd_parse::parse::<_, Age>(&input.0).unwrap();
    let expected: Vec<Age> = ["0041..0042 ; 1.1", "0061 ; 2.0"]
        .iter()
        .map(|line| line.parse().unwrap())
        .collect();
    assert_eq!(rows, expected);
}

#[test]
fn codepoint_map_expands_ranges_and_keeps_records() {
    let input = InputDir::new();
    let rows = ucd_parse::parse_by_codepoint::<_, Age>(&input.0).unwrap();
    let expected: std::collections::BTreeMap<Codepoint, Age> = [
        (0x41, "0041..0042 ; 1.1"),
        (0x42, "0041..0042 ; 1.1"),
        (0x61, "0061 ; 2.0"),
    ]
    .into_iter()
    .map(|(cp, line)| (Codepoint::from_u32(cp).unwrap(), line.parse().unwrap()))
    .collect();
    assert_eq!(rows, expected);
}
