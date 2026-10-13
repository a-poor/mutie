mod comparisons;

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().expect("comparison name");
    let left = args.next().expect("left operand");
    let right = args.next().expect("right operand");
    let result = match mode.as_str() {
        "less" => comparisons::less(left.parse().unwrap(), right.parse().unwrap()),
        "at-most" => comparisons::at_most(&left, &right),
        "greater" => comparisons::greater(left.parse().unwrap(), right.parse().unwrap()),
        "at-least" => comparisons::at_least(left.parse().unwrap(), right.parse().unwrap()),
        _ => std::process::exit(2),
    };
    println!("{result}");
}
