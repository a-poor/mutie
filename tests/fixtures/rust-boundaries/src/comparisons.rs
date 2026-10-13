pub fn less(left: i64, right: i64) -> bool {
    left < right
}

pub fn at_most<T: PartialOrd>(left: &T, right: &T) -> bool {
    left <= right
}

pub fn greater(left: f64, right: f64) -> bool {
    left > right
}

pub fn at_least(left: u64, right: u64) -> bool {
    left >= right
}

#[allow(dead_code)]
pub fn evaluated_once(log: &mut String) -> bool {
    fn operand(log: &mut String, name: char) -> i32 {
        log.push(name);
        4
    }
    operand(log, 'L') < operand(log, 'R')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_boundary_includes_extremes() {
        assert!(less(i64::MIN, i64::MAX));
        assert!(!less(i64::MAX, i64::MIN));
        assert!(!less(7, 7));
    }

    #[test]
    fn generic_comparisons_borrow_noncopy_operands() {
        let left = String::from("same");
        let right = String::from("same");
        assert!(at_most(&left, &right));
        assert_eq!(left, right);
    }

    #[test]
    fn float_boundary_and_unordered_values() {
        assert!(!greater(f64::NAN, 0.0));
        assert!(!greater(0.0, f64::NAN));
        assert!(greater(f64::INFINITY, 0.0));
        assert!(!greater(0.0, -0.0));
    }

    #[test]
    fn operand_side_effects_remain_left_to_right_and_once() {
        let mut log = String::new();
        let result = evaluated_once(&mut log);
        assert_eq!(log, "LR");
        assert!(!result);
    }
}
