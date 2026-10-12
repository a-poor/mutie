pub fn unit_flag() -> bool {
    true
}

pub fn cli_flag() -> bool {
    false
}

pub fn untested_flag() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn unit_flag_is_enabled() {
        assert_eq!(super::unit_flag(), crate::test_support::expected_flag());
    }
}
