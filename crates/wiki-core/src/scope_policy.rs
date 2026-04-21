//! 多 agent / 团队 scope 可见性（检索与 lint 共用）。

use crate::model::Scope;

/// 严格隔离：`Private(a)` 仅见同 agent 的私有数据；`Shared(t)` 仅见同 team 的共享数据。
/// 不做「私有用户默认看到团队库」的隐式合并，避免误泄露；需要团队数据时显式使用 `shared:team`。
#[inline]
pub fn document_visible_to_viewer(doc: &Scope, viewer: &Scope) -> bool {
    match (viewer, doc) {
        (
            Scope::Private {
                agent_id: viewer_agent,
            },
            Scope::Private {
                agent_id: doc_agent,
            },
        ) => viewer_agent == doc_agent,
        (
            Scope::Shared {
                team_id: viewer_team,
            },
            Scope::Shared { team_id: doc_team },
        ) => viewer_team == doc_team,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_isolated() {
        let v = Scope::Private {
            agent_id: "a".into(),
        };
        let d = Scope::Private {
            agent_id: "b".into(),
        };
        assert!(!document_visible_to_viewer(&d, &v));
        assert!(document_visible_to_viewer(
            &Scope::Private {
                agent_id: "a".into()
            },
            &v
        ));
    }

    #[test]
    fn shared_team_match() {
        let v = Scope::Shared {
            team_id: "t1".into(),
        };
        assert!(document_visible_to_viewer(
            &Scope::Shared {
                team_id: "t1".into()
            },
            &v
        ));
        assert!(!document_visible_to_viewer(
            &Scope::Shared {
                team_id: "t2".into()
            },
            &v
        ));
    }

    #[test]
    fn private_does_not_see_shared() {
        let v = Scope::Private {
            agent_id: "a".into(),
        };
        let d = Scope::Shared {
            team_id: "t1".into(),
        };
        assert!(!document_visible_to_viewer(&d, &v));
    }
}
