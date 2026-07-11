use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendKind {
    OpenAiCompatible,
    Bedrock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatStreamChunk {
    pub content: String,
    pub done: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("authentication failed")]
    AuthError,
    #[error("stream interrupted")]
    StreamInterrupted,
    #[error("http error: {0}")]
    Http(String),
    #[error("backend not configured")]
    NotConfigured,
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<BoxStream<'static, Result<ChatStreamChunk, LlmError>>, LlmError>;

    fn supports_tools(&self) -> bool;
    fn backend_kind(&self) -> BackendKind;
}
