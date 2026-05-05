use crate::config::AgentConfig;
use crate::tool_backend::DiscoveredTools;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum McpFallbackError {
    #[error("wiki-cli child path does not exist: {0}")]
    MissingBinary(String),
    #[error("failed to spawn wiki-cli child: {0}")]
    Spawn(std::io::Error),
    #[error("failed to write MCP request: {0}")]
    Write(std::io::Error),
    #[error("failed to wait for MCP child: {0}")]
    Wait(std::io::Error),
    #[error("wiki-cli child exited with status {status}: {stderr}")]
    Exit { status: String, stderr: String },
    #[error("invalid MCP response: {0}")]
    InvalidResponse(String),
}

pub fn discover_child_tools(
    config: &AgentConfig,
    wiki_cli_override: Option<&Path>,
) -> Result<DiscoveredTools, McpFallbackError> {
    let wiki_cli = wiki_cli_override
        .map(Path::to_path_buf)
        .unwrap_or_else(default_wiki_cli_path);
    if !wiki_cli.exists() {
        return Err(McpFallbackError::MissingBinary(
            wiki_cli.display().to_string(),
        ));
    }

    let mut cmd = Command::new(wiki_cli);
    cmd.arg("--db")
        .arg(&config.db)
        .arg("--viewer-scope")
        .arg(&config.viewer_scope)
        .arg("--llm-config")
        .arg(&config.llm_config);
    if config.vectors {
        cmd.arg("--vectors");
    }
    if let Some(wiki_dir) = &config.wiki_dir {
        cmd.arg("--wiki-dir").arg(wiki_dir).arg("--sync-wiki");
    }
    if let Some(palace) = &config.palace {
        cmd.arg("--palace").arg(palace);
    }
    cmd.arg("mcp").arg("--once");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(McpFallbackError::Spawn)?;
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });
    if let Some(stdin) = child.stdin.as_mut() {
        writeln!(stdin, "{request}").map_err(McpFallbackError::Write)?;
    }
    let output = child.wait_with_output().map_err(McpFallbackError::Wait)?;
    if !output.status.success() {
        return Err(McpFallbackError::Exit {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| McpFallbackError::InvalidResponse("empty stdout".to_string()))?;
    let response: Value =
        serde_json::from_str(line).map_err(|e| McpFallbackError::InvalidResponse(e.to_string()))?;
    if let Some(error) = response.get("error") {
        return Err(McpFallbackError::InvalidResponse(error.to_string()));
    }
    let raw = response
        .get("result")
        .cloned()
        .ok_or_else(|| McpFallbackError::InvalidResponse("missing result".to_string()))?;
    let tools = raw
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| McpFallbackError::InvalidResponse("missing result.tools".to_string()))?
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    Ok(DiscoveredTools {
        backend: "mcp-child",
        tools,
    })
}

fn default_wiki_cli_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("wiki-cli")))
        .unwrap_or_else(|| PathBuf::from("wiki-cli"))
}
