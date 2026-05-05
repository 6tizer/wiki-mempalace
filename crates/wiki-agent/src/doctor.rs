use crate::config::AgentConfig;
use crate::mcp_fallback;
use crate::tool_backend::{discover_native_tools, DiscoveredTools, ToolBackendKind};
use std::path::Path;

pub fn run(
    config: &AgentConfig,
    backend: ToolBackendKind,
    wiki_cli: Option<&Path>,
) -> Result<String, Box<dyn std::error::Error>> {
    let discovered = match backend {
        ToolBackendKind::Native => discover_native_tools(),
        ToolBackendKind::McpChild => mcp_fallback::discover_child_tools(config, wiki_cli)?,
        ToolBackendKind::Auto => discover_native_tools(),
    };
    Ok(render_report(discovered))
}

fn render_report(discovered: DiscoveredTools) -> String {
    let mut tools = discovered.tools;
    tools.sort();
    format!(
        "wiki-agent doctor\n\
         tool_backend={}\n\
         tools={}\n\
         tool_names={}\n",
        discovered.backend,
        tools.len(),
        tools.join(",")
    )
}
