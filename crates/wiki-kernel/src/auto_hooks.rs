use crate::hooks::WikiHook;
use wiki_core::WikiEvent;

pub struct AutoWikiHook {
    pub reinforced_claims: Vec<wiki_core::ClaimId>,
    pub contradictions_flagged: usize,
    pub events_processed: usize,
}

impl AutoWikiHook {
    pub fn new() -> Self {
        Self {
            reinforced_claims: Vec::new(),
            contradictions_flagged: 0,
            events_processed: 0,
        }
    }

    pub fn take_reinforced(&mut self) -> Vec<wiki_core::ClaimId> {
        std::mem::take(&mut self.reinforced_claims)
    }
}

impl WikiHook for AutoWikiHook {
    fn on_event(&mut self, event: &WikiEvent) {
        self.events_processed += 1;
        match event {
            WikiEvent::QueryServed { .. } => {
                // After query, the engine should reinforce claims that appeared in results.
                // We track that the event happened — the engine drives the reinforcement.
            }
            WikiEvent::ClaimUpserted { claim_id, .. } => {
                self.reinforced_claims.push(*claim_id);
            }
            WikiEvent::ClaimSuperseded { old, new, .. } => {
                let _ = (old, new);
            }
            _ => {}
        }
    }
}
