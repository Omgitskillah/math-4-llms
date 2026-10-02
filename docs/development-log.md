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
