pub fn add(a: i32, b: i32) -> i32 { a + b }
pub fn subtract(a: i32, b: i32) -> i32 { a - b }
pub fn multiply(a: i32, b: i32) -> i32 { a*/* right operand */b }
pub fn divide(a: i32, b: i32) -> i32 { a / b }
pub fn remainder(a: i32, b: i32) -> i32 { a % b }
pub fn weak_add(a: i32, b: i32) -> i32 { a + b }
pub fn weak_remainder(a: i32, b: i32) -> i32 { a % b }
pub fn zero_product(a: i32, b: i32) -> i32 { a * b }
pub fn maximum_minus(a: i32, b: i32) -> i32 { a - b }
// Addition is valid; Rust has no String subtraction. Keep this as a build failure.
pub fn append(a: String, b: &str) -> String { a + b }
