use crate::planner::{TaskPlan, WebMode};
use crate::tool_backend::ToolBackendKind;

pub(crate) struct PromptContext<'a> {
    pub(crate) profile: &'a str,
    pub(crate) viewer_scope: &'a str,
    pub(crate) tool_backend: ToolBackendKind,
    pub(crate) web_mode: WebMode,
    pub(crate) allow_private_web_search: bool,
    pub(crate) task_plan: &'a TaskPlan,
}

pub(crate) fn build_system_prompt(ctx: PromptContext<'_>) -> String {
    let tool_names = ctx.task_plan.tool_names().join(", ");
    let private_web_policy = if ctx.allow_private_web_search {
        "private scope web search may run only when explicitly allowed by this invocation"
    } else {
        "private:* viewer scopes must not use web search"
    };
    let web_policy = if ctx.task_plan.evidence_budget.allow_web {
        "web evidence is allowed and must be cross-verified before it supports claims"
    } else {
        "local evidence first; web is off, blocked, or not needed for this task"
    };

    [
        "You are wiki-agent. Answer directly and ground every substantive claim in supplied evidence.",
        "",
        "Runtime context:",
        &format!("- profile={}", ctx.profile),
        &format!("- viewer_scope={}", ctx.viewer_scope),
        &format!("- tool_backend={:?}", ctx.tool_backend),
        &format!("- web_mode={}", ctx.web_mode),
        &format!("- intent={}", ctx.task_plan.intent.render()),
        &format!("- risk={:?}", ctx.task_plan.risk),
        &format!("- tools={tool_names}"),
        "",
        "Tool policy:",
        tool_backend_policy(ctx.tool_backend),
        "Use native tools as the primary backend. Treat MCP as fallback unless the selected backend explicitly requires it.",
        "",
        "Memory policy:",
        "Use session memory as context, but do not treat memories as proof unless they appear in the evidence pack.",
        "Never save or expose secrets. Private scope content stays private to the viewer_scope.",
        "",
        "Evidence rules:",
        web_policy,
        private_web_policy,
        "Prefer local wiki evidence for stable knowledge. Use web evidence only for freshness or external facts.",
        "If evidence is missing, failed, blocked, or not cross-verified, say so clearly instead of inventing support.",
        "When citing, use source titles, doc ids, URLs, or evidence labels already present in the prompt.",
        "",
        "Output contract:",
        "Answer the user request first. Keep the response concise. Include caveats only when evidence is weak or degraded.",
    ]
    .join("\n")
}

fn tool_backend_policy(backend: ToolBackendKind) -> &'static str {
    match backend {
        ToolBackendKind::Native => "Current backend is native; do not claim MCP tool execution.",
        ToolBackendKind::McpChild => "Current backend is MCP child fallback; preserve native tool semantics where possible.",
        ToolBackendKind::Auto => "Current backend is auto; prefer native tools and fall back to MCP only when native is unavailable.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::WebMode;

    #[test]
    fn prompt_injects_runtime_context_and_policies() {
        let plan = TaskPlan::for_chat("latest release", WebMode::Always, "shared:wiki", false);
        let prompt = build_system_prompt(PromptContext {
            profile: "default",
            viewer_scope: "shared:wiki",
            tool_backend: ToolBackendKind::Native,
            web_mode: WebMode::Always,
            allow_private_web_search: false,
            task_plan: &plan,
        });

        assert!(prompt.contains("profile=default"));
        assert!(prompt.contains("viewer_scope=shared:wiki"));
        assert!(prompt.contains("tool_backend=Native"));
        assert!(prompt.contains("intent=fresh_answer"));
        assert!(prompt.contains("web evidence is allowed and must be cross-verified"));
        assert!(prompt.contains("Use native tools as the primary backend"));
    }

    #[test]
    fn prompt_blocks_private_web_by_default() {
        let plan = TaskPlan::for_chat("latest release", WebMode::Always, "private:agent", false);
        let prompt = build_system_prompt(PromptContext {
            profile: "default",
            viewer_scope: "private:agent",
            tool_backend: ToolBackendKind::Auto,
            web_mode: WebMode::Always,
            allow_private_web_search: false,
            task_plan: &plan,
        });

        assert!(prompt.contains("private:* viewer scopes must not use web search"));
        assert!(prompt.contains("local evidence first; web is off, blocked, or not needed"));
    }
}
