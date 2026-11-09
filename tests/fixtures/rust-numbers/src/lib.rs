const NUMERATOR: u8 = 10;
const VALUES: [u8; 1] = [9];

pub fn checked() -> u8 {
    7u8
}

pub fn weak() -> f32 {
    0.0f32
}

pub fn divide() -> u8 {
    NUMERATOR / 1
}

pub fn indexed() -> u8 {
    VALUES[0]
}

#[cfg(test)]
fn excluded() -> u8 {
    99
}
