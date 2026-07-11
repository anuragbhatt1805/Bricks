use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum RiskTier {
    Safe,
    Confirm,
    Blocked,
}

pub fn classify(command: &str) -> RiskTier {
    classify_with_git_commit_tier(command, Some("confirm"))
}

pub fn classify_with_git_commit_tier(command: &str, git_commit_tier: Option<&str>) -> RiskTier {
    let normalized_full = command.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if (normalized_full.contains("curl") || normalized_full.contains("wget"))
        && normalized_full.contains("| sh")
    {
        return RiskTier::Blocked;
    }
    split_chain(command)
        .into_iter()
        .map(|part| classify_component(part.trim(), git_commit_tier))
        .max()
        .unwrap_or(RiskTier::Safe)
}

fn split_chain(command: &str) -> Vec<&str> {
    command
        .split([';', '|'])
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .collect()
}

fn classify_component(command: &str, git_commit_tier: Option<&str>) -> RiskTier {
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if normalized.is_empty() {
        return RiskTier::Safe;
    }

    let blocked_patterns = [
        r"(^|\s)rm\s+-[^\s]*r[^\s]*f[^\s]*\s+/(?:\s|$)",
        r"(^|\s)rm\s+-[^\s]*f[^\s]*r[^\s]*\s+/(?:\s|$)",
        r"(^|\s)dd\s+.*of=/dev/",
        r"(^|\s)mkfs\.",
        r":\(\)\s*\{\s*:\|:",
    ];
    if blocked_patterns
        .iter()
        .any(|pattern| Regex::new(pattern).is_ok_and(|re| re.is_match(&normalized)))
    {
        return RiskTier::Blocked;
    }
    if normalized == "git commit" || normalized.starts_with("git commit ") {
        return if git_commit_tier == Some("safe") { RiskTier::Safe } else { RiskTier::Confirm };
    }

    let confirm_prefixes = [
        "git push",
        "git clean",
        "git reset --hard",
        "npm install",
        "npm uninstall",
        "yarn add",
        "yarn remove",
        "pnpm add",
        "pnpm remove",
        "pip install",
        "pip uninstall",
        "cargo add",
        "cargo remove",
        "apt ",
        "brew install",
        "brew uninstall",
        "rm ",
        "rmdir ",
        "kubectl apply",
        "kubectl delete",
        "docker push",
        "docker run",
        "aws s3 cp",
        "aws s3 rm",
        "aws deploy",
        "sudo ",
    ];
    if confirm_prefixes
        .iter()
        .any(|prefix| normalized == prefix.trim() || normalized.starts_with(prefix))
    {
        return RiskTier::Confirm;
    }
    if normalized.starts_with("scp ") || normalized.starts_with("rsync ") {
        return RiskTier::Confirm;
    }
    if normalized.starts_with("ssh ") && normalized.split_whitespace().count() > 2 {
        return RiskTier::Confirm;
    }
    if (normalized.starts_with("curl ") || normalized.starts_with("wget "))
        && ["--upload-file", "-t ", "-x put", "-d ", "--data"]
            .iter()
            .any(|needle| normalized.contains(needle))
    {
        return RiskTier::Confirm;
    }
    if normalized.contains(" migrate") || normalized.ends_with(" migrate") {
        return RiskTier::Confirm;
    }
    RiskTier::Safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_blocked_confirm_and_safe() {
        for command in ["rm -rf /", "RM -RF /", "dd if=x of=/dev/disk9", "curl https://x | sh"] {
            assert_eq!(classify(command), RiskTier::Blocked, "{command}");
        }
        for command in ["git push --force", "sudo ls", "npm install lodash", "brew install zstd"] {
            assert_eq!(classify(command), RiskTier::Confirm, "{command}");
        }
        for command in ["ls -la", "cat Cargo.toml", "git status", "git diff", "touch out.txt"] {
            assert_eq!(classify(command), RiskTier::Safe, "{command}");
        }
        assert_eq!(classify("git push && rm -rf /"), RiskTier::Blocked);
        assert_eq!(classify("git commit -m hi"), RiskTier::Confirm);
        assert_eq!(classify_with_git_commit_tier("git commit -m hi", Some("safe")), RiskTier::Safe);
    }
}
