//! Native identity-only Node stand-in, compiled by vitest_preview's fixture.
use std::{env, fs, io::Write, path::PathBuf};

fn main() {
    let mut args = env::args_os();
    let program = PathBuf::from(args.next().unwrap());
    let first = args.next().unwrap();
    if first == "--fixture-ready" {
        return;
    }
    assert_eq!(first, "--eval", "must not execute Vitest");
    assert_eq!(
        args.next().unwrap(),
        "console.log(JSON.stringify({version:process.version,executable:process.execPath}))"
    );
    assert!(args.next().is_none());
    let program = if program.is_absolute() {
        program
    } else {
        env::split_paths(&env::var_os("PATH").unwrap())
            .map(|dir| dir.join(&program))
            .find(|path| path.is_file())
            .unwrap()
    };
    let response = fs::read_to_string(program.with_extension("response")).unwrap();
    let (counter, identity) = response.split_once('\n').unwrap();
    if !counter.is_empty() {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(counter)
            .unwrap();
        writeln!(file, "called").unwrap();
    }
    println!("{identity}");
}
