use crate::roles::WorkerRole;
use crate::task::AgentTask;

#[derive(Default)]
pub struct Manager;

impl Manager {
    pub fn plan(&self, goal: &str, override_role: Option<WorkerRole>, apply: bool) -> AgentTask {
        let role = override_role.unwrap_or_else(|| route_goal(goal));
        AgentTask::new(goal, role, apply)
    }
}

fn route_goal(goal: &str) -> WorkerRole {
    let lower = goal.to_ascii_lowercase();
    if contains_any(goal, &["综合", "合成", "综合分析"]) || lower.contains("synthesis") {
        return WorkerRole::Synthesis;
    }
    if contains_any(goal, &["修复", "修一下"]) || lower.contains("fix") || lower.contains("repair")
    {
        return WorkerRole::Fixer;
    }
    if contains_any(goal, &["治理", "审计", "健康"]) || lower.contains("governance") {
        return WorkerRole::Governance;
    }
    if contains_any(goal, &["记忆", "技能"]) || lower.contains("memory") || lower.contains("skill")
    {
        return WorkerRole::MemoryCurator;
    }
    if contains_any(goal, &["搜索", "查询", "最新", "最近"]) || lower.contains("search") {
        return WorkerRole::Search;
    }
    if lower.contains("lint") || contains_any(goal, &["检查", "格式"]) {
        return WorkerRole::Lint;
    }
    WorkerRole::Search
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_common_prompts_to_expected_roles() {
        assert_eq!(
            Manager.plan("lint the wiki", None, false).role,
            WorkerRole::Lint
        );
        assert_eq!(
            Manager.plan("做一次治理审计", None, false).role,
            WorkerRole::Governance
        );
        assert_eq!(
            Manager.plan("修复安全问题", None, false).role,
            WorkerRole::Fixer
        );
        assert_eq!(
            Manager.plan("综合分析标签", None, false).role,
            WorkerRole::Synthesis
        );
        assert_eq!(
            Manager.plan("搜索最新资料", None, false).role,
            WorkerRole::Search
        );
        assert_eq!(
            Manager.plan("提炼技能", None, false).role,
            WorkerRole::MemoryCurator
        );
    }
}
