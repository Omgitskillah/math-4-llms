//! Versioned, strict scalar calculator backed by fend. Never executes shell code
//! or reads/writes notes. Positions are zero-based UTF-8 byte offsets.
use serde::Serialize;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

mod validation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
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
}

#[derive(Debug, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct MathError {
    pub code: ErrorCode,
    pub message: &'static str,
    pub hint: &'static str,
    pub position: Option<usize>,
}

impl MathError {
    pub fn new(code: ErrorCode, position: Option<usize>) -> Self {
        use ErrorCode::*;
        let (message, hint) = match code {
            EmptyInput => (
                "The expression is blank.",
                "Send one mathematical expression, for example 2 + 3.",
            ),
            InputTooLong => (
                "The expression exceeds 1024 UTF-8 bytes.",
                "Shorten the expression; nothing has been truncated.",
            ),
            UnsupportedFormatting => (
                "Text wrappers or non-parenthesis brackets are not supported.",
                "Send the expression without quotes, Markdown, LaTeX, or JSON wrappers. Use parentheses for grouping.",
            ),
            UnsupportedCharacter => (
                "The expression contains an unsupported character.",
                "Use ASCII numbers and operators + - * / ^, and remove invisible characters. Ordinary whitespace is allowed.",
            ),
            AmbiguousComma => (
                "Commas are not allowed in this expression format.",
                "Use a period for decimal fractions and no thousands separators. Only single-argument functions are supported.",
            ),
            MultipleStatements => (
                "Only one expression is allowed.",
                "Remove semicolons and send each calculation separately.",
            ),
            AssignmentNotAllowed => (
                "Assignments and equations are not supported.",
                "Substitute numeric values and send only the expression to evaluate, without an equals sign.",
            ),
            UnbalancedParentheses => (
                "Opening and closing parentheses do not match.",
                "Correct the parentheses explicitly; the tool will not add or remove them.",
            ),
            NestingTooDeep => (
                "Parenthesis nesting exceeds 32 levels.",
                "Simplify the nesting or split the calculation into smaller steps.",
            ),
            UnknownIdentifier => (
                "A name is not an approved function or constant.",
                "Use only sqrt, exp, ln, log10, sin, cos, tan, abs, pi, or e. Remove prose, variables, and units.",
            ),
            InvalidSyntax => (
                "The expression does not follow the supported grammar.",
                "Use explicit multiplication *, powers ^, and function parentheses, for example sqrt(9). Do not split numbers or repeat binary operators.",
            ),
            EvaluationFailed => (
                "The valid expression could not be evaluated.",
                "Check denominators and function domains, or simplify the calculation. No result is available.",
            ),
            EvaluationTimeout => (
                "Evaluation reached the cooperative time limit.",
                "Simplify the calculation; retrying the identical expression may time out again.",
            ),
            NonScalarResult => (
                "Evaluation did not produce a supported real scalar result.",
                "Check function domains. Complex values, units, and text results are not supported.",
            ),
            InvalidArguments => (
                "The CLI requires exactly one expression argument.",
                "Pass a single shell-quoted expression: math-tool '2 + 3'.",
            ),
        };
        Self {
            code,
            message,
            hint,
            position,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MathResponse {
    pub schema_version: u8,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MathError>,
}

impl MathResponse {
    pub fn failure(error: MathError) -> Self {
        Self {
            schema_version: 1,
            ok: false,
            result: None,
            approximate: None,
            error: Some(error),
        }
    }
}

/// Structured tool response. Never contains both a result and an error.
pub fn evaluate_response(expression: &str) -> MathResponse {
    match evaluate(expression) {
        Ok(result) => MathResponse {
            schema_version: 1,
            ok: true,
            approximate: Some(result.starts_with("approx. ")),
            result: Some(result),
            error: None,
        },
        Err(error) => MathResponse::failure(error),
    }
}

struct Deadline {
    end: Instant,
    interrupted: AtomicBool,
}
impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        let expired = Instant::now() >= self.end;
        if expired {
            self.interrupted.store(true, Ordering::Relaxed);
        }
        expired
    }
}

/// Strict scalar calculation. Length/nesting are bounded; evaluation cancellation
/// is cooperative and does not provide a hard CPU/memory sandbox.
pub fn evaluate(expression: &str) -> Result<String, MathError> {
    validation::validate(expression)?;
    evaluate_validated(
        expression,
        &Deadline {
            end: Instant::now() + Duration::from_secs(2),
            interrupted: AtomicBool::new(false),
        },
    )
}

fn evaluate_validated(expression: &str, deadline: &Deadline) -> Result<String, MathError> {
    let mut context = fend_core::Context::new();
    let result = fend_core::evaluate_with_interrupt(expression, &mut context, deadline);
    if deadline.interrupted.load(Ordering::Relaxed) {
        return Err(MathError::new(ErrorCode::EvaluationTimeout, None));
    }
    let result = result.map_err(|_| MathError::new(ErrorCode::EvaluationFailed, None))?;
    let text = result.get_main_result();
    if !validation::is_scalar(text.strip_prefix("approx. ").unwrap_or(text)) {
        return Err(MathError::new(ErrorCode::NonScalarResult, None));
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_has_stable_code_without_timing_races() {
        let deadline = Deadline {
            end: Instant::now(),
            interrupted: AtomicBool::new(false),
        };
        assert_eq!(
            evaluate_validated("2^100000", &deadline).unwrap_err().code,
            ErrorCode::EvaluationTimeout
        );
    }
}
