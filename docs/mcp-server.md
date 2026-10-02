# MCP server and Inspector guide

The `math-mcp` binary exposes the calculator as one Model Context Protocol tool
over the local standard-input/standard-output transport. It does not open a
network port, authenticate users, access notes, or read and write files.

## Tool contract

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
stdio. It verifies initialization, tool discovery, the published schemas, a
successful calculation, a stable calculation error, and malformed parameters:

```sh
cargo test --locked --all-features --test mcp_server
```
