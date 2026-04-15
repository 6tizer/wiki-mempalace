use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const KNOWN_HALLS: [&str; 5] = [
    "hall_facts",
    "hall_events",
    "hall_discoveries",
    "hall_preferences",
    "hall_advice",
];

#[derive(Debug, Clone)]
pub struct Classification {
    pub wing: String,
    pub hall: String,
    pub room: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierRules {
    pub wing_keywords: Vec<KeywordRule>,
    pub hall_keywords: Vec<KeywordRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordRule {
    pub target: String,
    pub keywords: Vec<String>,
}

pub fn default_rules() -> ClassifierRules {
    ClassifierRules {
        wing_keywords: vec![
            KeywordRule {
                target: "wing_project".to_string(),
                keywords: vec!["project".to_string(), "repo".to_string(), "workspace".to_string()],
            },
            KeywordRule {
                target: "wing_ops".to_string(),
                keywords: vec!["deploy".to_string(), "infra".to_string(), "k8s".to_string()],
            },
        ],
        hall_keywords: vec![
            KeywordRule {
                target: "hall_facts".to_string(),
                keywords: vec!["decision".to_string(), "choose".to_string(), "tradeoff".to_string(), "because".to_string()],
            },
            KeywordRule {
                target: "hall_events".to_string(),
                keywords: vec!["incident".to_string(), "outage".to_string(), "retrospective".to_string(), "timeline".to_string()],
            },
            KeywordRule {
                target: "hall_discoveries".to_string(),
                keywords: vec!["learned".to_string(), "insight".to_string(), "discovered".to_string(), "breakthrough".to_string()],
            },
            KeywordRule {
                target: "hall_preferences".to_string(),
                keywords: vec!["prefer".to_string(), "style".to_string(), "like".to_string(), "opinion".to_string()],
            },
            KeywordRule {
                target: "hall_advice".to_string(),
                keywords: vec!["recommend".to_string(), "suggest".to_string(), "should".to_string(), "best practice".to_string()],
            },
        ],
    }
}

pub fn load_rules(path: &Path) -> Option<ClassifierRules> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn classify(path: &Path, content: &str, rules: Option<&ClassifierRules>) -> Classification {
    let path_s = path.to_string_lossy().to_lowercase();
    let text = content.to_lowercase();
    let wing = infer_wing(&path_s, &text, rules);
    let hall = infer_hall(&path_s, &text, rules);
    let room = infer_room(&path_s);
    Classification { wing, hall, room }
}

fn infer_wing(path_s: &str, text: &str, rules: Option<&ClassifierRules>) -> String {
    if let Some(rules) = rules {
        for rule in &rules.wing_keywords {
            if rule
                .keywords
                .iter()
                .any(|k| path_s.contains(k) || text.contains(k))
            {
                return rule.target.clone();
            }
        }
    }
    let candidates = ["project", "workspace", "repo", "service", "app"];
    for c in candidates {
        if path_s.contains(c) || text.contains(c) {
            return format!("wing_{c}");
        }
    }
    "wing_general".to_string()
}

fn infer_hall(path_s: &str, text: &str, rules: Option<&ClassifierRules>) -> String {
    if let Some(rules) = rules {
        for rule in &rules.hall_keywords {
            if rule
                .keywords
                .iter()
                .any(|k| path_s.contains(k) || text.contains(k))
            {
                return rule.target.clone();
            }
        }
    }
    if contains_any(path_s, text, &["decision", "choose", "tradeoff", "because"]) {
        return "hall_facts".to_string();
    }
    if contains_any(path_s, text, &["incident", "outage", "retrospective", "timeline"]) {
        return "hall_events".to_string();
    }
    if contains_any(path_s, text, &["learned", "insight", "discovered", "breakthrough"]) {
        return "hall_discoveries".to_string();
    }
    if contains_any(path_s, text, &["prefer", "style", "like", "opinion"]) {
        return "hall_preferences".to_string();
    }
    if contains_any(path_s, text, &["recommend", "suggest", "should", "best practice"]) {
        return "hall_advice".to_string();
    }
    "hall_events".to_string()
}

fn infer_room(path_s: &str) -> String {
    let file_stem = path_s
        .rsplit('/')
        .next()
        .unwrap_or("general-room")
        .split('.')
        .next()
        .unwrap_or("general-room");
    normalize_slug(file_stem)
}

fn normalize_slug(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn contains_any(path_s: &str, text: &str, words: &[&str]) -> bool {
    words
        .iter()
        .any(|w| path_s.contains(w) || text.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hall_is_detected_from_words() {
        let c = classify(
            Path::new("/tmp/notes/decision-auth.md"),
            "we choose clerk because migration cost is low",
            None,
        );
        assert_eq!(c.hall, "hall_facts");
    }
}
