# Development log

This log is append-only. The calculator originated in the Remote Notes project;
its detailed experiment reports are retained beside this file.

## 2026-10-01 — Select and harden the evaluator

- Compared fend-core 1.5.8 with evalexpr 13.1.0 over ten research expressions.
  Fend matched all ten numerical references within absolute tolerance 1e-9 and
  was selected as the sole engine. Evalexpr was removed.
- Added a strict scalar grammar, 1024-byte and 32-level nesting limits, a
  cooperative two-second deadline, fresh evaluation contexts, and numeric-only
  final-result validation.
- Added the versioned JSON envelope and 15 stable error codes with messages,
  hints, and optional zero-based UTF-8 byte positions.
- Characterized 42 model-style inputs and generated 125 whitespace variants.
  Under strict validation, nine corpus inputs succeed and 33 are explicitly
  rejected; all ten clean research expressions remain unchanged.
- Added deterministic timeout, error serialization, approximation metadata,
  CLI, byte-boundary, nesting, function, constant, and operator tests.
- The original implementation and evidence were committed in Remote Notes as
  `1e299d2` (`feat: add stable math evaluation tool`).

## 2026-10-02 — Extract standalone repository

- Moved the calculator library, CLI, examples, corpora, tests, and dedicated
  documentation into `math-4-llms`.
- Renamed the Rust crate import from `vault_access::math` to `math_4_llms` while
  retaining `math-tool` as the CLI binary name.
- Added standalone formatting, test, Clippy, line-coverage, and branch-coverage
  GitHub checks.
- Remote Notes consumes this repository as a root Git submodule/path dependency and
  re-exports it as `vault_access::math` for source compatibility.
- Clean verification passed all 13 tests plus formatting and Clippy. Coverage
  passed at 99.31% lines and 96.55% branches, exceeding the workflow's 85% line
  and strictly-above-80% branch gates.

## 2026-10-02 — Add local MCP server

- Added the feature-gated `math-mcp` stdio server using the official Rust MCP
  SDK. Its sole `calculate_math` tool publishes input/output schemas and returns
  the existing versioned `MathResponse`; invalid calculations are marked as MCP
  tool errors while retaining their stable code, message, hint, and position.
- Kept the `mcp` feature disabled by default so library consumers do not inherit
  the MCP runtime. GitHub Actions now enables all features for tests, Clippy, and
  coverage so the optional server cannot be skipped in CI.
- Added an end-to-end process test that performs MCP initialization, lists the
  tool and schemas, calls a successful calculation, checks a structured
  `UNBALANCED_PARENTHESES` tool error, and verifies missing parameters fail
  clearly. The test talks to the real binary over stdio.
- Documented local use and the official MCP Inspector workflow. The server opens
  no network listener and requires no token; Inspector starts it as a child
  process under the current operating-system account.
- `cargo test --locked --workspace --all-features`: passed 13 existing tests and
  the new MCP integration test (14 total).
- `cargo test --locked --workspace`: passed all 13 default-feature tests,
  confirming the optional MCP feature is not required by library-only users.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  passed.
- `cargo fmt --all --check`: initially identified one import-layout difference;
  `cargo fmt --all` applied it, and the final formatting check passed.
- `cargo +nightly llvm-cov --branch --locked --workspace --all-features --json
  --output-path /tmp/math-mcp-coverage.json`: passed with 92.01% line, 84.00%
  function, 91.69% region, and 93.33% branch coverage. The subprocess protocol
  test is behaviorally exercised, although LLVM's per-file report does not
  attribute the spawned binary's execution to `src/mcp.rs`.
- `cargo test --locked --manifest-path vault-access/Cargo.toml --workspace` from
  the parent Remote Notes repository: all 48 tests passed, confirming the
  feature-gated dependency remains compatible with `vault-access`.

## 2026-10-02 — Make nullable MCP output schemas portable

- Replaced array-valued JSON Schema `type` declarations for `result`,
  `approximate`, and `position` with explicit `anyOf` branches containing one
  type each. Runtime response serialization and the v1 contract are unchanged.
- Extended the stdio integration test to inspect the real `tools/list` output
  and pin the portable nullable representation for all three reported fields.
- `cargo test --locked --all-features --test mcp_server`: passed.
- `cargo test --locked --workspace --all-features`: all 14 tests passed.
- `cargo test --locked --workspace`: all 13 default-feature tests passed.
- `cargo fmt --all --check` and `cargo clippy --locked --workspace --all-targets
  --all-features -- -D warnings`: passed.

## 2026-10-02 — Simplify the MCP tool name

- Renamed the public MCP tool and its Rust handler from `calculate_math` to
  `calculate`; the input and output contracts are unchanged.
- `cargo test --locked --workspace --all-features`: all 14 tests passed.
- `cargo fmt --all --check` and `cargo clippy --locked --workspace --all-targets
  --all-features -- -D warnings`: passed.

## 2026-10-02 — Add descriptive statistics MCP tool

- Added the `statistics` MCP tool with the strict input
  `{"numbers":[...]}`. It returns count, compensated sum, mean, median, sample
  standard deviation using `n-1`, minimum, and maximum in one versioned response.
- Added statrs 0.19.1 behind the existing optional `mcp` feature with default
  features disabled and only `std` enabled. `cargo tree --locked --features mcp
  -e normal` confirmed that nalgebra and rand are not in the enabled dependency
  graph.
- Enforced 2 to 10,000 finite inputs and rejected empty, singleton, oversized,
  non-finite-input, and non-finite-result cases with stable codes and hints.
  Unknown fields and non-numeric elements fail parameter deserialization.
- Added direct tests for ordering, duplicates, fractional and negative values,
  compensated cancellation, every error class, and both dataset-size boundaries.
  Extended the real stdio MCP process test to verify discovery and schemas, the
  exact `[12, 15, 9]` response, an even-sized dataset, the 10,000-value limit,
  every protocol-reachable error, JSON text fallback, and malformed arguments.
- `cargo test --locked --workspace --all-features`: all 17 tests passed.
- `cargo test --locked --workspace`: all 13 default-feature tests passed,
  confirming that statrs remains absent from default library consumers.
- `cargo fmt --all --check` and `cargo clippy --locked --workspace --all-targets
  --all-features -- -D warnings`: passed.
- `cargo +nightly llvm-cov --branch --locked --workspace --all-features --json
  --output-path /tmp/math-statistics-coverage.json`: passed with 89.93% line and
  94.44% branch coverage; `src/mcp.rs` measured 84.83% lines and 85.71% branches.
- `cargo test --locked --manifest-path vault-access/Cargo.toml --workspace` from
  the parent repository: all 48 tests passed.
