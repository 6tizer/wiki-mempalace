//! 治理：ingest 前脱敏（规则占位，生产可换为 ML/熵检测）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveKind {
    ApiKeyLike,
    BearerToken,
    EmailAddress,
    HighEntropySecret,
    PhoneNumber,
    PrivateMarker,
    CreditCardLike,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionFinding {
    pub kind: SensitiveKind,
    pub placeholder: String,
}

const REDACTED_BEARER: &str = "[REDACTED_BEARER]";
const REDACTED_SECRET: &str = "[REDACTED_SECRET]";
const REDACTED_PRIVATE: &str = "[REDACTED_PRIVATE]";
const REDACTED_EMAIL: &str = "[REDACTED_EMAIL]";
const REDACTED_PHONE: &str = "[REDACTED_PHONE]";
const REDACTED_CARD: &str = "[REDACTED_CARD]";

/// 保守脱敏：常见 token / key / PII 行；命中则整行替换为占位符并记录。
pub fn redact_for_ingest(input: &str) -> (String, Vec<RedactionFinding>) {
    let mut out = String::new();
    let mut findings = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let mut replaced = line.to_string();
        if lower.starts_with("authorization:") || lower.starts_with("bearer ") {
            replaced = REDACTED_BEARER.into();
            findings.push(RedactionFinding {
                kind: SensitiveKind::BearerToken,
                placeholder: REDACTED_BEARER.into(),
            });
        } else if trimmed.contains("AKIA")
            || (lower.contains("api_key") && trimmed.contains('='))
            || trimmed.contains("BEGIN RSA PRIVATE KEY")
            || contains_common_token(trimmed)
        {
            replaced = REDACTED_SECRET.into();
            findings.push(RedactionFinding {
                kind: SensitiveKind::ApiKeyLike,
                placeholder: REDACTED_SECRET.into(),
            });
        } else if trimmed.contains("PRIVATE") && trimmed.contains("DO_NOT_COMMIT") {
            replaced = REDACTED_PRIVATE.into();
            findings.push(RedactionFinding {
                kind: SensitiveKind::PrivateMarker,
                placeholder: REDACTED_PRIVATE.into(),
            });
        } else if contains_email(trimmed) {
            replaced = REDACTED_EMAIL.into();
            findings.push(RedactionFinding {
                kind: SensitiveKind::EmailAddress,
                placeholder: REDACTED_EMAIL.into(),
            });
        } else if contains_luhn_card(trimmed) {
            replaced = REDACTED_CARD.into();
            findings.push(RedactionFinding {
                kind: SensitiveKind::CreditCardLike,
                placeholder: REDACTED_CARD.into(),
            });
        } else if contains_phone_number(trimmed) {
            replaced = REDACTED_PHONE.into();
            findings.push(RedactionFinding {
                kind: SensitiveKind::PhoneNumber,
                placeholder: REDACTED_PHONE.into(),
            });
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&replaced);
    }
    (out, findings)
}

fn contains_common_token(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("sk-")
        || lower.contains("ghp_")
        || lower.contains("github_pat_")
        || lower.contains("xoxb-")
        || lower.contains("xoxp-")
}

fn contains_email(s: &str) -> bool {
    s.split_whitespace().any(|word| {
        let w = word.trim_matches(|c: char| ",;:()[]{}<>\"'".contains(c));
        let Some((local, domain)) = w.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && domain.contains('.')
            && domain
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
    })
}

fn contains_phone_number(s: &str) -> bool {
    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
    if !(10..=15).contains(&digits) {
        return false;
    }
    s.chars().all(|c| {
        c.is_ascii_digit() || c.is_ascii_whitespace() || matches!(c, '+' | '-' | '(' | ')')
    })
}

fn contains_luhn_card(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let mut sum = 0;
    let mut double = false;
    for digit in digits.iter().rev() {
        let mut n = *digit;
        if double {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        double = !double;
    }
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer() {
        let (s, f) = redact_for_ingest("Authorization: Bearer abc.def");
        assert!(s.contains("REDACTED"));
        assert!(!f.is_empty());
    }

    #[test]
    fn redacts_common_tokens_and_pii() {
        for sample in [
            "OPENAI_API_KEY=sk-proj-abc123",
            "github token ghp_abcdefghijklmnopqrstuvwxyz123456",
            "slack=xoxb-123-456",
            "email alice@example.com",
            "+1 (415) 555-0100",
            "4111 1111 1111 1111",
        ] {
            let (s, f) = redact_for_ingest(sample);
            assert!(s.contains("REDACTED"), "{sample}");
            assert_eq!(f.len(), 1, "{sample}");
        }
    }
}
