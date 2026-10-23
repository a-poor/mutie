pub fn count(mut n: u8) -> u8 {
    let mut total = 0;
    while n > 0 {
        total += 1;
        n -= 1;
    }
    total
}
pub fn weak(mut n: u8) -> u8 {
    let mut total = 0;
    while n > 0 {
        total += 1;
        n -= 1;
    }
    total
}
pub fn initialized(flag: bool) -> u8 {
    let mut value;
    while {
        value = 7;
        flag
    } {
        break;
    }
    value
}
