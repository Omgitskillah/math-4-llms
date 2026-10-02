use math_4_llms::evaluate_response;

fn main() {
    let cases: Vec<(String, String)> =
        serde_json::from_str(include_str!("../tests/model-inputs.json")).unwrap();
    for (name, input) in cases {
        println!(
            "{}",
            serde_json::json!({
                "case": name, "input": input,
            "response": evaluate_response(&input)
            })
        );
    }
}
