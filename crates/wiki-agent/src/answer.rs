use crate::evidence::EvidencePack;

pub fn build_user_prompt(question: &str, evidence: &EvidencePack) -> String {
    format!(
        "Question:\n{question}\n\nEvidence:\n{}\n\nAnswer with clear separation between internal wiki evidence and web evidence when relevant.",
        evidence.prompt_context()
    )
}
