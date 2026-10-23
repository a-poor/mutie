pub fn eq(a: i32, b: i32) -> bool {
    a == b
}
pub fn ne(a: i32, b: i32) -> bool {
    a != b
}
pub fn lt(a: f64, b: f64) -> bool {
    a < b
}
pub fn le(a: i32, b: i32) -> bool {
    a <= b
}
pub fn gt(a: i32, b: i32) -> bool {
    a > b
}
pub fn ge(a: i32, b: i32) -> bool {
    a >= b
}
pub fn bool_not(x: bool) -> bool {
    !x
}
pub fn bit_not(x: u8) -> u8 {
    !x
}
pub fn weak_lt(a: f64, b: f64) -> bool {
    a < b
}
pub fn weak_not(x: bool) -> bool {
    !x
}
pub struct Flag;
impl std::ops::Not for Flag {
    type Output = bool;
    fn not(self) -> bool {
        false
    }
}
pub fn type_change(x: Flag) -> bool {
    !x
}
