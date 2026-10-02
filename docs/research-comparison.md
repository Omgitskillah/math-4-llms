# Research expression comparison: fend-core and evalexpr

**Current interface:** see the [strict v1 calculator contract](contract.md).
The CLI now returns JSON and the default validator restricts inputs to scalar
expressions. The engine comparison below is historical evidence.

Historical comparison executed on 2026-10-01, on Apple Silicon macOS, using
fend-core **1.5.8** and evalexpr **13.1.0**.

## Decision and current usage

Fend is now the sole calculation engine, as requested by the user. It matched
all ten research references within 1e-9 and handles fractional division without
changing integer literals. Evalexpr's implementation, dependency, and specific
tests have been removed. Its measured results remain below as decision history.

The current library signature is
`math_4_llms::evaluate(&str) -> Result<String, MathError>`.
Use these commands to calculate or replay the ten fend cases:

```sh
cargo run --locked --bin math-tool -- '12 / sqrt(36)'
cargo run --locked --example research_math
```

Fend's input-size limit, cooperative deadline, fresh contexts, and original
formatted results remain, with strict scalar input/output validation now added.
This is not a hard resource sandbox. The following comparison describes the earlier two-engine experiment;
its old CLI commands and evalexpr checks are no longer part of the tool.

## Historical comparison implementation

the original `vault_access::math::evaluate(Backend, &str)` API
offers two independent calculation options. `math-tool` exposes them through
the terminal. Neither option reads or edits Obsidian notes.

```sh
cargo run --bin math-tool -- fend '12 / sqrt(36)'
cargo run --bin math-tool -- evalexpr '12 / math::sqrt(36)'
cargo run --example compare_math
```

The comparison's machine-readable inputs and reference values live in
`tests/research-expressions.json`. The example prints actual
results and errors; automated tests independently check them against those
reference values using an absolute tolerance of 1e-9. This tolerance allows for
fend's default display rounding. This is a correctness/syntax comparison, not
a performance benchmark or proof of correctness for arbitrary expressions.

## Ten research expressions

These represent small calculations often used in statistics, experimental
science, and data analysis. They are synthetic examples, not study findings.

| # | Purpose | Expression | Reference result |
| --- | --- | --- | --- |
| 1 | Arithmetic mean | `(1 + 2 + 3 + 4) / 4` | 2.5 |
| 2 | Sample variance (n−1 denominator) | `((2 - 4)^2 + (4 - 4)^2 + (6 - 4)^2) / (3 - 1)` | 4 |
| 3 | Standard error (s / √n) | `12 / sqrt(36)` | 2 |
| 4 | 95% normal-approximation CI upper bound | `100 + 1.96 * 15 / sqrt(25)` | 105.88 |
| 5 | Exponential decay after five time units | `100 * exp(-0.2 * 5)` | 36.787944117144235 |
| 6 | Half-life for decay rate 0.1 | `ln(2) / 0.1` | 6.931471805599453 |
| 7 | pH, treating concentration as dimensionless relative to 1 mol/L | `-log10(0.00001)` | 5 |
| 8 | Bayesian posterior from sensitivity, prior, and false-positive rate | `(0.95 * 0.01) / (0.95 * 0.01 + 0.05 * 0.99)` | 0.16101694915254236 |
| 9 | Weighted mean | `(10 * 2 + 20 * 3 + 30 * 5) / (2 + 3 + 5)` | 23 |
| 10 | Euclidean distance in two dimensions | `sqrt((4 - 1)^2 + (6 - 2)^2)` | 5 |

Case 4 specifically uses a normal approximation, not a Student-t interval.
Closed-form references include 100/e for case 5, 10 ln(2) for case 6, and
19/118 for case 8. The chosen tolerance tests displayed numerical agreement;
it does not distinguish exact internal arithmetic from rounded approximations.

## Observed results

Every expression was first sent unchanged to both engines. A second evalexpr
pass used its native function names: `math::sqrt`, `math::exp`, `math::ln`, and
`math::log10`. No numeric literals or arithmetic operators were changed.

| # | fend result | evalexpr, identical input | evalexpr, native function names |
| --- | --- | --- | --- |
| 1 | `2.5` | `2` — numerical mismatch | `2` — numerical mismatch |
| 2 | `4` | `4` | `4` |
| 3 | `2` | Error: unbound `sqrt` | `2` |
| 4 | `105.88` | Error: unbound `sqrt` | `105.88` |
| 5 | `approx. 36.7879441171` | Error: unbound `exp` | `36.787944117144235` |
| 6 | `approx. 6.9314718056` | Error: unbound `ln` | `6.931471805599452` |
| 7 | `approx. 5` | Error: unbound `log10` | `5` |
| 8 | `approx. 0.1610169492` | `0.16101694915254236` | `0.16101694915254236` |
| 9 | `23` | `23` | `23` |
| 10 | `5` | Error: unbound `sqrt` | `5` |

- **fend:** 10/10 numerical matches within tolerance using the original inputs.
- **evalexpr, identical inputs:** 3/10 numerical matches, six function-name
  errors, and one integer-division mismatch.
- **evalexpr, native function names:** all ten evaluate successfully; 9/10
  match the research reference values. Changing case 1's divisor to `4.0`
  produces `2.5`, verified separately by a regression test.

The evalexpr result of 2 is its documented integer-division behavior, not a
parser failure. Function-name differences do not mean the functions are absent.
The corpus uses fend-compatible function names; the native-syntax pass is
necessary to avoid mistaking syntax compatibility for mathematical capability.

## Additional checks and implementation limits

- Verified precedence with `2 + 3 * 4`, `-2^2`, `(2^3)^2`, and `2^(3^2)`.
- `0.1 + 0.2` returns `0.3` with fend and `0.30000000000000004` with evalexpr.
- Both wrappers reject blank and over-1024-byte input; boundary-size input is
  tested. Malformed expressions, undefined functions, and integer division by
  zero return errors. Evalexpr nonnumeric and non-finite final results are
  rejected, including floating division by zero and a negative real square root.
- Every call gets a fresh context. Fend uses a cooperative two-second deadline;
  a deterministic interrupt test verifies cancellation without race timing.
  Neither wrapper provides a hard memory/process sandbox. Evalexpr has no
  execution deadline here. These are local experimental backends, not remotely
  exposed model tools.
- Native behavior is intentionally preserved. Fend may return units, complex
  values, or other formatted results; the wrapper does not enforce a numeric-only
  grammar for it. Evalexpr accepts only finite integer/float final results.
  We have not standardized function names, precision, assignments, or every
  intermediate-domain behavior across engines.
- Evalexpr explicitly builds its operator tree before evaluation. Fend performs
  parsing and evaluation through its combined API; there is no separate public
  validation operation in our wrapper.

## Recommendation

For research calculations embedded in notes, **fend is the stronger default
candidate in this small comparison**: familiar function names, rational
arithmetic, and fewer surprises for ordinary division. Preserve its `approx.`
marker in displayed results. Evalexpr remains useful for applications that want
typed integer/float semantics and expression trees, but a model-facing contract
must explain integer division and namespaced functions.

The downloaded manifests declare **MIT** for fend-core and **AGPL-3.0-only** for
evalexpr 13.1.0. Record this distinction when selecting a production dependency;
this comparison does not resolve distribution/licensing requirements.

Sources: [fend-core API](https://docs.rs/fend-core/1.5.8/fend_core/),
[evalexpr operators and functions](https://docs.rs/evalexpr/13.1.0/evalexpr/).
Observed outputs above come from the local run, not documentation examples.
