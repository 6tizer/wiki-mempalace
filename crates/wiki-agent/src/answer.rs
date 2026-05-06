use crate::evidence::EvidencePack;

pub fn build_user_prompt(question: &str, evidence: &EvidencePack) -> String {
    format!(
        "Question:\n{question}\n\nEvidence:\n{}\n\nInternal scores are ranking scores, not confidence percentages. If internal evidence includes titles or excerpts, use that content instead of dismissing it because the score looks numerically small.\n\nAnswer with clear separation between internal wiki evidence and web evidence when relevant.",
        evidence.prompt_context()
    )
}
