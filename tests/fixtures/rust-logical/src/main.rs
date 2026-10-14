mod logic;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = args[1].parse().unwrap();
    let b = args[2].parse().unwrap();
    let c = args[3].parse().unwrap();
    let mut trace = String::new();
    let result = match args[0].as_str() {
        "all" => logic::all(a, b, &mut trace),
        "any" => logic::any(a, b, &mut trace),
        "weak" => logic::weak_any(a, b),
        "all-three" => logic::all_three(a, b, c),
        "any-three" => logic::any_three(a, b, c),
        "chain" => logic::chain(a, b, c),
        _ => panic!("unknown mode"),
    };
    println!("{result}:{trace}");
}
