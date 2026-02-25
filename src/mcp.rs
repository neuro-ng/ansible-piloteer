//! MCP (Model Context Protocol) server for Antigravity IDE and Claude Code integration.
//! Uses `rs-fast-mcp` for async-first MCP protocol handling over stdio.
//!
//! [Phase 38] — Exposes Ansible session data as Resources and provides Tools
//! for execution control, querying, and inspection.

use anyhow::Result;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::{
    BaseMetadata, ContentBlock, Resource, ResourceContents, ResourceTemplate, TextContent,
};
use rs_fast_mcp::resources::manager::ResourceReadHandler;
use rs_fast_mcp::server::builder::ServerBuilder;
use rs_fast_mcp::tools::tool::{Tool, ToolResult};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ── Public entry point ──────────────────────────────────────────────

/// Run the MCP stdio server. Registers tools and resources, then runs
/// using the stdio transport for Antigravity IDE / Claude Code mounting.
pub async fn run_stdio_server() -> Result<()> {
    let server = ServerBuilder::new("ansible-piloteer", env!("CARGO_PKG_VERSION"))
        .stdio()
        .build();

    // ── Register Tools ──
    register_tools(&server.core)?;

    // ── Register Resources ──
    register_resources(&server.core)?;

    eprintln!("[piloteer-mcp] MCP server started (rs-fast-mcp stdio transport)");
    server.run().await.map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

// ── Tool Registration ───────────────────────────────────────────────

fn register_tools(core: &rs_fast_mcp::server::core::FastMCPServer) -> Result<()> {
    // piloteer_query — JMESPath query against a session file
    let query_tool = Tool::new(
        "piloteer_query",
        "Run a JMESPath query against a saved Piloteer session file.",
    )
    .add_parameter(
        "input",
        "string",
        "Path to session file (e.g. session.json.gz)",
    )
    .add_parameter("query", "string", "JMESPath query expression")
    .with_handler(Box::new(|_ctx, args| {
        Box::pin(async move { tool_query(args).await })
            as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
    }));
    core.add_tool(query_tool)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // piloteer_list_sessions — List archived session files
    let list_tool = Tool::new(
        "piloteer_list_sessions",
        "List archived Piloteer session files.",
    )
    .with_handler(Box::new(|_ctx, _args| {
        Box::pin(async move { tool_list_sessions().await })
            as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
    }));
    core.add_tool(list_tool)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // piloteer_status — Get session summary
    let status_tool = Tool::new(
        "piloteer_status",
        "Get summary of a Piloteer session (host counts, task counts, failures).",
    )
    .add_parameter("input", "string", "Path to session file")
    .with_handler(Box::new(|_ctx, args| {
        Box::pin(async move { tool_status(args).await })
            as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
    }));
    core.add_tool(status_tool)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // piloteer_run — Start ansible-playbook execution
    let run_tool = Tool::new(
        "piloteer_run",
        "Start an Ansible playbook execution with specific parameters.",
    )
    .add_parameter("playbook", "string", "Path to the Ansible playbook file")
    .add_parameter("inventory", "string", "Path to inventory file (optional)")
    .add_parameter(
        "extra_vars",
        "string",
        "Extra variables as YAML/JSON string (optional)",
    )
    .with_handler(Box::new(|_ctx, args| {
        Box::pin(async move { tool_run(args).await })
            as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
    }));
    core.add_tool(run_tool)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // piloteer_inspect — Deep JMESPath query on a specific archived session
    let inspect_tool = Tool::new(
        "piloteer_inspect",
        "Perform a deep JMESPath query across a specific archived session's data.",
    )
    .add_parameter("session_id", "string", "Session archive filename or path")
    .add_parameter("query", "string", "JMESPath query expression")
    .with_handler(Box::new(|_ctx, args| {
        Box::pin(async move { tool_inspect(args).await })
            as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
    }));
    core.add_tool(inspect_tool)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

// ── Resource Registration ───────────────────────────────────────────

fn register_resources(core: &rs_fast_mcp::server::core::FastMCPServer) -> Result<()> {
    // Static resource: list all available sessions
    let sessions_resource = Resource {
        uri: "ansible://sessions/list".to_string(),
        base_metadata: BaseMetadata {
            name: "Session List".to_string(),
            title: Some("Archived Piloteer Sessions".to_string()),
        },
        description: Some("Lists all available archived session IDs".to_string()),
        mime_type: Some("application/json".to_string()),
        annotations: None,
        size: None,
        icons: None,
        tags: None,
    };
    let sessions_handler: Arc<ResourceReadHandler> = Arc::new(Box::new(|_uri, _ctx| {
        Box::pin(async move { resource_sessions_list().await })
            as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
    }));
    core.add_resource(sessions_resource, Some(sessions_handler))
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Template: session facts
    let facts_template = ResourceTemplate {
        uri_template: "ansible://sessions/{id}/facts".to_string(),
        name: "Session Facts".to_string(),
        description: Some("Collected Ansible facts for a specific session".to_string()),
        mime_type: Some("application/json".to_string()),
        annotations: None,
    };
    let facts_handler: Arc<ResourceReadHandler> = Arc::new(Box::new(|uri, _ctx| {
        Box::pin(async move { resource_session_data(&uri, "facts").await })
            as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
    }));
    core.add_resource_template(facts_template, facts_handler)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Template: session vars
    let vars_template = ResourceTemplate {
        uri_template: "ansible://sessions/{id}/vars".to_string(),
        name: "Session Variables".to_string(),
        description: Some("Task variables at point of failure for a specific session".to_string()),
        mime_type: Some("application/json".to_string()),
        annotations: None,
    };
    let vars_handler: Arc<ResourceReadHandler> = Arc::new(Box::new(|uri, _ctx| {
        Box::pin(async move { resource_session_data(&uri, "vars").await })
            as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
    }));
    core.add_resource_template(vars_template, vars_handler)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Template: session logs
    let logs_template = ResourceTemplate {
        uri_template: "ansible://sessions/{id}/logs".to_string(),
        name: "Session Logs".to_string(),
        description: Some("Full execution logs for a specific session".to_string()),
        mime_type: Some("application/json".to_string()),
        annotations: None,
    };
    let logs_handler: Arc<ResourceReadHandler> = Arc::new(Box::new(|uri, _ctx| {
        Box::pin(async move { resource_session_data(&uri, "logs").await })
            as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
    }));
    core.add_resource_template(logs_template, logs_handler)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

// ── Tool Implementations ────────────────────────────────────────────

fn text_result(text: String) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::Text(TextContent {
            text,
            type_: "text".to_string(),
            annotations: None,
        })],
        structured_content: None,
    }
}

fn err(msg: String) -> FastMCPError {
    FastMCPError::new(msg)
}

async fn tool_query(args: Value) -> Result<ToolResult, FastMCPError> {
    let input = args
        .get("input")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("Missing 'input' argument".to_string()))?;
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("Missing 'query' argument".to_string()))?;

    let session = crate::session::Session::load(input)
        .map_err(|e| err(format!("Failed to load session: {}", e)))?;

    let json_string =
        serde_json::to_string(&session).map_err(|e| err(format!("Serialization error: {}", e)))?;

    let mut runtime = jmespath::Runtime::new();
    runtime.register_builtin_functions();
    crate::query::register_functions(&mut runtime);

    let expr = runtime
        .compile(query)
        .map_err(|e| err(format!("Invalid JMESPath: {}", e)))?;

    let variable = jmespath::Variable::from_json(&json_string)
        .map_err(|e| err(format!("JSON parse error: {}", e)))?;

    let result = expr
        .search(&variable)
        .map_err(|e| err(format!("Query error: {}", e)))?;

    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(text_result(text))
}

async fn tool_list_sessions() -> Result<ToolResult, FastMCPError> {
    let config_dir = crate::config::Config::get_config_dir()
        .map_err(|e| err(format!("Config dir error: {}", e)))?;

    let archive_dir = config_dir.join("archive");
    if !archive_dir.exists() {
        return Ok(text_result("No archived sessions found.".to_string()));
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
    sessions.reverse();

    let text = if sessions.is_empty() {
        "No archived sessions found.".to_string()
    } else {
        sessions.join("\n")
    };

    Ok(text_result(text))
}

async fn tool_status(args: Value) -> Result<ToolResult, FastMCPError> {
    let input = args
        .get("input")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("Missing 'input' argument".to_string()))?;

    let session = crate::session::Session::load(input)
        .map_err(|e| err(format!("Failed to load session: {}", e)))?;

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

    Ok(text_result(summary))
}

async fn tool_run(args: Value) -> Result<ToolResult, FastMCPError> {
    let playbook = args
        .get("playbook")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("Missing 'playbook' argument".to_string()))?;

    let mut cmd_args = vec!["ansible-playbook".to_string(), playbook.to_string()];

    if let Some(inventory) = args.get("inventory").and_then(|v| v.as_str()) {
        cmd_args.push("-i".to_string());
        cmd_args.push(inventory.to_string());
    }

    if let Some(extra_vars) = args.get("extra_vars").and_then(|v| v.as_str()) {
        cmd_args.push("-e".to_string());
        cmd_args.push(extra_vars.to_string());
    }

    // Return the command that would be executed.
    // Full execution integration will be wired to the TUI event loop in a future phase.
    let text = format!(
        "Prepared execution command:\n  {}\n\n\
         Note: Full execution integration requires the TUI event loop. \
         Use `ansible-piloteer -- {}` from the command line for now.",
        cmd_args.join(" "),
        cmd_args[1..].join(" ")
    );
    Ok(text_result(text))
}

async fn tool_inspect(args: Value) -> Result<ToolResult, FastMCPError> {
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("Missing 'session_id' argument".to_string()))?;
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("Missing 'query' argument".to_string()))?;

    let path = resolve_session_path(session_id)?;

    let session = crate::session::Session::load(&path)
        .map_err(|e| err(format!("Failed to load session '{}': {}", session_id, e)))?;

    let json_string =
        serde_json::to_string(&session).map_err(|e| err(format!("Serialization error: {}", e)))?;

    let mut runtime = jmespath::Runtime::new();
    runtime.register_builtin_functions();
    crate::query::register_functions(&mut runtime);

    let expr = runtime
        .compile(query)
        .map_err(|e| err(format!("Invalid JMESPath: {}", e)))?;

    let variable = jmespath::Variable::from_json(&json_string)
        .map_err(|e| err(format!("JSON parse error: {}", e)))?;

    let result = expr
        .search(&variable)
        .map_err(|e| err(format!("Query error: {}", e)))?;

    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(text_result(text))
}

// ── Resource Implementations ────────────────────────────────────────

async fn resource_sessions_list() -> Result<Vec<ResourceContents>, FastMCPError> {
    let config_dir = crate::config::Config::get_config_dir()
        .map_err(|e| err(format!("Config dir error: {}", e)))?;

    let archive_dir = config_dir.join("archive");
    let mut sessions: Vec<String> = Vec::new();
    if archive_dir.exists()
        && let Ok(entries) = std::fs::read_dir(&archive_dir)
    {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json.gz") {
                sessions.push(name);
            }
        }
    }
    sessions.sort();
    sessions.reverse();

    let text = serde_json::to_string_pretty(&sessions).unwrap_or_else(|_| "[]".to_string());
    Ok(vec![ResourceContents {
        uri: "ansible://sessions/list".to_string(),
        mime_type: Some("application/json".to_string()),
        text: Some(text),
        blob: None,
    }])
}

async fn resource_session_data(
    uri: &str,
    field: &str,
) -> Result<Vec<ResourceContents>, FastMCPError> {
    let session_id = extract_session_id(uri)?;
    let path = resolve_session_path(&session_id)?;

    let session = crate::session::Session::load(&path)
        .map_err(|e| err(format!("Failed to load session '{}': {}", session_id, e)))?;

    let data = match field {
        "facts" => serde_json::to_string_pretty(&session.facts),
        "vars" => serde_json::to_string_pretty(&session.task_vars),
        "logs" => {
            let log_lines: Vec<&str> = session.logs.iter().map(|(text, _)| text.as_str()).collect();
            serde_json::to_string_pretty(&log_lines)
        }
        _ => Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unknown field: {}", field),
        ))),
    }
    .map_err(|e| err(format!("Serialization error: {}", e)))?;

    Ok(vec![ResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text: Some(data),
        blob: None,
    }])
}

// ── Helpers ─────────────────────────────────────────────────────────

fn extract_session_id(uri: &str) -> Result<String, FastMCPError> {
    // URI format: ansible://sessions/{id}/facts (or vars, logs)
    // parts: ["ansible:", "", "sessions", "{id}", "field"]
    let parts: Vec<&str> = uri.split('/').collect();
    if parts.len() >= 4 {
        Ok(parts[3].to_string())
    } else {
        Err(err(format!("Invalid session URI: {}", uri)))
    }
}

fn resolve_session_path(session_id: &str) -> Result<String, FastMCPError> {
    if session_id.contains('/') || session_id.contains('\\') {
        return Ok(session_id.to_string());
    }

    let config_dir = crate::config::Config::get_config_dir()
        .map_err(|e| err(format!("Config dir error: {}", e)))?;

    let archive_dir = config_dir.join("archive");
    let mut filename = session_id.to_string();
    if !filename.ends_with(".json.gz") {
        filename.push_str(".json.gz");
    }

    let path = archive_dir.join(&filename);
    Ok(path.to_string_lossy().to_string())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_session_id() {
        let id = extract_session_id("ansible://sessions/my-session-123/facts").unwrap();
        assert_eq!(id, "my-session-123");
    }

    #[test]
    fn test_extract_session_id_vars() {
        let id = extract_session_id("ansible://sessions/test-session/vars").unwrap();
        assert_eq!(id, "test-session");
    }

    #[test]
    fn test_resolve_session_path_full() {
        let path = resolve_session_path("/tmp/session.json.gz").unwrap();
        assert_eq!(path, "/tmp/session.json.gz");
    }

    #[test]
    fn test_resolve_session_path_bare() {
        let path = resolve_session_path("my-session").unwrap();
        assert!(path.ends_with("my-session.json.gz"));
    }

    #[test]
    fn test_text_result() {
        let result = text_result("hello".to_string());
        assert_eq!(result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_list_sessions_no_archive() {
        let result = tool_list_sessions().await;
        assert!(result.is_ok());
    }
}
