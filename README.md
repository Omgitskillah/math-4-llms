# math-4-llms

`math-4-llms` is a small Rust library and CLI for evaluating one strict,
real-valued mathematical expression. It wraps fend-core with bounded input,
explicit syntax validation, and stable versioned JSON errors designed for
callers that cannot safely interpret free-form calculator messages.

The tool does not execute shell commands, read files, repair malformed input,
or infer mathematical intent.

## Use the CLI

```sh
cargo run --locked --bin math-tool -- '12 / sqrt(36)'
```

```json
{"schema_version":1,"ok":true,"result":"2","approximate":false}
```

Invalid input returns the same envelope with `ok: false`, a stable error code,
a plain-language message, a correction hint, and an optional UTF-8 byte offset.
The process exits nonzero for errors.

## Use the MCP server

The optional `mcp` feature provides a local stdio server with two tools:
`calculate` evaluates strict scalar expressions, while `statistics` summarizes
numeric datasets and optionally fits a least-squares line and compares a reference
line. Both publish JSON input and output schemas and return versioned structured
responses.

```sh
cargo run --locked --features mcp --bin math-mcp
```

The process speaks MCP over stdin/stdout, so launch it from an MCP client rather
than typing into it interactively. To inspect it with the official MCP Inspector:

```sh
npx @modelcontextprotocol/inspector cargo run -- --locked --features mcp --bin math-mcp
```

In Inspector, open **Tools** and select `calculate` or `statistics`.
Node.js 22.19.0 or newer is required by the current Inspector release. See the
[MCP server guide](docs/mcp-server.md) for the complete contracts, errors,
security boundaries, and an alternative prebuilt-binary command.

### Statistics tool

The `statistics` MCP tool is available when the `mcp` feature is enabled. It
accepts 2 to 10,000 finite numbers:

```json
{"numbers":[12,15,9]}
```

It returns count, compensated sum, mean, median, sample standard deviation
(using the `n-1` denominator), minimum, and maximum. For this example, the count
is 3, sum is 36, mean and median are 12, sample standard deviation is 3,
minimum is 9, and maximum is 15.

Empty or singleton datasets, oversized datasets, non-finite inputs, and
non-finite computed results are rejected. Statistics is exposed through MCP;
the `math-tool` CLI evaluates scalar expressions.

## Use the library

```rust
use math_4_llms::{evaluate, evaluate_response};

assert_eq!(evaluate("2 + 3 * 4")?, "14");
let response = evaluate_response("(2 + 3))");
assert!(!response.ok);
# Ok::<(), math_4_llms::MathError>(())
```

The public entry points are:

- `evaluate(&str) -> Result<String, MathError>`
- `evaluate_response(&str) -> MathResponse`
- `ErrorCode`, `MathError`, and `MathResponse` for structured handling

See the [v1 contract](docs/contract.md) for the accepted grammar, all error
codes, precedence, resource limits, and model-caller guidance.

## Test and inspect behavior

```sh
cargo test --locked --workspace --all-features
cargo run --locked --example research_math
cargo run --locked --example model_inputs
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

The research corpus contains ten representative calculations. The model-input
corpus contains 42 deliberately messy expressions, supplemented by generated
whitespace, nesting, byte-boundary, timeout, CLI, and serialization tests.

Coverage requires nightly Rust, `llvm-tools-preview`, and `cargo-llvm-cov`:

```sh
rustup toolchain install nightly --component llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo +nightly llvm-cov --branch --locked --workspace --all-features
```

GitHub Actions runs formatting, tests, Clippy, and coverage. It enforces at
least 85% line coverage and branch coverage strictly above 80%, then uploads
the text, JSON, and HTML reports.

## Documentation

- [Stable v1 contract](docs/contract.md)
- [MCP server and Inspector guide](docs/mcp-server.md)
- [Research-engine comparison](docs/research-comparison.md)
- [Historical permissive-input characterization](docs/model-input-robustness.md)
- [Strict implementation rerun](docs/test-rerun-comparison.md)
- [Development log](docs/development-log.md)

## Limitations

Evaluation has a 1024-byte input limit, nesting is capped at 32 levels, and the
fend interrupt deadline is cooperative rather than a hard CPU or memory sandbox.
Successful parsing proves neither mathematical intent nor research correctness.
See the contract for the complete supported subset and recovery guidance.

This repository was extracted from Remote Notes commit `1e299d2` on 2026-10-02.
No project license has been declared yet; fend-core 1.5.8 and statrs 0.19.1
declare MIT for their own code.
