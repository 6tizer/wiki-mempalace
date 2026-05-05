use wiki_ai::web_search::{WebSearchEvidence, WebSearchRun};

#[derive(Clone, Debug)]
pub struct InternalEvidence {
    pub doc_id: String,
    pub score: f64,
}

#[derive(Clone, Debug)]
pub struct EvidencePack {
    pub internal: Vec<InternalEvidence>,
    pub web: Option<WebSearchRun>,
    pub web_status: Option<String>,
}

impl EvidencePack {
    pub fn new(internal: Vec<InternalEvidence>) -> Self {
        Self {
            internal,
            web: None,
            web_status: None,
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("internal evidence:\n");
        if self.internal.is_empty() {
            out.push_str("- none\n");
        } else {
            for item in &self.internal {
                out.push_str(&format!("- {} score={:.6}\n", item.doc_id, item.score));
            }
        }
        out.push_str("web evidence:\n");
        if let Some(web) = &self.web {
            for item in &web.evidence {
                out.push_str(&format!(
                    "- [{}] {} ({}) {}\n",
                    item.provider, item.title, item.domain, item.url
                ));
            }
        } else {
            out.push_str(&format!(
                "- {}\n",
                self.web_status.as_deref().unwrap_or("not requested")
            ));
        }
        out
    }

    pub fn prompt_context(&self) -> String {
        let mut out = self.render();
        if let Some(web) = &self.web {
            out.push_str("\nweb snippets:\n");
            for item in web.evidence.iter().take(8) {
                out.push_str(&format!("- {}: {}\n", item.url, snippet_text(item)));
            }
        }
        out
    }
}

fn snippet_text(item: &WebSearchEvidence) -> String {
    item.summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&item.snippet)
        .chars()
        .take(800)
        .collect()
}
