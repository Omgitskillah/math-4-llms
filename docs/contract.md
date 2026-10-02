# Math tool contract — schema version 1

The default calculator now accepts one strict scalar expression. This supersedes
the permissive wrapper described in the original comparison and robustness
reports. Fend-core 1.5.8 remains the evaluation engine.

The [full test rerun comparison](test-rerun-comparison.md) records clean and
model-style results against both the historical baseline and this strict contract.

## Calling it

```sh
cargo run --locked --bin math-tool -- '12 / sqrt(36)'
```

```json
{"schema_version":1,"ok":true,"result":"2","approximate":false}
```

The CLI writes one JSON response to stdout for both calculation success and
failure, exiting 0 or 1 respectively. `--help` and `-h` return human-readable
usage instead. Missing/extra arguments return `INVALID_ARGUMENTS` as JSON.

The Rust API provides `evaluate(&str) -> Result<String, MathError>` and
`evaluate_response(&str) -> MathResponse`. Library errors expose the same code,
message, hint, and optional position as CLI errors.

## Input grammar

- Decimal numbers, scientific notation (`1e-3`), `+ - * / ^`, parentheses,
  and ordinary whitespace (including the tested nonbreaking spaces).
- Single-argument functions: `sqrt`, `exp`, `ln`, `log10`, `sin`, `cos`, `tan`,
  `abs`. Constants: `pi`, `e`. Names are case sensitive. Trigonometric inputs
  follow fend's default radian convention.
- Function parentheses and multiplication signs are required: `sqrt(9)` and
  `2 * (3 + 4)`. Powers follow fend's precedence: `-2^2` is -4 and `2^3^2` is
  512. Explicit grouping is preferable for generated expressions.
- One unary sign per operand is supported: `2 + -3`, `2 -- 3`, and `2 ++ 3`
  are valid. The last two mean subtraction of negative 3 and addition of positive
  3, respectively; the tool does not infer whether they were typing mistakes.
- Maximum 1024 UTF-8 bytes, maximum 32 nested parenthesis pairs. Input length is
  checked before blankness, so oversized whitespace returns `INPUT_TOO_LONG`.

Quotes, Markdown/LaTeX wrappers, JSON envelopes, commas, units, variables,
assignments, semicolons, implicit multiplication, and Unicode operator glyphs
are rejected. Input is never auto-repaired. The caller supplies the decoded
expression field, not a serialized tool-call object.

## Error format and stability policy

For `(2 + 3))`:

```json
{
  "schema_version": 1,
  "ok": false,
  "error": {
    "code": "UNBALANCED_PARENTHESES",
    "message": "Opening and closing parentheses do not match.",
    "hint": "Correct the parentheses explicitly; the tool will not add or remove them.",
    "position": 7
  }
}
```

Codes and envelope semantics are the machine contract. Within v1, do not rename
or repurpose existing codes. Additions must be documented; consumers should have
a fallback for an unfamiliar code. Breaking envelope/meaning changes require a
new schema version. Tests pin every serialized code name and corpus outcome.
Messages and hints may improve without a version change; do not parse them.

Positions are **zero-based UTF-8 byte offsets**, not character columns. A syntax
error at end-of-input may point to the byte length. A missing closing bracket
points to an unmatched opening bracket. `null` means there is no reliable local
position, including engine evaluation failures. Only one error is reported.
After size/blank checks, lexical checks run left to right, then unmatched opening
checks, then grammar checks. Consequently `x = 3` reports `UNKNOWN_IDENTIFIER`
before reaching the equals sign; `pi = 3` reports `ASSIGNMENT_NOT_ALLOWED`.

| Code | Meaning / correction |
| --- | --- |
| `EMPTY_INPUT` | Supply an expression. |
| `INPUT_TOO_LONG` | Shorten input; it is never truncated. |
| `UNSUPPORTED_FORMATTING` | Remove text wrappers; use parentheses for grouping. |
| `UNSUPPORTED_CHARACTER` | Remove invisible/control characters or use ASCII operators. |
| `AMBIGUOUS_COMMA` | Use period decimals and no digit grouping; multi-argument calls are unsupported. |
| `MULTIPLE_STATEMENTS` | Send calculations separately, without semicolons. |
| `ASSIGNMENT_NOT_ALLOWED` | Substitute values; remove the equation/assignment. |
| `UNBALANCED_PARENTHESES` | Correct the parentheses. |
| `NESTING_TOO_DEEP` | Reduce nesting below 33 levels. |
| `UNKNOWN_IDENTIFIER` | Use approved function/constant names; remove prose or units. |
| `INVALID_SYNTAX` | Correct operand/operator order or function call syntax. |
| `EVALUATION_FAILED` | Syntax passed but fend failed; check domains and denominators. |
| `EVALUATION_TIMEOUT` | Fend observed the cooperative deadline; simplify the calculation. |
| `NON_SCALAR_RESULT` | The returned value was not a supported real numeric literal. |
| `INVALID_ARGUMENTS` | CLI expects exactly one expression argument. |

We deliberately do not invent a `DIVISION_BY_ZERO` or `DOMAIN_ERROR` code by
matching dependency error strings. `1/0` and `ln(0)` both reliably return
`EVALUATION_FAILED`. Timeout classification uses our interrupt flag, not error
wording. The wrapper's validation codes are independent of fend's messages.

## Result interpretation

Successful results contain a numeric string and an `approximate` Boolean, with
no error field. `approximate` reflects fend's `approx. ` marker; the original
marker remains in the result, e.g. `approx. 0.6931471806` for `ln(2)`.
Do not treat `approximate: false` as a proof of numerical accuracy. Keep the
value as text rather than coercing arbitrary-precision results into a machine
float. Empty, complex, unit-bearing, and text outputs are rejected. Inputs with
complex intermediate values may still succeed if the final result is real;
the wrapper checks the final rendered value, not every intermediate domain.

## Retest of model-output pitfalls

The same 42-input corpus is now tested against explicit v1 codes or result
strings. The 125 whitespace combinations and nesting depths 1–32 still pass;
depth 33 is rejected. All ten research reference calculations remain covered.

| Input | Earlier behavior | Current result |
| --- | --- | --- |
| `(2 + 3))` / `((2 + 3)` | Returned 5 | `UNBALANCED_PARENTHESES` |
| `()` | Empty success | `INVALID_SYNTAX` |
| `"2+3"` including quotes | Returned text | `UNSUPPORTED_FORMATTING` |
| `1,5 + 2` | Returned 17 | `AMBIGUOUS_COMMA` |
| `$2 + 3$` | Exchange-rate error | `UNSUPPORTED_FORMATTING` |
| `2+3; 9` | Returned 9 | `MULTIPLE_STATEMENTS` |
| `2 m + 3 m` | Returned 5 m | `UNKNOWN_IDENTIFIER` |
| `sqrt(-1)` | Complex output | `NON_SCALAR_RESULT` |
| `2(3+4)` / `2 ** 3` | Accepted | `INVALID_SYNTAX` |
| `2 × 3` | Returned 6 | `UNSUPPORTED_CHARACTER`, use `*` |

Replay current JSON responses with
`cargo run --locked --example model_inputs`.

## Instructions to give a small model

> Send exactly one arithmetic expression using period decimals, explicit `*`,
> balanced parentheses, and the approved functions. Do not include prose,
> Markdown, assignments, units, or quotes around the expression's contents.
> Read `ok` before using a result. On failure, use `error.code` and `hint` to
> correct the expression without changing the intended math. If the intended
> value or operation is ambiguous, ask instead of guessing. Do not retry the
> same failed expression repeatedly. Preserve approximation markers in notes.

This reduces predictable mistakes; it cannot determine a model's mathematical
intent or guarantee correctness for every input. The deadline remains
cooperative with no hard memory/process sandbox. No actual model sampling or
exhaustive fuzzing was performed. This remains a local tool, not a remote service.
