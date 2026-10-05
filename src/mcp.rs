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
    /// Between 2 and 10,000 finite numbers; y values when x is supplied.
    #[schemars(length(min = 2, max = 10_000))]
    pub numbers: Vec<f64>,
    /// x values paired with numbers as y values, requesting a least-squares line fit.
    pub x: Option<Vec<f64>>,
    /// A line to compare with the x values and y values; requires x.
    pub reference_line: Option<ReferenceLine>,
}

/// A supplied straight line to compare with the data.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLine {
    /// Slope of the reference line.
    pub slope: f64,
    /// Intercept of the reference line.
    pub intercept: f64,
}

/// One input point and its signed residual (y minus predicted).
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct LineResidual {
    /// Input x value.
    pub x: f64,
    /// Input y value.
    pub y: f64,
    /// Line prediction at x.
    pub predicted: f64,
    /// y minus predicted.
    pub residual: f64,
}

/// Residual statistics for a straight line evaluated at the input x values.
#[derive(Debug, Serialize, JsonSchema)]
pub struct LineStatistics {
    /// Line slope.
    pub slope: f64,
    /// Line intercept.
    pub intercept: f64,
    /// One minus RSS divided by centred y sum of squares; null for constant y.
    pub r_squared: Option<f64>,
    /// Sum of squared residuals.
    pub residual_sum_of_squares: f64,
    /// Square root of RSS divided by the point count.
    pub rmse: f64,
    /// Square root of RSS divided by n-2; null for two points.
    pub residual_standard_error: Option<f64>,
    /// Point with largest absolute residual; the first point wins ties.
    pub largest_residual: LineResidual,
    /// Residuals in input order, omitted above 100 points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residuals: Option<Vec<LineResidual>>,
}

/// Ordinary least-squares fit of the y values on the x values.
#[derive(Debug, Serialize, JsonSchema)]
pub struct LinearFit {
    /// Fitted line coefficients and residual statistics.
    #[serde(flatten)]
    pub statistics: LineStatistics,
    /// Statistics for the supplied reference line, omitted when not requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<LineStatistics>,
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
    /// Least-squares line fit, omitted unless x values were supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear_fit: Option<LinearFit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatisticsErrorCode {
    EmptyDataset,
    SampleTooSmall,
    TooManyValues,
    NonFiniteInput,
    NonFiniteResult,
    LengthMismatch,
    ConstantX,
    ReferenceWithoutX,
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
            LengthMismatch => (
                "The x values and y values have different lengths.",
                "Give exactly one x value for each y value, in the same order.",
            ),
            ConstantX => (
                "All x values are equal.",
                "A line needs at least two different x values.",
            ),
            ReferenceWithoutX => (
                "A reference line requires x values.",
                "Pass x values with the reference line, or omit it.",
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

fn summarize_statistics(request: StatisticsRequest) -> Result<StatisticsResponse, StatisticsError> {
    let StatisticsRequest {
        numbers,
        x,
        reference_line,
    } = request;
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

    if x.as_ref()
        .is_some_and(|x| x.iter().any(|value| !value.is_finite()))
        || reference_line
            .as_ref()
            .is_some_and(|line| !line.slope.is_finite() || !line.intercept.is_finite())
    {
        return Err(StatisticsError::new(StatisticsErrorCode::NonFiniteInput));
    }
    if x.as_ref().is_some_and(|x| x.len() != numbers.len()) {
        return Err(StatisticsError::new(StatisticsErrorCode::LengthMismatch));
    }
    if x.as_ref()
        .is_some_and(|x| x.iter().all(|value| *value == x[0]))
    {
        return Err(StatisticsError::new(StatisticsErrorCode::ConstantX));
    }
    if reference_line.is_some() && x.is_none() {
        return Err(StatisticsError::new(StatisticsErrorCode::ReferenceWithoutX));
    }

    let count = numbers.len();
    let sum = compensated_sum(&numbers);
    let mean = sum / count as f64;
    let sample_standard_deviation = numbers.as_slice().std_dev();
    let min = numbers.as_slice().min();
    let max = numbers.as_slice().max();
    let linear_fit = x
        .as_ref()
        .map(|x| fit_line(x, &numbers, mean, reference_line))
        .transpose()?;
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
        linear_fit,
    })
}

fn require_finite(values: &[f64]) -> Result<(), StatisticsError> {
    if values.iter().any(|value| !value.is_finite()) {
        Err(StatisticsError::new(StatisticsErrorCode::NonFiniteResult))
    } else {
        Ok(())
    }
}

fn fit_line(
    x: &[f64],
    y: &[f64],
    y_mean: f64,
    reference: Option<ReferenceLine>,
) -> Result<LinearFit, StatisticsError> {
    // Preserve an exactly horizontal line even if division rounds the mean.
    let y_mean = if y.iter().all(|value| *value == y[0]) {
        y[0]
    } else {
        y_mean
    };
    let x_mean = compensated_sum(x) / x.len() as f64;
    let sxx = compensated_sum(&x.iter().map(|x| (x - x_mean).powi(2)).collect::<Vec<_>>());
    let sxy = compensated_sum(
        &x.iter()
            .zip(y)
            .map(|(x, y)| (x - x_mean) * (y - y_mean))
            .collect::<Vec<_>>(),
    );
    let syy = compensated_sum(&y.iter().map(|y| (y - y_mean).powi(2)).collect::<Vec<_>>());
    require_finite(&[x_mean, y_mean, sxx, sxy, syy])?;
    if sxx == 0.0 {
        return Err(StatisticsError::new(StatisticsErrorCode::ConstantX));
    }
    let slope = sxy / sxx;
    let intercept = y_mean - slope * x_mean;
    let statistics = line_statistics(x, y, slope, intercept, syy)?;
    let reference = reference
        .map(|line| line_statistics(x, y, line.slope, line.intercept, syy))
        .transpose()?;
    Ok(LinearFit {
        statistics,
        reference,
    })
}

fn line_statistics(
    x: &[f64],
    y: &[f64],
    slope: f64,
    intercept: f64,
    syy: f64,
) -> Result<LineStatistics, StatisticsError> {
    require_finite(&[slope, intercept])?;
    let mut residuals = Vec::with_capacity(x.len());
    for (&x, &y) in x.iter().zip(y) {
        let predicted = slope * x + intercept;
        let residual = y - predicted;
        require_finite(&[predicted, residual])?;
        residuals.push(LineResidual {
            x,
            y,
            predicted,
            residual,
        });
    }
    let mut largest_residual = &residuals[0];
    for point in &residuals[1..] {
        if point.residual.abs() > largest_residual.residual.abs() {
            largest_residual = point;
        }
    }
    let largest_residual = largest_residual.clone();
    let rss = compensated_sum(
        &residuals
            .iter()
            .map(|point| point.residual.powi(2))
            .collect::<Vec<_>>(),
    );
    let r_squared = (syy != 0.0).then(|| 1.0 - rss / syy);
    let rmse = (rss / x.len() as f64).sqrt();
    let residual_standard_error = (x.len() > 2).then(|| (rss / (x.len() - 2) as f64).sqrt());
    require_finite(&[rss, rmse])?;
    require_finite(
        &r_squared
            .into_iter()
            .chain(residual_standard_error)
            .collect::<Vec<_>>(),
    )?;
    Ok(LineStatistics {
        slope,
        intercept,
        r_squared,
        residual_sum_of_squares: rss,
        rmse,
        residual_standard_error,
        largest_residual,
        residuals: (x.len() <= 100).then_some(residuals),
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
        description = "Return count, compensated sum, mean, median, sample standard deviation (n-1), minimum, and maximum for 2 to 10,000 finite numbers. Pass x values too to get a least-squares line fit (slope, intercept, R², residuals), and a reference_line to compare another line with the data."
    )]
    fn statistics(
        &self,
        Parameters(request): Parameters<StatisticsRequest>,
    ) -> Result<Json<StatisticsResponse>, Json<StatisticsError>> {
        summarize_statistics(request).map(Json).map_err(Json)
    }
}

#[tool_handler]
impl ServerHandler for MathMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("math-4-llms", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Call calculate for one strict scalar expression or statistics for one numeric dataset, optionally with x values for a least-squares line fit and reference_line to compare another line with the data. Inspect ok before using a result.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summarize_statistics(numbers: Vec<f64>) -> Result<StatisticsResponse, StatisticsError> {
        super::summarize_statistics(StatisticsRequest {
            numbers,
            x: None,
            reference_line: None,
        })
    }

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
    fn fit_request(
        x: Vec<f64>,
        numbers: Vec<f64>,
        reference_line: Option<ReferenceLine>,
    ) -> StatisticsRequest {
        StatisticsRequest {
            numbers,
            x: Some(x),
            reference_line,
        }
    }

    fn assert_relative(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
            "{actual} != {expected}"
        );
    }

    #[test]
    fn sensor_fit_and_reference_match_real_case() {
        let response = super::summarize_statistics(fit_request(
            vec![0.0, 10.0, 25.0, 50.0, 100.0, 200.0],
            vec![812.0, 1490.0, 2510.0, 4205.0, 7588.0, 14102.0],
            Some(ReferenceLine {
                slope: 69.2,
                intercept: 900.0,
            }),
        ))
        .unwrap();
        let fit = response.linear_fit.unwrap();
        let stats = fit.statistics;
        assert_relative(stats.slope, 66.50346238130022);
        assert_relative(stats.intercept, 850.5278305332359);
        assert_relative(stats.r_squared.unwrap(), 0.999896795208191);
        assert_relative(stats.residual_sum_of_squares, 13019.53308984659);
        assert_relative(stats.rmse, 46.5824235269173);
        assert_relative(stats.residual_standard_error.unwrap(), 57.051584311582864);
        assert_eq!(stats.largest_residual.x, 100.0);
        assert_relative(
            stats.largest_residual.residual,
            7588.0 - (stats.slope * 100.0 + stats.intercept),
        );
        assert!((stats.largest_residual.residual - 87.13).abs() < 0.005);
        let points = stats.residuals.unwrap();
        assert_eq!(points.len(), 6);
        assert_eq!(points[0].x, 0.0);
        assert_eq!(points[5].x, 200.0);
        let reference = fit.reference.unwrap();
        assert_relative(reference.residual_sum_of_squares, 517441.0);
        assert_relative(reference.r_squared.unwrap(), 0.9958982868041496);
        assert_eq!(reference.largest_residual.x, 200.0);
        assert_close(reference.largest_residual.residual, -638.0);
        assert_eq!(reference.residuals.unwrap()[0].predicted, 900.0);
    }

    #[test]
    fn perfect_line_two_points_constant_y_and_timestamp_precision() {
        let fit = super::summarize_statistics(fit_request(
            (0..10).map(f64::from).collect(),
            (0..10).map(|x| f64::from(2 * x + 1)).collect(),
            None,
        ))
        .unwrap()
        .linear_fit
        .unwrap();
        assert_eq!(fit.statistics.slope, 2.0);
        assert_eq!(fit.statistics.intercept, 1.0);
        assert_eq!(fit.statistics.residual_sum_of_squares, 0.0);
        assert_eq!(fit.statistics.r_squared, Some(1.0));
        assert_eq!(fit.statistics.largest_residual.x, 0.0);
        assert!(fit.reference.is_none());

        let two = super::summarize_statistics(fit_request(
            vec![1.0, 3.0],
            vec![4.0, 8.0],
            Some(ReferenceLine {
                slope: 0.0,
                intercept: 0.0,
            }),
        ))
        .unwrap();
        let value = serde_json::to_value(two).unwrap();
        assert_eq!(value["linear_fit"]["residual_sum_of_squares"], 0.0);
        assert!(value["linear_fit"]["residual_standard_error"].is_null());
        assert!(value["linear_fit"]["reference"]["residual_standard_error"].is_null());

        for y in [7.0, 0.1] {
            let constant = super::summarize_statistics(fit_request(
                vec![0.0, 1.0, 2.0],
                vec![y; 3],
                Some(ReferenceLine {
                    slope: 1.0,
                    intercept: 0.0,
                }),
            ))
            .unwrap();
            let fit = constant.linear_fit.as_ref().unwrap();
            assert_eq!(fit.statistics.slope, 0.0);
            assert_eq!(fit.statistics.intercept, y);
            assert_eq!(fit.statistics.residual_sum_of_squares, 0.0);
            let value = serde_json::to_value(constant).unwrap();
            assert!(value["linear_fit"]["r_squared"].is_null());
            assert!(value["linear_fit"]["reference"]["r_squared"].is_null());
        }

        let x: Vec<_> = (0..100).map(|i| 1e9 + f64::from(i)).collect();
        let y = x.iter().map(|x| 3.0 * x + 5.0).collect();
        let fit = super::summarize_statistics(fit_request(x, y, None))
            .unwrap()
            .linear_fit
            .unwrap();
        assert_relative(fit.statistics.slope, 3.0);
        assert_close(fit.statistics.intercept, 5.0);
    }

    #[test]
    fn residual_output_is_bounded_for_fit_and_reference() {
        for n in [100, 101, 10_000] {
            let x: Vec<_> = (0..n).map(f64::from).collect();
            let result = super::summarize_statistics(fit_request(
                x.clone(),
                x,
                Some(ReferenceLine {
                    slope: 0.0,
                    intercept: 0.0,
                }),
            ))
            .unwrap();
            let value = serde_json::to_value(result).unwrap();
            for stats in [&value["linear_fit"], &value["linear_fit"]["reference"]] {
                assert!(stats["largest_residual"].is_object());
                if n <= 100 {
                    assert_eq!(stats["residuals"].as_array().unwrap().len(), n as usize);
                } else {
                    assert!(stats.get("residuals").is_none());
                }
            }
        }
    }

    #[test]
    fn fit_validation_and_overflow_errors() {
        use StatisticsErrorCode::*;
        let line = || {
            Some(ReferenceLine {
                slope: 1.0,
                intercept: 0.0,
            })
        };
        let cases = [
            (fit_request(vec![f64::NAN], vec![], line()), EmptyDataset),
            (
                fit_request(vec![f64::NAN], vec![1.0], line()),
                SampleTooSmall,
            ),
            (
                fit_request(vec![], vec![1.0; 10_001], line()),
                TooManyValues,
            ),
            (
                fit_request(vec![1.0], vec![f64::NAN, 1.0], line()),
                NonFiniteInput,
            ),
            (
                fit_request(vec![f64::NAN], vec![1.0, 2.0], None),
                NonFiniteInput,
            ),
            (
                fit_request(vec![0.0, f64::INFINITY], vec![1.0, 2.0], None),
                NonFiniteInput,
            ),
            (
                fit_request(
                    vec![0.0],
                    vec![1.0, 2.0],
                    Some(ReferenceLine {
                        slope: f64::NAN,
                        intercept: 0.0,
                    }),
                ),
                NonFiniteInput,
            ),
            (
                fit_request(
                    vec![0.0, 1.0],
                    vec![1.0, 2.0],
                    Some(ReferenceLine {
                        slope: 1.0,
                        intercept: f64::NAN,
                    }),
                ),
                NonFiniteInput,
            ),
            (
                fit_request(
                    vec![0.0, 1.0],
                    vec![1.0, 2.0],
                    Some(ReferenceLine {
                        slope: f64::INFINITY,
                        intercept: 0.0,
                    }),
                ),
                NonFiniteInput,
            ),
            (fit_request(vec![0.0], vec![1.0, 2.0], None), LengthMismatch),
            (fit_request(vec![], vec![1.0, 2.0], None), LengthMismatch),
            (fit_request(vec![1.0; 2], vec![1.0, 2.0], None), ConstantX),
            (
                StatisticsRequest {
                    numbers: vec![1.0, 2.0],
                    x: None,
                    reference_line: line(),
                },
                ReferenceWithoutX,
            ),
            (
                StatisticsRequest {
                    numbers: vec![1.0, 2.0],
                    x: None,
                    reference_line: Some(ReferenceLine {
                        slope: f64::NAN,
                        intercept: 0.0,
                    }),
                },
                NonFiniteInput,
            ),
            (
                fit_request(vec![-1e200, 1e200], vec![1.0, 2.0], None),
                NonFiniteResult,
            ),
            (
                fit_request(
                    vec![0.0, 1.0],
                    vec![1.0, 2.0],
                    Some(ReferenceLine {
                        slope: f64::MAX,
                        intercept: f64::MAX,
                    }),
                ),
                NonFiniteResult,
            ),
            (
                fit_request(
                    vec![0.0, 1.0],
                    vec![1.0, 2.0],
                    Some(ReferenceLine {
                        slope: 1e200,
                        intercept: 0.0,
                    }),
                ),
                NonFiniteResult,
            ),
            (
                fit_request(
                    vec![0.0, 1.0],
                    vec![1.0, 1.0 + f64::EPSILON],
                    Some(ReferenceLine {
                        slope: 1e150,
                        intercept: 0.0,
                    }),
                ),
                NonFiniteResult,
            ),
        ];
        for (request, code) in cases {
            let error = super::summarize_statistics(request).unwrap_err();
            assert_eq!(error.code, code);
            assert!(!error.message.is_empty());
            assert!(!error.hint.is_empty());
        }
    }

    #[test]
    fn numbers_only_serialization_is_byte_for_byte_unchanged() {
        let request: StatisticsRequest = serde_json::from_str(r#"{"numbers":[12,15,9]}"#).unwrap();
        let response = super::summarize_statistics(request).unwrap();
        assert_eq!(
            serde_json::to_string(&response).unwrap(),
            r#"{"schema_version":1,"ok":true,"count":3,"sum":36.0,"mean":12.0,"median":12.0,"sample_standard_deviation":3.0,"min":9.0,"max":15.0}"#
        );
        assert!(
            serde_json::from_str::<StatisticsRequest>(
                r#"{"numbers":[1,2],"reference_line":{"slope":1,"intercept":0,"extra":true}}"#
            )
            .is_err()
        );
    }
}
