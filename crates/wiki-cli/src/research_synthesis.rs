use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, Confidence, EntryStatus, EntryType, PageContract, Scope,
    SynthesisCandidate, SynthesisDiscoveryReport, WikiPage,
};
use wiki_kernel::InMemoryStore;

use crate::{llm, web_search};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisComposeStatus {
    Ready,
    Applied,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalEvidenceItem {
    pub page_id: String,
    pub title: String,
    pub entry_type: Option<EntryType>,
    pub status: EntryStatus,
    pub source_url: Option<String>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisDraftFinding {
    pub text: String,
    #[serde(default)]
    pub citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisDraft {
    pub title: String,
    pub research_question: String,
    pub comprehensive_analysis: String,
    #[serde(default)]
    pub key_findings: Vec<SynthesisDraftFinding>,
    #[serde(default)]
    pub action_recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisVerifierVerdict {
    pub approved: bool,
    #[serde(default)]
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisComposeReport {
    pub report_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub source_discovery_report_id: String,
    pub candidate_id: String,
    pub status: SynthesisComposeStatus,
    pub blockers: Vec<String>,
    pub page_id: Option<String>,
    pub candidate: SynthesisCandidate,
    pub internal_evidence: Vec<InternalEvidenceItem>,
    pub web_runs: Vec<web_search::WebSearchRun>,
    pub draft: Option<SynthesisDraft>,
    pub verifier: Option<SynthesisVerifierVerdict>,
}

pub struct ComposeFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

pub fn synthesis_compose_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-synthesis-compose",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub fn read_discovery_report(
    path: &Path,
) -> Result<SynthesisDiscoveryReport, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

pub fn select_candidate<'a>(
    report: &'a SynthesisDiscoveryReport,
    candidate_id: &str,
) -> Result<&'a SynthesisCandidate, Box<dyn std::error::Error>> {
    report
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .ok_or_else(|| format!("candidate not found in discovery report: {candidate_id}").into())
}

pub fn build_internal_evidence(
    store: &InMemoryStore,
    viewer: &Scope,
    candidate: &SynthesisCandidate,
) -> Vec<InternalEvidenceItem> {
    let wanted: BTreeSet<_> = candidate.page_ids.iter().cloned().collect();
    let mut items: Vec<_> = store
        .pages
        .values()
        .filter(|page| wanted.contains(&page.id.0.to_string()))
        .filter(|page| document_visible_to_viewer(&page.scope, viewer))
        .map(|page| InternalEvidenceItem {
            page_id: page.id.0.to_string(),
            title: page.title.clone(),
            entry_type: page.entry_type.clone(),
            status: page.status,
            source_url: page.source_url.clone(),
            excerpt: truncate_chars(&page.markdown, 1_200),
        })
        .collect();
    items.sort_by(|a, b| {
        a.title
            .cmp(&b.title)
            .then_with(|| a.page_id.cmp(&b.page_id))
    });
    items
}

pub fn read_web_runs(
    path: &Path,
) -> Result<Vec<web_search::WebSearchRun>, Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    if value.is_array() {
        Ok(serde_json::from_value(value)?)
    } else {
        Ok(vec![serde_json::from_value(value)?])
    }
}

pub fn read_draft(path: &Path) -> Result<SynthesisDraft, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(llm::parse_json_object_slice(&raw))?)
}

pub fn read_verifier(path: &Path) -> Result<SynthesisVerifierVerdict, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(llm::parse_json_object_slice(&raw))?)
}

pub fn run_web_research(
    llm_config_path: &Path,
    candidate: &SynthesisCandidate,
    internal_evidence: &[InternalEvidenceItem],
) -> Result<Vec<web_search::WebSearchRun>, Box<dyn std::error::Error>> {
    let app = llm::load_app_config(llm_config_path)?;
    let research_cfg = llm::resolve_llm_profile(&app, "synthesis_research")?;
    let query_prompt = research_query_prompt(candidate, internal_evidence);
    let query_reply = llm::complete_chat_json_object(
        &research_cfg,
        research_query_system_prompt(),
        &query_prompt,
        research_cfg.max_output_tokens.min(4096),
    )?;
    let queries = parse_research_queries(&query_reply)?;
    let mut runs = Vec::new();
    for query in queries {
        runs.push(web_search::run_search(&app, &[], &query)?);
    }
    Ok(runs)
}

pub fn generate_draft_with_llm(
    llm_config_path: &Path,
    candidate: &SynthesisCandidate,
    internal_evidence: &[InternalEvidenceItem],
    web_runs: &[web_search::WebSearchRun],
) -> Result<SynthesisDraft, Box<dyn std::error::Error>> {
    let writer_cfg = llm::load_llm_profile_config(llm_config_path, Some("synthesis_writer"))?;
    let prompt = compose_prompt(candidate, internal_evidence, web_runs);
    let reply = llm::complete_chat_json_object(
        &writer_cfg,
        writer_system_prompt(),
        &prompt,
        writer_cfg.max_output_tokens.min(24_000),
    )?;
    Ok(serde_json::from_str(llm::parse_json_object_slice(&reply))?)
}

pub fn verify_with_llm(
    llm_config_path: &Path,
    draft: &SynthesisDraft,
    candidate: &SynthesisCandidate,
    internal_evidence: &[InternalEvidenceItem],
    web_runs: &[web_search::WebSearchRun],
) -> Result<SynthesisVerifierVerdict, Box<dyn std::error::Error>> {
    let verifier_cfg = llm::load_llm_profile_config(llm_config_path, Some("synthesis_verifier"))?;
    let prompt = verifier_prompt(draft, candidate, internal_evidence, web_runs)?;
    let reply = llm::complete_chat_json_object(
        &verifier_cfg,
        verifier_system_prompt(),
        &prompt,
        verifier_cfg.max_output_tokens.min(4096),
    )?;
    Ok(serde_json::from_str(llm::parse_json_object_slice(&reply))?)
}

pub fn build_compose_report(
    report_id: String,
    generated_at: OffsetDateTime,
    discovery_report: &SynthesisDiscoveryReport,
    candidate: SynthesisCandidate,
    internal_evidence: Vec<InternalEvidenceItem>,
    web_runs: Vec<web_search::WebSearchRun>,
    draft: Option<SynthesisDraft>,
    verifier: Option<SynthesisVerifierVerdict>,
    apply: bool,
    page_id: Option<String>,
    internal_only: bool,
) -> SynthesisComposeReport {
    let mut blockers = Vec::new();
    if !internal_only {
        validate_web_evidence(&web_runs, &mut blockers);
    }
    if let Some(draft) = &draft {
        validate_draft_citations(draft, &internal_evidence, &web_runs, &mut blockers);
    } else {
        blockers.push("missing synthesis draft".into());
    }
    if let Some(verifier) = &verifier {
        if !verifier.approved {
            blockers.extend(verifier.blockers.clone());
            if verifier.blockers.is_empty() {
                blockers.push("verifier rejected draft".into());
            }
        }
    } else {
        blockers.push("missing verifier verdict".into());
    }
    let status = if blockers.is_empty() {
        if apply {
            SynthesisComposeStatus::Applied
        } else {
            SynthesisComposeStatus::Ready
        }
    } else {
        SynthesisComposeStatus::Blocked
    };
    SynthesisComposeReport {
        report_id,
        generated_at: Some(generated_at),
        source_discovery_report_id: discovery_report.report_id.clone(),
        candidate_id: candidate.candidate_id.clone(),
        status,
        blockers,
        page_id,
        candidate,
        internal_evidence,
        web_runs,
        draft,
        verifier,
    }
}

pub fn build_synthesis_page(
    candidate: &SynthesisCandidate,
    draft: &SynthesisDraft,
    scope: Scope,
) -> WikiPage {
    PageContract::new(clean_title(&draft.title, candidate), EntryType::Synthesis)
        .with_confidence(Confidence::High)
        .with_tags(candidate.tags.clone())
        .with_source("research-synthesis")
        .with_section("研究问题", draft.research_question.trim())
        .with_section("综合分析", draft.comprehensive_analysis.trim())
        .with_section("关键发现", render_findings(&draft.key_findings))
        .with_section("来源列表", render_source_list(draft))
        .with_section("外部证据", render_external_citations(draft))
        .with_section(
            "行动建议",
            render_recommendations(&draft.action_recommendations),
        )
        .into_page(scope, EntryStatus::InReview)
}

pub fn render_compose_text(report: &SynthesisComposeReport) -> String {
    format!(
        "synthesis compose: report_id={} candidate={} status={:?} blockers={} page={}\n",
        report.report_id,
        report.candidate_id,
        report.status,
        report.blockers.len(),
        report.page_id.as_deref().unwrap_or("none")
    )
}

pub fn write_compose_files(
    report: &SynthesisComposeReport,
    report_dir: &Path,
) -> Result<ComposeFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let json_name = format!("{}.json", report.report_id);
    let markdown_name = format!("{}.md", report.report_id);
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    std::fs::write(&markdown_path, render_compose_markdown(report, &json_name))?;
    Ok(ComposeFiles {
        json_path,
        markdown_path,
    })
}

fn validate_web_evidence(web_runs: &[web_search::WebSearchRun], blockers: &mut Vec<String>) {
    if web_runs.is_empty() {
        blockers.push("web evidence required unless --internal-only is set".into());
        return;
    }
    for run in web_runs {
        if !run.cross_verified {
            blockers.push(format!(
                "web query is not cross-verified: {} providers={} domains={}",
                run.query,
                run.providers_succeeded.len(),
                run.distinct_domains
            ));
        }
    }
}

fn validate_draft_citations(
    draft: &SynthesisDraft,
    internal_evidence: &[InternalEvidenceItem],
    web_runs: &[web_search::WebSearchRun],
    blockers: &mut Vec<String>,
) {
    let allowed = allowed_citations(internal_evidence, web_runs);
    if draft.key_findings.is_empty() {
        blockers.push("draft must contain key_findings".into());
    }
    for (idx, finding) in draft.key_findings.iter().enumerate() {
        if finding.text.trim().is_empty() {
            blockers.push(format!("key finding {} is empty", idx + 1));
        }
        if finding.citations.is_empty() {
            blockers.push(format!("key finding {} has no citations", idx + 1));
        }
        for citation in &finding.citations {
            if !allowed.contains(citation) {
                blockers.push(format!(
                    "key finding {} has unknown citation: {citation}",
                    idx + 1
                ));
            }
        }
    }
}

fn allowed_citations(
    internal_evidence: &[InternalEvidenceItem],
    web_runs: &[web_search::WebSearchRun],
) -> BTreeSet<String> {
    let mut citations = BTreeSet::new();
    for item in internal_evidence {
        citations.insert(format!("internal:{}", item.page_id));
    }
    for run in web_runs {
        for item in &run.evidence {
            citations.insert(format!("web:{}", item.content_hash));
            citations.insert(format!("web:{}", item.url));
        }
    }
    citations
}

fn parse_research_queries(raw: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    struct QuerySet {
        #[serde(default)]
        queries: Vec<String>,
    }
    let parsed: QuerySet = serde_json::from_str(llm::parse_json_object_slice(raw))?;
    let mut queries: Vec<_> = parsed
        .queries
        .into_iter()
        .map(|query| query.trim().to_string())
        .filter(|query| !query.is_empty())
        .take(6)
        .collect();
    queries.sort();
    queries.dedup();
    if queries.is_empty() {
        return Err("synthesis_research produced no queries".into());
    }
    Ok(queries)
}

fn research_query_system_prompt() -> &'static str {
    "你是研究型检索规划器。只输出 JSON：{\"queries\":[...]}。每个 query 用中文，覆盖 validation、bridge、freshness 三类，不要包含私密原文。"
}

fn research_query_prompt(
    candidate: &SynthesisCandidate,
    internal_evidence: &[InternalEvidenceItem],
) -> String {
    format!(
        "候选标签: {}\n候选类型: {:?}\n内部页面标题: {}\n请生成 3-6 个外部搜索 query。",
        candidate.tags.join(", "),
        candidate.kind,
        internal_evidence
            .iter()
            .map(|item| item.title.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

fn writer_system_prompt() -> &'static str {
    "你是中文知识综合研究员。只输出 JSON。每个 key_findings 必须带 citations，引用只能使用提供的 internal:* 或 web:* id。不要编造来源。"
}

fn compose_prompt(
    candidate: &SynthesisCandidate,
    internal_evidence: &[InternalEvidenceItem],
    web_runs: &[web_search::WebSearchRun],
) -> String {
    serde_json::json!({
        "required_shape": {
            "title": "string",
            "research_question": "string",
            "comprehensive_analysis": "string",
            "key_findings": [{"text": "string", "citations": ["internal:<page_id>|web:<content_hash>"]}],
            "action_recommendations": ["string"]
        },
        "candidate": candidate,
        "internal_evidence": internal_evidence,
        "web_evidence": web_runs
    })
    .to_string()
}

fn verifier_system_prompt() -> &'static str {
    "你是严格校验器。只输出 JSON：{\"approved\":true|false,\"blockers\":[...]}。若关键发现缺来源、引用不存在、四标签没有结构同构/隐喻迁移/范式冲突，必须 rejected。"
}

fn verifier_prompt(
    draft: &SynthesisDraft,
    candidate: &SynthesisCandidate,
    internal_evidence: &[InternalEvidenceItem],
    web_runs: &[web_search::WebSearchRun],
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(serde_json::to_string(&serde_json::json!({
        "draft": draft,
        "candidate": candidate,
        "internal_evidence": internal_evidence,
        "web_evidence": web_runs
    }))?)
}

fn render_findings(findings: &[SynthesisDraftFinding]) -> String {
    findings
        .iter()
        .map(|finding| {
            format!(
                "- {} ({})",
                finding.text.trim(),
                finding.citations.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_source_list(draft: &SynthesisDraft) -> String {
    let mut citations = draft
        .key_findings
        .iter()
        .flat_map(|finding| finding.citations.clone())
        .collect::<Vec<_>>();
    citations.sort();
    citations.dedup();
    citations
        .into_iter()
        .map(|citation| format!("- {citation}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_external_citations(draft: &SynthesisDraft) -> String {
    let mut citations = draft
        .key_findings
        .iter()
        .flat_map(|finding| finding.citations.iter())
        .filter(|citation| citation.starts_with("web:"))
        .cloned()
        .collect::<Vec<_>>();
    citations.sort();
    citations.dedup();
    if citations.is_empty() {
        "无外部证据。".into()
    } else {
        citations
            .into_iter()
            .map(|citation| format!("- {citation}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn render_recommendations(items: &[String]) -> String {
    if items.is_empty() {
        "暂无。".into()
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn render_compose_markdown(report: &SynthesisComposeReport, sibling_json: &str) -> String {
    let mut out = format!(
        concat!(
            "# Synthesis Compose\n\n",
            "- report_id: `{}`\n",
            "- candidate_id: `{}`\n",
            "- status: `{:?}`\n",
            "- page_id: `{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth.\n\n",
            "## Blockers\n\n"
        ),
        report.report_id,
        report.candidate_id,
        report.status,
        report.page_id.as_deref().unwrap_or("none"),
        sibling_json,
        sibling_json
    );
    if report.blockers.is_empty() {
        out.push_str("No blockers.\n\n");
    } else {
        for blocker in &report.blockers {
            out.push_str(&format!("- {blocker}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Candidate\n\n");
    out.push_str(&format!("- tags: `{}`\n", report.candidate.tags.join(",")));
    out.push_str(&format!("- kind: `{:?}`\n\n", report.candidate.kind));
    out.push_str("## Draft\n\n");
    if let Some(draft) = &report.draft {
        out.push_str(&format!("{}\n", draft.title));
    } else {
        out.push_str("No draft.\n");
    }
    out
}

fn clean_title(title: &str, candidate: &SynthesisCandidate) -> String {
    let title = title.trim();
    if title.is_empty() {
        format!("综合研究：{}", candidate.tags.join(" / "))
    } else {
        title.to_string()
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_validation_rejects_unknown_citation() {
        let draft = SynthesisDraft {
            title: "t".into(),
            research_question: "q".into(),
            comprehensive_analysis: "a".into(),
            key_findings: vec![SynthesisDraftFinding {
                text: "finding".into(),
                citations: vec!["web:missing".into()],
            }],
            action_recommendations: Vec::new(),
        };
        let mut blockers = Vec::new();
        validate_draft_citations(&draft, &[], &[], &mut blockers);
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("unknown citation")));
    }
}
