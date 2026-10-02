//! Local stdio MCP adapter for the strict calculator.

use crate::{MathResponse, evaluate_response};
use rmcp::{
    Json, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{Implementation, ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use statrs::statistics::{Data, Median, Statistics};

const MAX_STATISTICS_VALUES: usize = 10_000;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CalculateMathRequest {
    /// One strict scalar expression, without prose, Markdown, or JSON wrapping.
    pub expression: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatisticsRequest {
    /// Between 2 and 10,000 finite numbers.
    #[schemars(length(min = 2, max = 10_000))]
    pub numbers: Vec<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatisticsResponse {
    pub schema_version: u8,
    pub ok: bool,
    pub count: usize,
    pub sum: f64,
    pub mean: f64,
    pub median: f64,
    pub sample_standard_deviation: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatisticsErrorCode {
    EmptyDataset,
    SampleTooSmall,
    TooManyValues,
    NonFiniteInput,
    NonFiniteResult,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatisticsError {
    pub schema_version: u8,
    pub ok: bool,
    pub code: StatisticsErrorCode,
    pub message: &'static str,
    pub hint: &'static str,
}

impl StatisticsError {
    fn new(code: StatisticsErrorCode) -> Self {
        use StatisticsErrorCode::*;
        let (message, hint) = match code {
            EmptyDataset => (
                "The dataset is empty.",
                "Provide at least two finite numbers for sample statistics.",
            ),
            SampleTooSmall => (
                "Sample standard deviation requires at least two numbers.",
                "Add another finite observation or do not request sample statistics.",
            ),
            TooManyValues => (
                "The dataset contains more than 10,000 numbers.",
                "Split or summarize the dataset before calling this tool.",
            ),
            NonFiniteInput => (
                "Every input must be a finite number.",
                "Remove NaN and positive or negative infinity values.",
            ),
            NonFiniteResult => (
                "The dataset produced a non-finite statistic.",
                "Reduce the magnitude of the values or rescale the dataset.",
            ),
        };
        Self {
            schema_version: 1,
            ok: false,
            code,
            message,
            hint,
        }
    }
}

fn compensated_sum(numbers: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for &number in numbers {
        let next = sum + number;
        if sum.abs() >= number.abs() {
            correction += (sum - next) + number;
        } else {
            correction += (number - next) + sum;
        }
        sum = next;
    }
    sum + correction
}

fn summarize_statistics(numbers: Vec<f64>) -> Result<StatisticsResponse, StatisticsError> {
    if numbers.is_empty() {
        return Err(StatisticsError::new(StatisticsErrorCode::EmptyDataset));
    }
    if numbers.len() == 1 {
        return Err(StatisticsError::new(StatisticsErrorCode::SampleTooSmall));
    }
    if numbers.len() > MAX_STATISTICS_VALUES {
        return Err(StatisticsError::new(StatisticsErrorCode::TooManyValues));
    }
    if numbers.iter().any(|number| !number.is_finite()) {
        return Err(StatisticsError::new(StatisticsErrorCode::NonFiniteInput));
    }

    let count = numbers.len();
    let sum = compensated_sum(&numbers);
    let mean = sum / count as f64;
    let sample_standard_deviation = numbers.as_slice().std_dev();
    let min = numbers.as_slice().min();
    let max = numbers.as_slice().max();
    let median = Data::new(numbers).median();
    let statistics = [sum, mean, median, sample_standard_deviation, min, max];
    if statistics.iter().any(|value| !value.is_finite()) {
        return Err(StatisticsError::new(StatisticsErrorCode::NonFiniteResult));
    }

    Ok(StatisticsResponse {
        schema_version: 1,
        ok: true,
        count,
        sum,
        mean,
        median,
        sample_standard_deviation,
        min,
        max,
    })
}

#[derive(Clone, Debug, Default)]
pub struct MathMcpServer;

#[tool_router]
impl MathMcpServer {
    /// Evaluate one strict scalar mathematical expression.
    ///
    /// The versioned response includes either a result and approximation flag,
    /// or a stable error code, message, hint, and optional UTF-8 byte position.
    #[tool(
        name = "calculate",
        description = "Evaluate one strict scalar mathematical expression. Send only the expression, without prose, Markdown, LaTeX, or JSON wrapping. Returns a versioned result or stable correction error."
    )]
    fn calculate(
        &self,
        Parameters(CalculateMathRequest { expression }): Parameters<CalculateMathRequest>,
    ) -> Result<Json<MathResponse>, Json<MathResponse>> {
        let response = evaluate_response(&expression);
        if response.ok {
            Ok(Json(response))
        } else {
            Err(Json(response))
        }
    }

    /// Calculate common descriptive sample statistics for one numeric dataset.
    #[tool(
        name = "statistics",
        description = "Return count, compensated sum, mean, median, sample standard deviation (n-1), minimum, and maximum for 2 to 10,000 finite numbers."
    )]
    fn statistics(
        &self,
        Parameters(StatisticsRequest { numbers }): Parameters<StatisticsRequest>,
    ) -> Result<Json<StatisticsResponse>, Json<StatisticsError>> {
        summarize_statistics(numbers).map(Json).map_err(Json)
    }
}

#[tool_handler]
impl ServerHandler for MathMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("math-4-llms", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Call calculate for one strict scalar expression or statistics for one numeric dataset. Inspect ok before using a result.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() <= 1e-12, "{actual} != {expected}");
    }

    #[test]
    fn statistics_cover_ordering_duplicates_fractions_and_compensated_sum() {
        let summary = summarize_statistics(vec![10.0, 2.0, 4.0, 8.0]).unwrap();
        assert_eq!(summary.count, 4);
        assert_close(summary.sum, 24.0);
        assert_close(summary.mean, 6.0);
        assert_close(summary.median, 6.0);
        assert_close(summary.sample_standard_deviation, (40.0_f64 / 3.0).sqrt());
        assert_close(summary.min, 2.0);
        assert_close(summary.max, 10.0);

        let duplicates = summarize_statistics(vec![-2.5, 0.5, 0.5, 4.5]).unwrap();
        assert_close(duplicates.median, 0.5);
        assert_close(duplicates.sum, 3.0);

        let cancellation = summarize_statistics(vec![1e16, 1.0, -1e16]).unwrap();
        assert_close(cancellation.sum, 1.0);
        assert_close(cancellation.mean, 1.0 / 3.0);
    }

    #[test]
    fn statistics_reject_every_invalid_dataset_class() {
        for (numbers, code) in [
            (vec![], StatisticsErrorCode::EmptyDataset),
            (vec![1.0], StatisticsErrorCode::SampleTooSmall),
            (vec![f64::NAN, 1.0], StatisticsErrorCode::NonFiniteInput),
            (
                vec![f64::INFINITY, 1.0],
                StatisticsErrorCode::NonFiniteInput,
            ),
            (
                vec![f64::MAX, f64::MAX],
                StatisticsErrorCode::NonFiniteResult,
            ),
        ] {
            assert_eq!(summarize_statistics(numbers).unwrap_err().code, code);
        }
        assert!(matches!(
            summarize_statistics(vec![1.0; MAX_STATISTICS_VALUES + 1])
                .unwrap_err()
                .code,
            StatisticsErrorCode::TooManyValues
        ));
        assert_eq!(
            summarize_statistics(vec![1.0; MAX_STATISTICS_VALUES])
                .unwrap()
                .count,
            MAX_STATISTICS_VALUES
        );
    }
}
