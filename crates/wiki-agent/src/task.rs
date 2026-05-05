use crate::roles::WorkerRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentTask {
    pub task_id: String,
    pub goal: String,
    pub role: WorkerRole,
    pub apply: bool,
}

impl AgentTask {
    pub fn new(goal: impl Into<String>, role: WorkerRole, apply: bool) -> Self {
        Self {
            task_id: uuid::Uuid::new_v4().to_string(),
            goal: goal.into(),
            role,
            apply,
        }
    }
}
