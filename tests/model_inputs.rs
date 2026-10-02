use math_4_llms::{ErrorCode, MathError, MathResponse, evaluate, evaluate_response};

#[test]
fn public_v1_error_names_are_pinned() {
    use ErrorCode::*;
    let codes = [
        EmptyInput,
        InputTooLong,
        UnsupportedFormatting,
        UnsupportedCharacter,
        AmbiguousComma,
        MultipleStatements,
        AssignmentNotAllowed,
        UnbalancedParentheses,
        NestingTooDeep,
        UnknownIdentifier,
        InvalidSyntax,
        EvaluationFailed,
        EvaluationTimeout,
        NonScalarResult,
        InvalidArguments,
    ];
    let names = [
        "EMPTY_INPUT",
        "INPUT_TOO_LONG",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_CHARACTER",
        "AMBIGUOUS_COMMA",
        "MULTIPLE_STATEMENTS",
        "ASSIGNMENT_NOT_ALLOWED",
        "UNBALANCED_PARENTHESES",
        "NESTING_TOO_DEEP",
        "UNKNOWN_IDENTIFIER",
        "INVALID_SYNTAX",
        "EVALUATION_FAILED",
        "EVALUATION_TIMEOUT",
        "NON_SCALAR_RESULT",
        "INVALID_ARGUMENTS",
    ];
    for (code, name) in codes.into_iter().zip(names) {
        let response = MathResponse::failure(MathError::new(code, None));
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["error"]["code"], name);
        assert!(json["error"]["position"].is_null());
        assert!(!json["error"]["message"].as_str().unwrap().is_empty());
        assert!(!json["error"]["hint"].as_str().unwrap().is_empty());
        assert!(json.get("result").is_none());
        assert!(json.get("approximate").is_none());
    }
}

#[test]
fn model_corpus_has_stable_results_and_error_codes() {
    let cases: Vec<(String, String)> =
        serde_json::from_str(include_str!("model-inputs.json")).unwrap();
    // Prefix = denotes a successful result; other entries are public v1 codes.
    let expected = [
        "=14",
        "=14",
        "=14",
        "=5",
        "UNSUPPORTED_CHARACTER",
        "=5",
        "UNBALANCED_PARENTHESES",
        "UNBALANCED_PARENTHESES",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_FORMATTING",
        "INVALID_SYNTAX",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_FORMATTING",
        "UNKNOWN_IDENTIFIER",
        "UNKNOWN_IDENTIFIER",
        "ASSIGNMENT_NOT_ALLOWED",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_CHARACTER",
        "UNSUPPORTED_CHARACTER",
        "UNSUPPORTED_CHARACTER",
        "AMBIGUOUS_COMMA",
        "AMBIGUOUS_COMMA",
        "INVALID_SYNTAX",
        "=5",
        "=5",
        "INVALID_SYNTAX",
        "INVALID_SYNTAX",
        "UNKNOWN_IDENTIFIER",
        "MULTIPLE_STATEMENTS",
        "UNKNOWN_IDENTIFIER",
        "UNKNOWN_IDENTIFIER",
        "NON_SCALAR_RESULT",
        "UNSUPPORTED_FORMATTING",
        "UNSUPPORTED_CHARACTER",
        "EMPTY_INPUT",
        "EMPTY_INPUT",
        "=0.003",
        "=512",
    ];
    assert_eq!(cases.len(), expected.len());
    for ((name, input), expected) in cases.iter().zip(expected) {
        let response = serde_json::to_value(evaluate_response(input)).unwrap();
        assert_eq!(response["schema_version"], 1);
        if let Some(value) = expected.strip_prefix('=') {
            assert_eq!(response["result"], value, "{name}");
            assert_eq!(response["ok"], true);
            assert!(response.get("error").is_none());
        } else {
            assert_eq!(response["error"]["code"], expected, "{name}: {response}");
            assert_eq!(response["ok"], false);
            assert!(response.get("result").is_none());
            assert!(!response["error"]["hint"].as_str().unwrap().is_empty());
        }
    }
}

#[test]
fn whitespace_between_tokens_does_not_change_results() {
    let spaces = [" ", "   ", "\t", "\n", "\r\n"];
    for left in spaces {
        for middle in spaces {
            for right in spaces {
                let input = format!("{left}2{middle}+{right}3 * 4{left}");
                assert_eq!(evaluate(&input).unwrap(), "14", "{input:?}");
            }
        }
    }
}

#[test]
fn nesting_and_byte_limits_are_enforced() {
    for depth in 1..=32 {
        assert_eq!(
            evaluate(&format!("{}2+3{}", "(".repeat(depth), ")".repeat(depth))).unwrap(),
            "5"
        );
    }
    assert_eq!(
        evaluate(&format!("{}2{}", "(".repeat(33), ")".repeat(33)))
            .unwrap_err()
            .code,
        ErrorCode::NestingTooDeep
    );
    assert_eq!(
        evaluate(&format!("{}2", "\u{a0}".repeat(511))).unwrap(),
        "2"
    );
    for input in [format!("{}2", "\u{a0}".repeat(512)), " ".repeat(2048)] {
        assert_eq!(evaluate(&input).unwrap_err().code, ErrorCode::InputTooLong);
    }
}

#[test]
fn stable_errors_include_positions_and_do_not_depend_on_engine_messages() {
    for input in ["1/0", "ln(0)"] {
        let error = evaluate(input).unwrap_err();
        assert_eq!(error.code, ErrorCode::EvaluationFailed);
        assert_eq!(error.position, None);
        assert_eq!(
            error.message,
            "The valid expression could not be evaluated."
        );
    }
    let error = evaluate("\u{a0}2 + ]").unwrap_err();
    assert_eq!(error.position, Some(6)); // UTF-8 byte offset, not character index.
    assert_eq!(evaluate("(2+3").unwrap_err().position, Some(0));
    for input in [
        "1e", ".", "1e+", "2 3", "sqrt 9", "sqrt()", "2***3", "2+", "---2", "2(3)", "sin(1,2)",
    ] {
        assert!(evaluate(input).is_err(), "{input}");
    }
}

#[test]
fn approved_functions_and_result_metadata_are_explicit() {
    for (input, value) in [
        ("sqrt(9)", "3"),
        ("exp(0)", "1"),
        ("ln(1)", "0"),
        ("log10(1)", "0"),
        ("sin(0)", "0"),
        ("cos(0)", "1"),
        ("tan(0)", "0"),
        ("abs(-2)", "2"),
        (".5 + 0.5", "1"),
        ("+2", "2"),
    ] {
        let result = evaluate(input).unwrap();
        assert_eq!(
            result.strip_prefix("approx. ").unwrap_or(&result),
            value,
            "{input}"
        );
    }
    for constant in ["pi", "e"] {
        assert!(evaluate(constant).is_ok());
    }
    let response = evaluate_response("ln(2)");
    assert_eq!(response.approximate, Some(true));
    assert!(response.result.unwrap().starts_with("approx. "));
    assert_eq!(
        evaluate("sqrt(-1)").unwrap_err().code,
        ErrorCode::NonScalarResult
    );
}

#[test]
fn adjacent_operands_inside_groups_are_not_silently_multiplied() {
    for expression in ["(2 3)", "sqrt(2 3)", "(pi e)"] {
        let response = evaluate_response(expression);
        assert!(!response.ok, "{expression}");
        assert_eq!(response.error.unwrap().code, ErrorCode::InvalidSyntax);
        assert!(response.result.is_none());
    }
}
