use crate::roles::WorkerRole;
use clap::ValueEnum;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum WebMode {
    Auto,
    Always,
    Off,
}

impl fmt::Display for WebMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Always => f.write_str("always"),
            Self::Off => f.write_str("off"),
        }
    }
}

impl FromStr for WebMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "off" => Ok(Self::Off),
            other => Err(format!("unknown web mode: {other}")),
        }
    }
}

pub fn should_use_web(query: &str, mode: WebMode) -> bool {
    match mode {
        WebMode::Off => false,
        WebMode::Always => true,
        WebMode::Auto => looks_external_or_fresh(query),
    }
}

pub fn private_scope_blocks_web(viewer_scope: &str, allow_private_web_search: bool) -> bool {
    !allow_private_web_search && viewer_scope.trim().starts_with("private:")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskIntent {
    KnowledgeAnswer,
    FreshAnswer,
    Lint,
    Governance,
    Fixer,
    Synthesis,
    Search,
    MemoryCuration,
}

impl TaskIntent {
    pub(crate) fn render(self) -> &'static str {
        match self {
            Self::KnowledgeAnswer => "knowledge_answer",
            Self::FreshAnswer => "fresh_answer",
            Self::Lint => "lint",
            Self::Governance => "governance",
            Self::Fixer => "fixer",
            Self::Synthesis => "synthesis",
            Self::Search => "search",
            Self::MemoryCuration => "memory_curation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskRisk {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceBudget {
    pub(crate) local_limit: usize,
    pub(crate) allow_web: bool,
    pub(crate) require_cross_verified_web: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetryPolicy {
    pub(crate) max_retries: usize,
    pub(crate) retry_on_low_evidence: bool,
    pub(crate) retry_on_tool_failure: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlannedTool {
    WikiQuery,
    WebSearch,
    Worker(WorkerRole),
}

impl PlannedTool {
    pub(crate) fn render(self) -> &'static str {
        match self {
            Self::WikiQuery => "wiki_query",
            Self::WebSearch => "web_search",
            Self::Worker(role) => role.agent_name(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TaskPlan {
    pub(crate) intent: TaskIntent,
    pub(crate) evidence_budget: EvidenceBudget,
    pub(crate) tools: Vec<PlannedTool>,
    pub(crate) workers: Vec<WorkerRole>,
    pub(crate) risk: TaskRisk,
    pub(crate) retry_policy: RetryPolicy,
}

impl TaskPlan {
    pub(crate) fn for_chat(
        query: &str,
        web_mode: WebMode,
        viewer_scope: &str,
        allow_private_web_search: bool,
    ) -> Self {
        let worker = route_worker(query);
        let wants_web = should_use_web(query, web_mode);
        let web_blocked = private_scope_blocks_web(viewer_scope, allow_private_web_search);
        let allow_web = wants_web && !web_blocked;
        let intent = intent_for_query(query, worker, allow_web);
        let risk = risk_for_worker(worker);
        let retry_policy = retry_policy_for(intent, risk);
        let mut tools = vec![PlannedTool::WikiQuery];
        if allow_web {
            tools.push(PlannedTool::WebSearch);
        }
        if let Some(role) = worker {
            tools.push(PlannedTool::Worker(role));
        }
        Self {
            intent,
            evidence_budget: EvidenceBudget {
                local_limit: 5,
                allow_web,
                require_cross_verified_web: allow_web,
            },
            tools,
            workers: worker.into_iter().collect(),
            risk,
            retry_policy,
        }
    }

    pub(crate) fn tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|tool| tool.render().to_string())
            .collect()
    }
}

fn looks_external_or_fresh(query: &str) -> bool {
    let q = query.to_ascii_lowercase();
    let lower_hits = [
        "latest",
        "current",
        "today",
        "this week",
        "news",
        "release",
        "pricing",
    ];
    lower_hits.iter().any(|needle| q.contains(needle))
        || ["最新", "今天", "最近", "现在", "新闻", "价格", "发布"]
            .iter()
            .any(|needle| query.contains(needle))
}

fn intent_for_query(query: &str, worker: Option<WorkerRole>, allow_web: bool) -> TaskIntent {
    match worker {
        Some(WorkerRole::Lint) => TaskIntent::Lint,
        Some(WorkerRole::Governance) => TaskIntent::Governance,
        Some(WorkerRole::Fixer) => TaskIntent::Fixer,
        Some(WorkerRole::Synthesis) => TaskIntent::Synthesis,
        Some(WorkerRole::Search) => TaskIntent::Search,
        Some(WorkerRole::MemoryCurator) => TaskIntent::MemoryCuration,
        None if allow_web || looks_external_or_fresh(query) => TaskIntent::FreshAnswer,
        None => TaskIntent::KnowledgeAnswer,
    }
}

fn risk_for_worker(worker: Option<WorkerRole>) -> TaskRisk {
    match worker {
        Some(WorkerRole::Fixer) => TaskRisk::High,
        Some(_) => TaskRisk::Medium,
        None => TaskRisk::Low,
    }
}

fn retry_policy_for(intent: TaskIntent, risk: TaskRisk) -> RetryPolicy {
    RetryPolicy {
        max_retries: match risk {
            TaskRisk::High => 0,
            TaskRisk::Medium => 1,
            TaskRisk::Low => 1,
        },
        retry_on_low_evidence: matches!(
            intent,
            TaskIntent::KnowledgeAnswer | TaskIntent::FreshAnswer
        ),
        retry_on_tool_failure: !matches!(risk, TaskRisk::High),
    }
}

fn route_worker(query: &str) -> Option<WorkerRole> {
    let q = query.to_ascii_lowercase();
    if contains_any(&q, query, &["lint", "health check"], &["体检", "健康检查"]) {
        return Some(WorkerRole::Lint);
    }
    if contains_any(
        &q,
        query,
        &["governance", "orphan", "stale", "audit"],
        &["治理", "孤儿", "过期", "审计"],
    ) {
        return Some(WorkerRole::Governance);
    }
    if contains_any(
        &q,
        query,
        &["fix", "repair", "apply", "patch"],
        &["修复", "修理", "应用"],
    ) {
        return Some(WorkerRole::Fixer);
    }
    if contains_any(
        &q,
        query,
        &["synthesis", "summarize", "connect"],
        &["综合", "总结", "关联"],
    ) {
        return Some(WorkerRole::Synthesis);
    }
    if contains_any(
        &q,
        query,
        &["memory", "remember", "skill"],
        &["记忆", "记住", "技能"],
    ) {
        return Some(WorkerRole::MemoryCurator);
    }
    if contains_any(
        &q,
        query,
        &["search", "find", "lookup"],
        &["搜索", "查找", "检索"],
    ) {
        return Some(WorkerRole::Search);
    }
    None
}

fn contains_any(lower_query: &str, query: &str, lower_needles: &[&str], needles: &[&str]) -> bool {
    lower_needles
        .iter()
        .any(|needle| lower_query.contains(needle))
        || needles.iter().any(|needle| query.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_web_detects_freshness_queries() {
        assert!(should_use_web("latest xAI release", WebMode::Auto));
        assert!(should_use_web("最近有什么变化", WebMode::Auto));
        assert!(!should_use_web("explain my local wiki", WebMode::Auto));
    }

    #[test]
    fn private_scope_blocks_by_default() {
        assert!(private_scope_blocks_web("private:cli", false));
        assert!(!private_scope_blocks_web("private:cli", true));
        assert!(!private_scope_blocks_web("shared:wiki", false));
    }

    #[test]
    fn task_plan_prefers_local_for_knowledge_answers() {
        let plan = TaskPlan::for_chat("explain my local wiki", WebMode::Auto, "shared:wiki", false);
        assert_eq!(plan.intent, TaskIntent::KnowledgeAnswer);
        assert_eq!(plan.evidence_budget.local_limit, 5);
        assert!(!plan.evidence_budget.allow_web);
        assert_eq!(plan.tools, vec![PlannedTool::WikiQuery]);
        assert!(plan.workers.is_empty());
    }

    #[test]
    fn task_plan_allows_web_for_fresh_shared_queries() {
        let plan = TaskPlan::for_chat("latest xAI release", WebMode::Auto, "shared:wiki", false);
        assert_eq!(plan.intent, TaskIntent::FreshAnswer);
        assert!(plan.evidence_budget.allow_web);
        assert!(plan.evidence_budget.require_cross_verified_web);
        assert_eq!(
            plan.tools,
            vec![PlannedTool::WikiQuery, PlannedTool::WebSearch]
        );
    }

    #[test]
    fn task_plan_blocks_private_web_without_override() {
        let plan = TaskPlan::for_chat("latest private note", WebMode::Always, "private:cli", false);
        assert_eq!(plan.intent, TaskIntent::FreshAnswer);
        assert!(!plan.evidence_budget.allow_web);
        assert_eq!(plan.tools, vec![PlannedTool::WikiQuery]);
    }

    #[test]
    fn task_plan_routes_complex_goals_to_workers() {
        let plan = TaskPlan::for_chat("run governance audit", WebMode::Off, "shared:wiki", false);
        assert_eq!(plan.intent, TaskIntent::Governance);
        assert_eq!(plan.workers, vec![WorkerRole::Governance]);
        assert_eq!(
            plan.tools,
            vec![
                PlannedTool::WikiQuery,
                PlannedTool::Worker(WorkerRole::Governance)
            ]
        );
        assert_eq!(plan.risk, TaskRisk::Medium);
    }
}
