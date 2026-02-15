//! MCP (Model Context Protocol) stdio server for Antigravity IDE integration.
//! Exposes Piloteer tools via JSON-RPC 2.0 over stdin/stdout.
//!
//! [Phase 36] — Allows Antigravity's AI assistant to invoke Piloteer commands.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

// ── JSON-RPC 2.0 types ──────────────────────────────────────────────

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// ── MCP protocol types ──────────────────────────────────────────────

#[derive(Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

// ── Public entry point ──────────────────────────────────────────────

/// Run the MCP stdio server. Reads JSON-RPC from stdin, writes responses to stdout.
pub fn run_stdio_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    eprintln!("[piloteer-mcp] MCP server started (stdio transport)");

    for line in stdin.lock().lines() {
        let line = line.context("Failed to read stdin")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&err_resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let response = handle_request(&request);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}

// ── Request router ──────────────────────────────────────────────────

fn handle_request(req: &JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(Value::Null);

    let result = match req.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(&req.params),
        _ => Err((-32601, format!("Method not found: {}", req.method))),
    };

    match result {
        Ok(val) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(val),
            error: None,
        },
        Err((code, msg)) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message: msg }),
        },
    }
}

// ── Handler implementations ─────────────────────────────────────────

fn handle_initialize() -> Result<Value, (i32, String)> {
    Ok(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "ansible-piloteer",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

fn handle_tools_list() -> Result<Value, (i32, String)> {
    let tools = vec![
        ToolDefinition {
            name: "piloteer_query".into(),
            description: "Run a JMESPath query against a saved Piloteer session file.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Path to session file (e.g. session.json.gz)"
                    },
                    "query": {
                        "type": "string",
                        "description": "JMESPath query expression"
                    }
                },
                "required": ["input", "query"]
            }),
        },
        ToolDefinition {
            name: "piloteer_list_sessions".into(),
            description: "List archived Piloteer session files.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "piloteer_status".into(),
            description: "Get summary of a Piloteer session (host counts, task counts, failures)."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Path to session file"
                    }
                },
                "required": ["input"]
            }),
        },
    ];

    Ok(serde_json::json!({ "tools": tools }))
}

fn handle_tools_call(params: &Value) -> Result<Value, (i32, String)> {
    let tool_name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or((-32602, "Missing 'name' parameter".into()))?;

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));

    match tool_name {
        "piloteer_query" => tool_query(&arguments),
        "piloteer_list_sessions" => tool_list_sessions(),
        "piloteer_status" => tool_status(&arguments),
        _ => Err((-32602, format!("Unknown tool: {}", tool_name))),
    }
}

// ── Tool implementations ────────────────────────────────────────────

fn tool_query(args: &Value) -> Result<Value, (i32, String)> {
    let input = args
        .get("input")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "Missing 'input' argument".to_string()))?;
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "Missing 'query' argument".to_string()))?;

    // Load session
    let session = crate::session::Session::load(input)
        .map_err(|e| (-32603, format!("Failed to load session: {}", e)))?;

    let json_string = serde_json::to_string(&session)
        .map_err(|e| (-32603, format!("Serialization error: {}", e)))?;

    let mut runtime = jmespath::Runtime::new();
    runtime.register_builtin_functions();
    crate::query::register_functions(&mut runtime);

    let expr = runtime
        .compile(query)
        .map_err(|e| (-32602, format!("Invalid JMESPath: {}", e)))?;

    let variable = jmespath::Variable::from_json(&json_string)
        .map_err(|e| (-32603, format!("JSON parse error: {}", e)))?;

    let result = expr
        .search(&variable)
        .map_err(|e| (-32603, format!("Query error: {}", e)))?;

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
        }]
    }))
}

fn tool_list_sessions() -> Result<Value, (i32, String)> {
    let config_dir = crate::config::Config::get_config_dir()
        .map_err(|e| (-32603, format!("Config dir error: {}", e)))?;

    let archive_dir = config_dir.join("archive");
    if !archive_dir.exists() {
        return Ok(serde_json::json!({
            "content": [{
                "type": "text",
                "text": "No archived sessions found."
            }]
        }));
    }

    let mut sessions: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&archive_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json.gz") {
                sessions.push(entry.path().to_string_lossy().to_string());
            }
        }
    }
    sessions.sort();
    sessions.reverse(); // Most recent first

    let text = if sessions.is_empty() {
        "No archived sessions found.".to_string()
    } else {
        sessions.join("\n")
    };

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": text
        }]
    }))
}

fn tool_status(args: &Value) -> Result<Value, (i32, String)> {
    let input = args
        .get("input")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "Missing 'input' argument".to_string()))?;

    let session = crate::session::Session::load(input)
        .map_err(|e| (-32603, format!("Failed to load session: {}", e)))?;

    let total_tasks = session.history.len();
    let failed = session.history.iter().filter(|t| t.failed).count();
    let changed = session.history.iter().filter(|t| t.changed).count();
    let ok = total_tasks - failed - changed;
    let total_hosts = session.hosts.len();
    let total_logs = session.logs.len();

    let summary = format!(
        "Session: {}\n\
         Hosts: {}\n\
         Tasks: {} total ({} ok, {} changed, {} failed)\n\
         Logs: {} entries",
        input, total_hosts, total_tasks, ok, changed, failed, total_logs
    );

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": summary
        }]
    }))
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(1.into())),
            method: "initialize".into(),
            params: Value::Object(Default::default()),
        };
        let resp = handle_request(&req);
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "ansible-piloteer");
    }

    #[test]
    fn test_tools_list() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(2.into())),
            method: "tools/list".into(),
            params: Value::Object(Default::default()),
        };
        let resp = handle_request(&req);
        assert!(resp.error.is_none());
        let tools = resp.result.unwrap();
        let tool_list = tools["tools"].as_array().unwrap();
        assert_eq!(tool_list.len(), 3);
        assert_eq!(tool_list[0]["name"], "piloteer_query");
        assert_eq!(tool_list[1]["name"], "piloteer_list_sessions");
        assert_eq!(tool_list[2]["name"], "piloteer_status");
    }

    #[test]
    fn test_unknown_method() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(3.into())),
            method: "unknown/method".into(),
            params: Value::Object(Default::default()),
        };
        let resp = handle_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_list_sessions_no_archive() {
        // Should not panic even if archive dir doesn't exist
        let result = tool_list_sessions();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_error_response() {
        let req_str = "not json at all";
        let parsed: Result<JsonRpcRequest, _> = serde_json::from_str(req_str);
        assert!(parsed.is_err());
    }
}
