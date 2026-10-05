# MCP server and Inspector guide

The `math-mcp` binary exposes two Model Context Protocol tools
over the local standard-input/standard-output transport. It does not open a
network port, authenticate users, access notes, or read and write files.

## Scalar calculation contract

Tool name: `calculate`

Input:

```json
{"expression":"2 + 3 * 4"}
```

Successful structured output:

```json
{
  "schema_version": 1,
  "ok": true,
  "result": "14",
  "approximate": false
}
```

An invalid mathematical expression produces an MCP tool error and retains the
calculator's structured correction response. For example, `(2 + 3))` returns
`UNBALANCED_PARENTHESES`, its stable message and hint, and `ok: false`. A missing
or wrongly typed `expression` is rejected as an MCP parameter error before the
calculator runs.

The server also includes a JSON text copy of each structured calculator response
for clients that do not yet consume MCP structured output.

Nullable output fields use explicit `anyOf` branches with one JSON Schema type
per branch. This avoids array-valued `type` declarations that are valid JSON
Schema but are rejected or weakened by some model-provider schema adapters.

## Statistics contract

Tool name: `statistics`

Input:

```json
{"numbers":[12,15,9]}
```

Successful structured output:

```json
{
  "schema_version": 1,
  "ok": true,
  "count": 3,
  "sum": 36.0,
  "mean": 12.0,
  "median": 12.0,
  "sample_standard_deviation": 3.0,
  "min": 9.0,
  "max": 15.0
}
```

The standard deviation is the sample statistic with Bessel's `n-1` correction.
All existing summary fields describe `numbers`; calls with only `numbers` return
exactly the output above, without a `linear_fit` key. `schema_version` remains 1.

| Input field | Contract |
| --- | --- |
| `numbers` | Required, 2 to 10,000 finite numbers; y values when `x` is supplied. |
| `x` | Optional finite x values, one for each y value in the same order; requests a fit. |
| `reference_line` | Optional object with finite `slope` and `intercept`; requires `x`. |

For example, fit sensor readings and compare an eyeballed line with one call:

```json
{
  "numbers": [812, 1490, 2510, 4205, 7588, 14102],
  "x": [0, 10, 25, 50, 100, 200],
  "reference_line": {"slope": 69.2, "intercept": 900}
}
```

The response includes the usual summary and a `linear_fit` object. Its slope is
approximately 66.50346238130022, intercept 850.5278305332359, R²
0.999896795208191, and RSS 13019.53308984659. The largest residual is at x = 100
(approximately +87.13). The reference line's RSS is 517441.0 and R² is
0.9958982868041496; its largest residual is at x = 200 (−638).

| `linear_fit` field | Meaning |
| --- | --- |
| `slope`, `intercept` | Ordinary least-squares line of y on x. |
| `residual_sum_of_squares` | RSS = sum of squared residuals, where residual = y − predicted. |
| `r_squared` | 1 − RSS / sum((y − mean(y))²); `null` when all y values are equal. |
| `rmse` | sqrt(RSS / n). |
| `residual_standard_error` | sqrt(RSS / (n − 2)); `null` for two points. |
| `largest_residual` | Object with `x`, `y`, `predicted`, `residual` for the largest absolute residual. The first point wins ties. |
| `residuals` | All residual objects in input order when n ≤ 25; omitted above 25. |
| `reference` | Present only with `reference_line`; the same fields evaluated using the supplied slope and intercept, without a nested `reference`. |

The fit uses centred two-pass sums with compensated summation, avoiding the
single-pass subtraction that loses precision for large x offsets such as
timestamps. Predictions use slope × x + intercept. All numeric results are
full-precision `f64` values, without rounding. Reference R² can be negative.
`largest_residual` is always present, even when `residuals` is omitted. Reference
statistics follow the same null and residual-size rules as the fit.

Validation runs in the following order before computing statistics (Sxx is
checked during the centred computation as well):

| Condition | Error code | Correction hint |
| --- | --- | --- |
| Empty `numbers` | `EMPTY_DATASET` | Provide at least two finite numbers for sample statistics. |
| One y value | `SAMPLE_TOO_SMALL` | Add another finite observation or do not request sample statistics. |
| More than 10,000 y values | `TOO_MANY_VALUES` | Split or summarize the dataset before calling this tool. |
| NaN or infinity in `numbers`, `x`, or reference coefficients | `NON_FINITE_INPUT` | Remove NaN and positive or negative infinity values. |
| Different x and y lengths | `LENGTH_MISMATCH` | Give exactly one x value for each y value, in the same order. |
| All x values equal (Sxx = 0) | `CONSTANT_X` | A line needs at least two different x values. |
| Reference line without x | `REFERENCE_WITHOUT_X` | Pass x values with the reference line, or omit it. |
| Any non-finite computed value | `NON_FINITE_RESULT` | Reduce the magnitude of the values or rescale the dataset. |

These errors return `schema_version: 1`, `ok: false`, `code`, a plain-language
`message`, and the correction `hint`, with MCP `isError: true`. Unknown fields
in the request or reference line and non-numeric values are MCP parameter errors.

## Test with MCP Inspector

The current Inspector requires Node.js 22.19.0 or newer. Confirm the local
version, then run Inspector from this repository's root:

```sh
node --version
npx @modelcontextprotocol/inspector cargo run -- --locked --features mcp --bin math-mcp
```

The `npx` command may download Inspector from npm the first time it runs. Once
the Inspector interface opens:

1. Connect to the supplied stdio server.
2. Open **Tools** and select `calculate`.
3. Enter `(1 + 2 + 3 + 4) / 4` in the `expression` field.
4. Run the tool and confirm that the result is `2.5` with `ok: true`.
5. Try `(2 + 3))` and confirm that the tool reports
   `UNBALANCED_PARENTHESES` with `isError: true`.
6. Select `statistics`, enter `[12, 15, 9]` in its `numbers` field, and confirm
   the response matches the statistics example above.

To avoid compiling every time, build once and point Inspector at the binary:

```sh
cargo build --locked --features mcp --bin math-mcp
npx @modelcontextprotocol/inspector ./target/debug/math-mcp
```

Inspector is a local development client in this setup. It does not make the
server remotely reachable and it does not add authentication. Any application
that can start this process under the current operating-system account can call
the tool; normal local process and account permissions are the security boundary.

## Automated verification

`tests/mcp_server.rs` starts the real binary and exercises the MCP lifecycle over
stdio. It verifies initialization, both tools and their published schemas,
successful and rejected calculations, statistics over varied datasets, the
maximum dataset boundary, every statistics error class, and malformed inputs:

```sh
cargo test --locked --all-features --test mcp_server
```
