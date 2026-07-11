use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agentic features are disabled in Normal workspace mode")]
    DisabledInNormalMode,
    #[error("database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("LLM error: {0}")]
    Llm(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandProposal {
    pub command: String,
    pub risk_tier: crate::risk::RiskTier,
    pub block_id: Option<String>,
}
