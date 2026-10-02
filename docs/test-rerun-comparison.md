# Calculator test rerun and comparison

Date: 2026-10-01. Original branch: `feat/math-for-models`. Engine: fend-core 1.5.8.

All 54 workspace tests passed. Both expression examples were rerun against the
current strict v1 implementation. No behavior changed relative to the previous
strict-validation run, and no implementation changes were needed.

Historical comparisons use the recorded [research comparison](research-comparison.md)
and [permissive model-input baseline](model-input-robustness.md); the old
implementation and removed evalexpr option were not rebuilt or rerun.

## Clean research expressions

All ten outputs are unchanged from the previous fend runs, including approximation
markers. Automated numerical reference checks pass with absolute tolerance 1e-9.
Exact expressions are in `tests/research-expressions.json`.

| Calculation | Current output | Comparison |
| --- | --- | --- |
| Arithmetic mean | `2.5` | Unchanged |
| Sample variance | `4` | Unchanged |
| Standard error | `2` | Unchanged |
| 95% normal CI upper bound | `105.88` | Unchanged |
| Exponential decay | `approx. 36.7879441171` | Unchanged |
| Half-life | `approx. 6.9314718056` | Unchanged |
| pH | `approx. 5` | Unchanged |
| Bayesian posterior probability | `approx. 0.1610169492` | Unchanged |
| Weighted mean | `23` | Unchanged |
| Euclidean distance | `5` | Unchanged |

## Dirty/model-style expressions

The same 42-case corpus now produces **9 successes and 33 explicit rejections**,
versus 24 successes and 18 rejections in the original permissive baseline.
All 42 outcomes match the immediately previous strict run. A rejection counts as
a passing test when rejection is the expected contract behavior.

### Fifteen historical successes now rejected

| Case | Historical output | Current error code |
| --- | --- | --- |
| Extra closing parenthesis | `5` | `UNBALANCED_PARENTHESES` |
| Missing closing parenthesis | `5` | `UNBALANCED_PARENTHESES` |
| Empty parentheses | Empty success string | `INVALID_SYNTAX` |
| Quoted expression | Text `2+3` | `UNSUPPORTED_FORMATTING` |
| Unicode minus | `3` | `UNSUPPORTED_CHARACTER` |
| Unicode multiplication | `6` | `UNSUPPORTED_CHARACTER` |
| Unicode division | `3` | `UNSUPPORTED_CHARACTER` |
| Decimal comma | `17` | `AMBIGUOUS_COMMA` |
| Thousands separator | `1002` | `AMBIGUOUS_COMMA` |
| Double star | `8` | `INVALID_SYNTAX` |
| Implicit multiplication | `14` | `INVALID_SYNTAX` |
| Multiple statements | `9` | `MULTIPLE_STATEMENTS` |
| Assignment (`x = 3; x + 2`) | `5` | `UNKNOWN_IDENTIFIER` |
| Units | `5 m` | `UNKNOWN_IDENTIFIER` |
| Complex value | `approx. 0 + i` | `NON_SCALAR_RESULT` |

These are intentional restrictions, not fifteen arithmetic bugs fixed. Some
inputs are valid native fend syntax but outside our scalar-expression contract.
The assignment example reports the unknown `x` before reaching `=`; callers
should handle the first reported error rather than assume an error priority.

### Nine retained successes

Spaces, tabs, and newlines still return `14`; nonbreaking spaces and redundant
parentheses still return `5`. Repeated plus (`2 ++ 3`) and double minus
(`2 -- 3`) still return `5`, because unary signs are valid. Scientific notation
still returns `0.003`, and the power chain `2^3^2` still returns `512`.
All nine result strings are unchanged from the permissive baseline.

### Eighteen retained rejections, now with stable codes

| Cases | Current error code |
| --- | --- |
| Zero-width space; NUL byte (2) | `UNSUPPORTED_CHARACTER` |
| Square, curly, mismatched brackets; inline code; Markdown fence; LaTeX delimiters; LaTeX fraction; JSON wrapper; shell-like syntax (9) | `UNSUPPORTED_FORMATTING` |
| Prose prefix; prose suffix; comment-like suffix (3) | `UNKNOWN_IDENTIFIER` |
| Trailing equals (1) | `ASSIGNMENT_NOT_ALLOWED` |
| Split number (1) | `INVALID_SYNTAX` |
| Empty input; whitespace only (2) | `EMPTY_INPUT` |

No formerly rejected corpus input became successful. Exact inputs remain in
`tests/model-inputs.json`; the replay prints complete JSON responses.

## Additional regression checks and coverage

- All 125 generated whitespace variants passed, unchanged.
- Balanced nesting at depths 1–32 passed; depth 33 was rejected as expected.
- Byte-length boundaries, all 15 serialized code names, response envelopes,
  byte positions, approximation metadata, CLI behavior, and deterministic
  cooperative-timeout handling passed.
- Formatting and Clippy with warnings denied passed.
- Coverage reran all 54 tests successfully and passed the 85% line threshold.

| Workspace metric | Previous strict run | This rerun |
| --- | --- | --- |
| Regions | 79.12% | 79.12% |
| Functions | 77.05% | 77.05% |
| Lines | 89.42% | 89.42% |
| Branches | 78.20% | 78.20% |

Current calculator-specific coverage: `math.rs` 98.45% lines / 66.67% branches;
`math/validation.rs` 99.32% lines / 97.83% branches; calculator CLI 100% lines
and branches. Passing tests do not imply every branch is covered.

## Reproduce

Run from the repository root:

```sh
cargo test --locked --workspace
cargo run --locked --example research_math
cargo run --locked --example model_inputs
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo +nightly llvm-cov --branch --locked --workspace --fail-under-lines 85
```

All commands above completed successfully in this rerun. Coverage requires the
existing nightly/llvm-tools-preview and cargo-llvm-cov installation.

## Remaining limitations

This is deterministic regression testing, not actual small-model sampling,
random fuzzing, or a hard resource-sandbox audit. Accepted syntax cannot prove
the model expressed the user's intended calculation. Engine-domain failures
still use generic `EVALUATION_FAILED`; only final output is checked for a real
scalar, so complex intermediate values are not categorically prohibited.
See the [v1 contract](contract.md) for input limits and recovery hints.
