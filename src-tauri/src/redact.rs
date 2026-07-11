use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactionEvent {
    pub kind: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactResult {
    pub output: String,
    pub redactions: Vec<RedactionEvent>,
}

pub fn redact(input: &str) -> RedactResult {
    let mut output = input.to_string();
    let mut redactions = Vec::new();
    for (pattern, replacement, kind) in [
        (r"AKIA[0-9A-Z]{16}", "[REDACTED:AWS_ACCESS_KEY]", "AWS_ACCESS_KEY"),
        (
            r"(?is)aws_secret(?:_access_key)?\s*[=:]\s*\S+",
            "[REDACTED:AWS_SECRET_KEY]",
            "AWS_SECRET_KEY",
        ),
        (
            r"(?i)(SECRET|TOKEN|PASSWORD|API_KEY|PRIVATE_KEY)\s*=\s*\S+",
            "[REDACTED:ENV_SECRET]",
            "ENV_SECRET",
        ),
        (
            r"(ghp_[A-Za-z0-9_]+|gho_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+)",
            "[REDACTED:GITHUB_TOKEN]",
            "GITHUB_TOKEN",
        ),
        (
            r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
            "[REDACTED:PRIVATE_KEY]",
            "PRIVATE_KEY",
        ),
    ] {
        output = replace_all(&output, pattern, replacement, kind, &mut redactions);
    }

    if let Ok(token_re) = Regex::new(r"[A-Za-z0-9+/=_-]{21,}") {
        let snapshot = output.clone();
        for mat in token_re.find_iter(&snapshot) {
            let token = mat.as_str();
            if entropy(token) > 4.5 && !token.starts_with("[REDACTED:") {
                redactions.push(RedactionEvent {
                    kind: "HIGH_ENTROPY".into(),
                    start: mat.start(),
                    end: mat.end(),
                });
                output = output.replace(token, "[FLAGGED:HIGH_ENTROPY]");
            }
        }
    }

    RedactResult { output, redactions }
}

fn replace_all(
    input: &str,
    pattern: &str,
    replacement: &str,
    kind: &str,
    redactions: &mut Vec<RedactionEvent>,
) -> String {
    let Ok(re) = Regex::new(pattern) else {
        return input.to_string();
    };
    for mat in re.find_iter(input) {
        redactions.push(RedactionEvent { kind: kind.into(), start: mat.start(), end: mat.end() });
    }
    re.replace_all(input, replacement).to_string()
}

fn entropy(token: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for byte in token.bytes() {
        *counts.entry(byte).or_insert(0_usize) += 1;
    }
    let len = token.len() as f64;
    counts
        .values()
        .map(|count| {
            let p = *count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

pub struct PromptBuilder {
    parts: Vec<String>,
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    pub fn push_redacted(mut self, value: &RedactResult) -> Self {
        self.parts.push(value.output.clone());
        self
    }

    pub fn build(self) -> String {
        self.parts.join("\n")
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_secret_shapes() {
        assert!(redact("AWS_ACCESS_KEY_ID=AKIA1234567890ABCDEF")
            .output
            .contains("[REDACTED:AWS_ACCESS_KEY]"));
        assert!(redact("MY_TOKEN=abc123").output.contains("[REDACTED:ENV_SECRET]"));
        assert!(redact("token ghp_abcdefghijklmnopqrstuvwxyz")
            .output
            .contains("[REDACTED:GITHUB_TOKEN]"));
        assert!(redact("-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----")
            .output
            .contains("[REDACTED:PRIVATE_KEY]"));
        assert_eq!(redact("const x = 1 + 2").output, "const x = 1 + 2");
        assert!(redact("QWxhZGRpbjpvcGVuIHNlc2FtZQAB9xZ4Kq7Vm2P0LzR8Yt3N")
            .output
            .contains("[FLAGGED:HIGH_ENTROPY]"));
    }
}
