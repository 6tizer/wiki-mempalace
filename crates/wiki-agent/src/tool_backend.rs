use clap::ValueEnum;
use wiki_tools::ToolRegistry;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ToolBackendKind {
    Native,
    McpChild,
    Auto,
}

#[derive(Clone, Debug)]
pub struct DiscoveredTools {
    pub backend: &'static str,
    pub tools: Vec<String>,
}

pub fn discover_native_tools() -> DiscoveredTools {
    let registry = ToolRegistry;
    let tools = registry
        .list()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    DiscoveredTools {
        backend: "native",
        tools,
    }
}
