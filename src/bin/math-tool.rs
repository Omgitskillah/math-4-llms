use math_4_llms::{ErrorCode, MathError, MathResponse, evaluate_response};
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        println!("usage: math-tool '<expression>'");
        return ExitCode::SUCCESS;
    }
    let response = if args.len() == 1 {
        evaluate_response(&args[0])
    } else {
        MathResponse::failure(MathError::new(ErrorCode::InvalidArguments, None))
    };
    println!(
        "{}",
        serde_json::to_string(&response).expect("serializable calculator response")
    );
    if response.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
