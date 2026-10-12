mod flags;
mod generated;

#[cfg(feature = "alternate")]
mod alternate;
#[cfg(any())]
mod inactive;
#[cfg(test)]
mod test_support;

// Macro bodies and their expanded code are outside the initial mutation scope.
macro_rules! define_flag {
    ($name:ident) => {
        fn $name() -> bool {
            true
        }
    };
}
define_flag!(macro_flag);

fn main() {
    let value = match std::env::args().nth(1).as_deref() {
        Some("unit") => flags::unit_flag(),
        Some("checked") => flags::cli_flag(),
        Some("unchecked") => flags::untested_flag(),
        Some("generated") => generated::flag(),
        Some("macro") => macro_flag(),
        #[cfg(feature = "alternate")]
        Some("alternate") => alternate::flag(),
        _ => std::process::exit(2),
    };
    println!("{value}");
}
