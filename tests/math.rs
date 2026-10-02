use math_4_llms::{ErrorCode, MathError, evaluate};
use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize)]
struct Case {
    name: String,
    expression: String,
    expected: f64,
}

#[test]
fn ten_research_cases_match_references() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("research-expressions.json")).unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let result = evaluate(&case.expression).unwrap();
        let number: f64 = result
            .strip_prefix("approx. ")
            .unwrap_or(&result)
            .parse()
            .unwrap();
        assert!(
            (number - case.expected).abs() <= 1e-9,
            "{}: {result}",
            case.name
        );
    }
}

#[test]
fn malformed_and_invalid_arithmetic_return_errors() {
    for expression in ["2 + (", "1 / 0", "unknown_function(2)"] {
        assert!(evaluate(expression).is_err(), "{expression}");
    }
    assert!(evaluate(" \n").unwrap_err().code == ErrorCode::EmptyInput);
    assert!(matches!(
        evaluate(&"1".repeat(1025)),
        Err(MathError {
            code: ErrorCode::InputTooLong,
            ..
        })
    ));
    assert_eq!(evaluate(&format!("{}2", " ".repeat(1023))).unwrap(), "2");
}

#[test]
fn precedence_and_decimal_behavior_are_explicit() {
    for (input, expected) in [
        ("2 + 3 * 4", "14"),
        ("-2^2", "-4"),
        ("(2^3)^2", "64"),
        ("2^(3^2)", "512"),
        ("0.1 + 0.2", "0.3"),
        ("10 / 4", "2.5"),
    ] {
        assert_eq!(evaluate(input).unwrap(), expected);
    }
}

#[test]
fn contexts_do_not_leak_between_calls() {
    assert!(evaluate("research_variable = 123").is_err());
    assert!(evaluate("research_variable + 1").is_err());
}

#[test]
fn cli_returns_results_and_nonzero_errors() {
    let bin = env!("CARGO_BIN_EXE_math-tool");
    let output = Command::new(bin).arg("2+3*4").output().unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"schema_version":1,"ok":true,"result":"14","approximate":false})
    );
    let output = Command::new(bin).arg("2 + (").output().unwrap();
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "UNBALANCED_PARENTHESES");
    assert!(json.get("result").is_none());
    for args in [vec![], vec!["fend", "2"], vec!["evalexpr", "2"]] {
        assert!(
            !Command::new(bin)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    for flag in ["--help", "-h"] {
        assert!(
            Command::new(bin)
                .arg(flag)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
