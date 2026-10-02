# Model-output robustness report

**Historical baseline:** the permissive behaviors below were recorded before
strict validation. The [v1 contract and retest](contract.md) describe
the current stable JSON codes and rejected ambiguous inputs. The replay command
now prints v1 responses; baseline outputs below remain for comparison.

Date: 2026-10-01. Engine: fend-core 1.5.8. Subject: the existing
`evaluate(&str)` wrapper before strict validation was introduced.

## Purpose and method

We generated 42 synthetic examples of formatting and expression mistakes a
smaller model might emit. These are hand-designed cases, not samples from a
particular model and not a statistical estimate of model reliability.

Every input was passed unchanged to the actual Rust wrapper. The replay command
prints JSON lines with escaped input and the complete observed success/error:

```sh
cargo run --locked --example model_inputs
cargo test --locked --test model_inputs
```

The corpus is `tests/model-inputs.json`. Regression tests lock down
all successful output strings and which inputs fail. Error-message wording is
recorded by the replay but is not frozen in assertions. Additional deterministic
tests generate 125 whitespace variants, check balanced nesting at depths 1–32,
and exercise Unicode byte limits. No evaluator behavior was changed.

## Results from the 42-case corpus

Inputs below use JSON-style escaping where necessary. Code quotation in the
input column is presentation only unless explicitly described as input quotes.

| Case | Input | Observed result |
| --- | --- | --- |
| Spaces | `   2   +   3 * 4   ` | `14` |
| Tabs | `\t2\t+\t3 * 4\t` | `14` |
| Newlines | `2 +\n3 *\r\n4` | `14` |
| Nonbreaking spaces | `2\u00a0+\u00a03` | `5` |
| Zero-width space | `2\u200b+3` | Error: unexpected character |
| Redundant parentheses | `(((2 + 3)))` | `5` |
| Extra closing parenthesis | `(2 + 3))` | **`5` — accepted** |
| Missing closing parenthesis | `((2 + 3)` | **`5` — accepted** |
| Square brackets | `[2 + 3]` | Error: unexpected `[` |
| Curly brackets | `{2 + 3}` | Error: unexpected `{` |
| Mismatched brackets | `(2 + 3]` | Error: unexpected `]` |
| Empty parentheses | `()` | **Empty success string** |
| Inline Markdown code | Backticks surrounding `2 + 3` | Error: unexpected backtick |
| Markdown fence | Three backticks, `math\n2 + 3\n`, three backticks | Error: unexpected backtick |
| LaTeX delimiters | `$2 + 3$` | Error: exchange rates unavailable |
| LaTeX fraction | `\frac{1}{2}` | Error: unexpected `{` |
| Prose prefix | `The answer is 2 + 3` | Error: unknown identifier `The` |
| Prose suffix | `2 + 3 equals 5` | Error: unknown identifier `equals` |
| Trailing equals | `2 + 3 =` | Error: expected identifier |
| JSON wrapper | `{"expression":"2+3"}` | Error: unexpected `{` |
| Quoted expression | `"2+3"` including quotes | **`2+3` — text, not 5** |
| Unicode minus | `5 − 2` | `3` |
| Unicode multiplication | `2 × 3` | `6` |
| Unicode division | `6 ÷ 2` | `3` |
| Decimal comma | `1,5 + 2` | **`17` — not decimal-comma 3.5** |
| Thousands separator | `1,000 + 2` | `1002` |
| Split number | `1 2 + 3` | Error: unexpected input |
| Repeated plus | `2 ++ 3` | `5` |
| Double minus | `2 -- 3` | `5` |
| Double star | `2 ** 3` | `8` — exponentiation |
| Implicit multiplication | `2(3 + 4)` | `14` |
| Comment-like suffix | `2+3 // answer` | Error: unknown identifier `answer` |
| Multiple statements | `2+3; 9` | **`9` — last result** |
| Assignment | `x = 3; x + 2` | `5` |
| Units | `2 m + 3 m` | `5 m` |
| Complex value | `sqrt(-1)` | `approx. 0 + i` |
| Shell-like syntax | `$(echo 5)` | Error: exchange rates unavailable |
| NUL byte | `2\u0000+3` | Error: unexpected character |
| Empty input | Empty string | Wrapper error: must not be blank |
| Whitespace only | ` \t\r\n` | Wrapper error: must not be blank |
| Scientific notation | `1e-3 + 2e-3` | `0.003` |
| Power chain | `2^3^2` | `512` |

These outcomes characterize fend's language, not whether the model intended
the computed meaning. A successful parse is not proof that the requested
research calculation was represented correctly.

## Limitations exposed

1. **Validation is permissive.** Unbalanced parentheses can succeed; empty
   parentheses can produce an empty successful result. The earlier claim that
   this tool validates mathematical syntax must be understood as acceptance by
   fend's language, not strict balanced-expression validation.
2. **Numeric success is not enforced.** Strings, units, complex values,
   assignments, and multiple statements are supported. A caller must not assume
   that `Ok(String)` always contains a scalar number or a single calculation.
3. **Locale mistakes can silently change the result.** The tested decimal-comma
   input becomes 17. Use a period for decimal fractions and omit digit-grouping
   commas in model-generated expressions.
4. **Formatting support is selective.** Ordinary spaces, tabs, CRLF, and tested
   nonbreaking spaces work. Zero-width spaces, Markdown fences, JSON envelopes,
   and LaTeX are not automatically stripped or converted. Dollar signs are
   interpreted by the calculator language, hence the confusing currency error.
5. **Operator meanings matter.** Double minus and double star are valid syntax.
   Missing multiplication signs may be accepted. This can mask generation
   mistakes rather than detect them.
6. **Resource controls are incomplete.** There is a 1024-byte input limit and a
   cooperative two-second fend deadline, not a hard CPU/memory boundary. Unicode
   consumes multiple bytes. Blank input is rejected before length checking.
   Nested expressions were tested only through depth 32; these tests do not
   prove safety for all nesting or expensive arithmetic within the byte limit.
7. **Result text is structured only by convention.** Approximation markers,
   units, complex forms, and empty output need explicit handling. Do not strip
   `approx.` or round a value silently when writing it into a research note.

No shell is invoked by the library. The shell-like case tests parsing only;
it is not a comprehensive security audit. No model was connected, no random
fuzzing or resource-exhaustion benchmark was run, and no vault content changed.

## Suggested contract for a smaller model

Until a strict mode exists, instruct the model to send exactly one expression,
without quotes, Markdown, prose, JSON wrapping, assignments, or semicolons.
Use balanced parentheses, explicit `*`, period decimals, and unambiguous powers
such as `2^(3^2)`. Supply the expression as a tool string field; the application
should pass the decoded field value directly to `evaluate`.

Prompt instructions alone do not enforce this contract. A future strict mode
should validate bracket balance, enforce an explicitly chosen expression subset,
and distinguish scalar results from other successful outputs. Such a change
must decide whether units and complex arithmetic remain allowed. Avoid repairing
brackets or deleting punctuation automatically: that can alter mathematical
meaning. This task adds evidence and regression tests, not that strict mode.
