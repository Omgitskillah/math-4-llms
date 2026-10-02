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

#[test]
fn stdio_server_lists_and_calls_the_structured_math_tool() {
    let mut server = McpProcess::start();
    initialize(&mut server);

    let listed = server.request(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}
    }));
    let tools = listed["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "calculate");
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["expression"]));
    assert_eq!(tools[0]["outputSchema"]["type"], "object");
    let output_schema = &tools[0]["outputSchema"];
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
