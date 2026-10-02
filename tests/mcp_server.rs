use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpProcess {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_math-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start MCP server");
        let stdin = child.stdin.take().expect("capture server stdin");
        let stdout = BufReader::new(child.stdout.take().expect("capture server stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, message: Value) {
        serde_json::to_writer(&mut self.stdin, &message).expect("write MCP message");
        self.stdin.write_all(b"\n").expect("terminate MCP message");
        self.stdin.flush().expect("flush MCP message");
    }

    fn request(&mut self, message: Value) -> Value {
        self.send(message);
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read MCP response");
        assert!(!line.is_empty(), "MCP server closed without a response");
        serde_json::from_str(&line).expect("parse MCP response")
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn initialize(server: &mut McpProcess) {
    let response = server.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "math-4-llms-test", "version": "1.0.0"}
        }
    }));
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "math-4-llms");
    assert!(response["result"]["capabilities"]["tools"].is_object());
    server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));
}

fn assert_portable_nullable(schema: &Value, pointer: &str, value_type: &str) {
    let property = schema.pointer(pointer).expect("nullable property schema");
    assert!(
        property.get("type").is_none(),
        "{pointer} must not use a type array: {property:#}"
    );
    let branches = property["anyOf"].as_array().expect("anyOf branches");
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().all(|branch| branch["type"].is_string()));
    assert!(branches.iter().any(|branch| branch["type"] == value_type));
    assert!(branches.iter().any(|branch| branch["type"] == "null"));
}

fn tool_by_name<'a>(tools: &'a [Value], name: &str) -> &'a Value {
    tools
        .iter()
        .find(|tool| tool["name"] == name)
        .unwrap_or_else(|| panic!("missing {name} tool"))
}

fn assert_number_close(actual: &Value, expected: f64) {
    let actual = actual.as_f64().expect("numeric statistic");
    let tolerance = 1e-12 * expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}

#[test]
fn stdio_server_lists_and_calls_the_structured_math_tool() {
    let mut server = McpProcess::start();
    initialize(&mut server);

    let listed = server.request(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}
    }));
    let tools = listed["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 2);
    let calculate = tool_by_name(tools, "calculate");
    assert_eq!(calculate["inputSchema"]["required"], json!(["expression"]));
    assert_eq!(calculate["outputSchema"]["type"], "object");
    let output_schema = &calculate["outputSchema"];
    assert_portable_nullable(output_schema, "/properties/result", "string");
    assert_portable_nullable(output_schema, "/properties/approximate", "boolean");
    assert_portable_nullable(
        output_schema,
        "/$defs/MathError/properties/position",
        "integer",
    );

    let success = server.request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "calculate", "arguments": {"expression": "2 + 3 * 4"}}
    }));
    let result = &success["result"];
    assert_eq!(
        result["structuredContent"],
        json!({
            "schema_version": 1,
            "ok": true,
            "result": "14",
            "approximate": false
        })
    );
    assert_ne!(result["isError"], true);
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap())
        .expect("JSON text fallback");
    assert_eq!(text, result["structuredContent"]);

    let calculation_error = server.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {"name": "calculate", "arguments": {"expression": "(2 + 3))"}}
    }));
    assert_eq!(calculation_error["result"]["isError"], true);
    assert_eq!(
        calculation_error["result"]["structuredContent"]["error"]["code"],
        "UNBALANCED_PARENTHESES"
    );

    let malformed = server.request(json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {"name": "calculate", "arguments": {}}
    }));
    assert_eq!(malformed["result"]["isError"], true);
    assert!(
        malformed["result"]["content"][0]["text"]
            .as_str()
            .expect("parameter error text")
            .contains("missing field `expression`")
    );
}

#[test]
fn stdio_statistics_tool_validates_and_summarizes_datasets() {
    let mut server = McpProcess::start();
    initialize(&mut server);

    let listed = server.request(json!({
        "jsonrpc": "2.0", "id": 20, "method": "tools/list", "params": {}
    }));
    let tools = listed["result"]["tools"].as_array().expect("tools array");
    let statistics = tool_by_name(tools, "statistics");
    assert_eq!(statistics["inputSchema"]["required"], json!(["numbers"]));
    assert_eq!(statistics["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        statistics["inputSchema"]["properties"]["numbers"]["type"],
        "array"
    );
    assert_eq!(
        statistics["inputSchema"]["properties"]["numbers"]["minItems"],
        2
    );
    assert_eq!(
        statistics["inputSchema"]["properties"]["numbers"]["maxItems"],
        10_000
    );
    assert_eq!(
        statistics["inputSchema"]["properties"]["numbers"]["items"]["type"],
        "number"
    );

    let basic = server.request(json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "tools/call",
        "params": {"name": "statistics", "arguments": {"numbers": [12, 15, 9]}}
    }));
    assert_ne!(basic["result"]["isError"], true);
    assert_eq!(
        basic["result"]["structuredContent"],
        json!({
            "schema_version": 1,
            "ok": true,
            "count": 3,
            "sum": 36.0,
            "mean": 12.0,
            "median": 12.0,
            "sample_standard_deviation": 3.0,
            "min": 9.0,
            "max": 15.0
        })
    );
    let text: Value = serde_json::from_str(
        basic["result"]["content"][0]["text"]
            .as_str()
            .expect("statistics text fallback"),
    )
    .expect("JSON statistics text fallback");
    assert_eq!(text, basic["result"]["structuredContent"]);

    let varied = server.request(json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "tools/call",
        "params": {"name": "statistics", "arguments": {"numbers": [10, 2, 4, 8]}}
    }));
    let varied = &varied["result"]["structuredContent"];
    assert_eq!(varied["count"], 4);
    assert_number_close(&varied["sum"], 24.0);
    assert_number_close(&varied["mean"], 6.0);
    assert_number_close(&varied["median"], 6.0);
    assert_number_close(
        &varied["sample_standard_deviation"],
        (40.0_f64 / 3.0).sqrt(),
    );
    assert_number_close(&varied["min"], 2.0);
    assert_number_close(&varied["max"], 10.0);

    let boundary = server.request(json!({
        "jsonrpc": "2.0",
        "id": 23,
        "method": "tools/call",
        "params": {"name": "statistics", "arguments": {"numbers": vec![1.0; 10_000]}}
    }));
    assert_ne!(boundary["result"]["isError"], true);
    assert_eq!(boundary["result"]["structuredContent"]["count"], 10_000);
    assert_eq!(
        boundary["result"]["structuredContent"]["sample_standard_deviation"],
        0.0
    );

    for (id, numbers, code) in [
        (24, json!([]), "EMPTY_DATASET"),
        (25, json!([7]), "SAMPLE_TOO_SMALL"),
        (
            26,
            json!([1.7976931348623157e308, 1.7976931348623157e308]),
            "NON_FINITE_RESULT",
        ),
        (27, json!(vec![1.0; 10_001]), "TOO_MANY_VALUES"),
    ] {
        let error = server.request(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": "statistics", "arguments": {"numbers": numbers}}
        }));
        assert_eq!(error["result"]["isError"], true, "{error:#}");
        assert_eq!(
            error["result"]["structuredContent"]["code"], code,
            "{error:#}"
        );
        assert_eq!(error["result"]["structuredContent"]["ok"], false);
        assert!(error["result"]["structuredContent"]["hint"].is_string());
    }

    for (id, arguments, expected) in [
        (28, json!({}), "missing field `numbers`"),
        (29, json!({"numbers": [1, "two"]}), "invalid type"),
        (
            30,
            json!({"numbers": [1, 2], "sample": true}),
            "unknown field `sample`",
        ),
    ] {
        let malformed = server.request(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": "statistics", "arguments": arguments}
        }));
        assert_eq!(malformed["result"]["isError"], true, "{malformed:#}");
        assert!(
            malformed["result"]["content"][0]["text"]
                .as_str()
                .expect("parameter error text")
                .contains(expected),
            "{malformed:#}"
        );
    }
}
