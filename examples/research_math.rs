use math_4_llms::evaluate;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    name: String,
    expression: String,
    expected: f64,
}

fn main() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../tests/research-expressions.json")).unwrap();
    for case in cases {
        println!("{} | expected {}", case.name, case.expected);
        match evaluate(&case.expression) {
            Ok(result) => println!("  fend: {result}"),
            Err(error) => println!("  fend: ERROR {error}"),
        }
    }
}
