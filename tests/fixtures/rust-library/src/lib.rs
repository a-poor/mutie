//! A library API also consumed by the fixture binary.

/// Whether access is enabled.
///
/// ```
/// assert!(mutie_rust_library_fixture::enabled());
/// ```
pub fn enabled() -> bool {
    true
}

/// An initially untested API for the stronger-test rerun workflow.
pub fn preview_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn access_is_enabled() {
        assert!(super::enabled());
    }
}
