use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};

use crate::database::Database;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuggestedCommand {
    pub command: String,
    pub score: i64,
    pub run_count: i64,
    pub cwd_local: bool,
}

pub async fn suggest_command(
    db: &Database,
    partial: &str,
    cwd: &str,
) -> anyhow::Result<Vec<SuggestedCommand>> {
    let candidates = db.command_candidates(cwd, 200).await?;
    let matcher = SkimMatcherV2::default();
    let mut scored = Vec::new();
    for candidate in candidates {
        let fuzzy = if partial.trim().is_empty() {
            Some(0)
        } else {
            matcher.fuzzy_match(&candidate.command, partial)
        };
        if let Some(score) = fuzzy {
            let cwd_local = candidate.cwd == cwd;
            scored.push(SuggestedCommand {
                command: candidate.command,
                score: score + if cwd_local { 10_000 } else { 0 },
                run_count: candidate.run_count,
                cwd_local,
            });
        }
    }
    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.run_count.cmp(&a.run_count))
            .then_with(|| a.command.cmp(&b.command))
    });
    scored.truncate(5);
    Ok(scored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[tokio::test]
    async fn ranks_git_status_first() {
        let db = Database::in_memory().await.unwrap();
        db.upsert_command_frequency("git diff", "/x").await.unwrap();
        db.upsert_command_frequency("git status", "/x").await.unwrap();
        db.upsert_command_frequency("git status", "/x").await.unwrap();
        db.upsert_command_frequency("git log", "/x").await.unwrap();
        let suggestions = suggest_command(&db, "git s", "/x").await.unwrap();
        assert_eq!(suggestions[0].command, "git status");
    }
}
