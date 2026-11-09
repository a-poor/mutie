pub fn checked() -> &'static str {
    "ready"
}

pub fn weak() -> &'static str {
    r#""#
}

#[cfg(test)]
fn excluded() -> &'static str {
    "excluded"
}
