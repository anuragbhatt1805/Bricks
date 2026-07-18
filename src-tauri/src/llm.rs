use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;

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
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatStreamChunk {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
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

pub fn validate_backend_url(base_url: &str, is_local: bool) -> Result<(), String> {
    let parsed = reqwest::Url::parse(base_url).map_err(|e| format!("Invalid URL format: {}", e))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("Only HTTP or HTTPS schemes are allowed".into());
    }

    let host = parsed.host_str().ok_or("Missing host in URL")?;

    if is_local {
        let is_loopback = host == "localhost"
            || host == "127.0.0.1"
            || host == "[::1]"
            || host.starts_with("127.");
        if !is_loopback {
            return Err("Local backends must use localhost, 127.0.0.1, or loopback address".into());
        }
    } else if scheme != "https" {
        return Err("Remote backends must use secure HTTPS connection".into());
    }

    Ok(())
}

// --- OpenAI Compatible Backend ---

pub struct OpenAiCompatibleBackend {
    pub base_url: String,
    pub model: String,
    pub api_key_ref: Option<String>,
    pub is_local: bool,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiTool>,
}

#[derive(Serialize)]
struct OpenAiTool {
    r#type: &'static str,
    function: OpenAiFunction,
}

#[derive(Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct OpenAiStreamResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    delta: OpenAiDelta,
}

#[derive(Deserialize)]
struct OpenAiDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    index: Option<usize>,
    id: Option<String>,
    function: Option<OpenAiFunctionCall>,
}

#[derive(Deserialize)]
struct OpenAiFunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

struct SseDecoder {
    buffer: String,
}

impl SseDecoder {
    fn new() -> Self {
        Self { buffer: String::new() }
    }

    fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        if let Ok(s) = std::str::from_utf8(chunk) {
            self.buffer.push_str(s);
            while let Some(pos) = self.buffer.find('\n') {
                let line = self.buffer[..pos].trim().to_string();
                self.buffer = self.buffer[pos + 1..].to_string();
                if !line.is_empty() {
                    lines.push(line);
                }
            }
        }
        lines
    }
}

fn get_tool_parameters(name: &str) -> serde_json::Value {
    match name {
        "run_command" => serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
                }
            },
            "required": ["command"]
        }),
        "read_file" => serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read."
                }
            },
            "required": ["path"]
        }),
        "list_directory" => serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the directory to list."
                }
            },
            "required": ["path"]
        }),
        "write_file" => serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to write/edit."
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file."
                }
            },
            "required": ["path", "content"]
        }),
        _ => serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

#[async_trait]
impl LlmBackend for OpenAiCompatibleBackend {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<BoxStream<'static, Result<ChatStreamChunk, LlmError>>, LlmError> {
        if let Err(e) = validate_backend_url(&self.base_url, self.is_local) {
            return Err(LlmError::Http(e));
        }
        let (tx, rx) = mpsc::channel(100);
        let base_url = self.base_url.clone();
        let model = self.model.clone();
        let api_key_ref = self.api_key_ref.clone();
        let supports_tools = self.supports_tools();

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let api_key = if let Some(key_ref) = &api_key_ref {
                if let Ok(entry) = keyring::Entry::new("brick", key_ref) {
                    entry.get_password().ok()
                } else {
                    None
                }
            } else {
                None
            };

            let mut req =
                client.post(format!("{}/v1/chat/completions", base_url)).json(&OpenAiRequest {
                    model,
                    messages,
                    stream: true,
                    tools: if supports_tools {
                        tools
                            .iter()
                            .map(|t| OpenAiTool {
                                r#type: "function",
                                function: OpenAiFunction {
                                    name: t.name.clone(),
                                    description: t.description.clone(),
                                    parameters: get_tool_parameters(&t.name),
                                },
                            })
                            .collect()
                    } else {
                        Vec::new()
                    },
                });

            if let Some(key) = api_key {
                req = req.bearer_auth(key);
            }

            let res = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(LlmError::Http(e.to_string()))).await;
                    return;
                }
            };

            if res.status() == 401 {
                let _ = tx.send(Err(LlmError::AuthError)).await;
                return;
            }

            if !res.status().is_success() {
                let _ =
                    tx.send(Err(LlmError::Http(format!("status code: {}", res.status())))).await;
                return;
            }

            let mut stream = res.bytes_stream();
            let mut decoder = SseDecoder::new();
            let mut accumulated_tool_calls: Vec<ToolCall> = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(_) => {
                        let _ = tx.send(Err(LlmError::StreamInterrupted)).await;
                        return;
                    }
                };
                let lines = decoder.feed(&chunk);
                for line in lines {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            break;
                        }
                        if let Ok(resp) = serde_json::from_str::<OpenAiStreamResponse>(data) {
                            for choice in resp.choices {
                                if let Some(content) = choice.delta.content {
                                    if !content.is_empty() {
                                        let _ = tx
                                            .send(Ok(ChatStreamChunk {
                                                content,
                                                tool_calls: None,
                                                done: false,
                                            }))
                                            .await;
                                    }
                                }
                                if let Some(tool_calls) = choice.delta.tool_calls {
                                    for tc in tool_calls {
                                        let index = tc.index.unwrap_or(0);
                                        while accumulated_tool_calls.len() <= index {
                                            accumulated_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: String::new(),
                                            });
                                        }
                                        let current = &mut accumulated_tool_calls[index];
                                        if let Some(id) = tc.id {
                                            current.id = id;
                                        }
                                        if let Some(func) = tc.function {
                                            if let Some(name) = func.name {
                                                current.name.push_str(&name);
                                            }
                                            if let Some(args) = func.arguments {
                                                current.arguments.push_str(&args);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !accumulated_tool_calls.is_empty() {
                let _ = tx
                    .send(Ok(ChatStreamChunk {
                        content: String::new(),
                        tool_calls: Some(accumulated_tool_calls),
                        done: true,
                    }))
                    .await;
            } else {
                let _ = tx
                    .send(Ok(ChatStreamChunk {
                        content: String::new(),
                        tool_calls: None,
                        done: true,
                    }))
                    .await;
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::OpenAiCompatible
    }
}

// --- AWS Bedrock Backend ---

pub struct BedrockBackend {
    pub client: aws_sdk_bedrockruntime::Client,
    pub model_id: String,
}

#[derive(Serialize)]
struct BedrockAnthropicRequest {
    anthropic_version: &'static str,
    max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<BedrockAnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<BedrockAnthropicTool>,
}

#[derive(Serialize)]
struct BedrockAnthropicMessage {
    role: String,
    content: Vec<BedrockAnthropicContent>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum BedrockAnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Serialize)]
struct BedrockAnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum BedrockStreamEvent {
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: usize, content_block: BedrockContentBlock },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: BedrockDelta },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum BedrockContentBlock {
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum BedrockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_delta")]
    InputDelta { partial_json: String },
    #[serde(other)]
    Other,
}

#[async_trait]
impl LlmBackend for BedrockBackend {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<BoxStream<'static, Result<ChatStreamChunk, LlmError>>, LlmError> {
        let (tx, rx) = mpsc::channel(100);
        let client = self.client.clone();
        let model_id = self.model_id.clone();
        let supports_tools = self.supports_tools();

        tokio::spawn(async move {
            let mut system_prompts = Vec::new();
            let mut anthropic_messages = Vec::new();

            for msg in messages {
                if msg.role == "system" {
                    system_prompts.push(msg.content);
                } else {
                    anthropic_messages.push(BedrockAnthropicMessage {
                        role: msg.role,
                        content: vec![BedrockAnthropicContent::Text { text: msg.content }],
                    });
                }
            }

            let system =
                if system_prompts.is_empty() { None } else { Some(system_prompts.join("\n")) };

            let payload = BedrockAnthropicRequest {
                anthropic_version: "bedrock-2023-05-31",
                max_tokens: 4096,
                system,
                messages: anthropic_messages,
                tools: if supports_tools {
                    tools
                        .iter()
                        .map(|t| BedrockAnthropicTool {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            input_schema: get_tool_parameters(&t.name),
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            };

            let body_bytes = match serde_json::to_vec(&payload) {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(Err(LlmError::Http(e.to_string()))).await;
                    return;
                }
            };

            let response = match client
                .invoke_model_with_response_stream()
                .model_id(&model_id)
                .body(aws_sdk_bedrockruntime::primitives::Blob::new(body_bytes))
                .send()
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    let _ = tx.send(Err(LlmError::Http(e.to_string()))).await;
                    return;
                }
            };

            let mut stream = response.body;
            let mut accumulated_tool_calls: Vec<ToolCall> = Vec::new();

            while let Some(event_res) = stream.recv().await.transpose() {
                let event = match event_res {
                    Ok(e) => e,
                    Err(_) => {
                        let _ = tx.send(Err(LlmError::StreamInterrupted)).await;
                        return;
                    }
                };

                if let aws_sdk_bedrockruntime::types::ResponseStream::Chunk(chunk) = event {
                    if let Some(blob) = chunk.bytes {
                        if let Ok(event_parsed) =
                            serde_json::from_slice::<BedrockStreamEvent>(blob.as_ref())
                        {
                            match event_parsed {
                                BedrockStreamEvent::ContentBlockStart { index, content_block } => {
                                    if let BedrockContentBlock::ToolUse { id, name } = content_block
                                    {
                                        while accumulated_tool_calls.len() <= index {
                                            accumulated_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: String::new(),
                                            });
                                        }
                                        accumulated_tool_calls[index].id = id;
                                        accumulated_tool_calls[index].name = name;
                                    }
                                }
                                BedrockStreamEvent::ContentBlockDelta { index, delta } => {
                                    match delta {
                                        BedrockDelta::TextDelta { text } => {
                                            if !text.is_empty() {
                                                let _ = tx
                                                    .send(Ok(ChatStreamChunk {
                                                        content: text,
                                                        tool_calls: None,
                                                        done: false,
                                                    }))
                                                    .await;
                                            }
                                        }
                                        BedrockDelta::InputDelta { partial_json } => {
                                            while accumulated_tool_calls.len() <= index {
                                                accumulated_tool_calls.push(ToolCall {
                                                    id: String::new(),
                                                    name: String::new(),
                                                    arguments: String::new(),
                                                });
                                            }
                                            accumulated_tool_calls[index]
                                                .arguments
                                                .push_str(&partial_json);
                                        }
                                        BedrockDelta::Other => {}
                                    }
                                }
                                BedrockStreamEvent::Unknown => {}
                            }
                        }
                    }
                }
            }

            if !accumulated_tool_calls.is_empty() {
                let _ = tx
                    .send(Ok(ChatStreamChunk {
                        content: String::new(),
                        tool_calls: Some(accumulated_tool_calls),
                        done: true,
                    }))
                    .await;
            } else {
                let _ = tx
                    .send(Ok(ChatStreamChunk {
                        content: String::new(),
                        tool_calls: None,
                        done: true,
                    }))
                    .await;
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Bedrock
    }
}

// --- Backend Registry ---

pub struct BackendRegistry {
    pub backends: std::collections::HashMap<String, Arc<dyn LlmBackend>>,
    pub default_id: Option<String>,
}

impl BackendRegistry {
    pub async fn load(db: &crate::database::Database) -> anyhow::Result<Self> {
        let configs = db.list_backends().await?;
        let mut backends = std::collections::HashMap::new();
        let mut default_id = None;

        for config in configs {
            if config.is_default {
                default_id = Some(config.id.clone());
            }
            let backend: Arc<dyn LlmBackend> = match config.kind {
                BackendKind::OpenAiCompatible => Arc::new(OpenAiCompatibleBackend {
                    base_url: config.base_url.clone().unwrap_or_default(),
                    model: config.model.clone(),
                    api_key_ref: config.api_key_ref.clone(),
                    is_local: config.is_local,
                }),
                BackendKind::Bedrock => {
                    let sdk_config =
                        aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
                    let client = aws_sdk_bedrockruntime::Client::new(&sdk_config);
                    Arc::new(BedrockBackend { client, model_id: config.model.clone() })
                }
            };
            backends.insert(config.id.clone(), backend);
        }

        Ok(Self { backends, default_id })
    }

    pub fn get_default(&self) -> Option<Arc<dyn LlmBackend>> {
        let id = self.default_id.as_ref()?;
        self.backends.get(id).cloned()
    }

    pub fn get_by_id(&self, id: &str) -> Option<Arc<dyn LlmBackend>> {
        self.backends.get(id).cloned()
    }
}

#[derive(Deserialize)]
struct OllamaShowResponse {
    parameters: Option<String>,
    modelfile: Option<String>,
}

#[tauri::command]
pub async fn test_backend_connection(
    state: State<'_, Arc<RwLock<Option<crate::AppState>>>>,
    backend_id: String,
) -> Result<u64, String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    let config = state
        .db
        .get_backend_by_id(&backend_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("backend not found: {}", backend_id))?;

    // Auto-detect context window if it's Ollama
    if config.kind == BackendKind::OpenAiCompatible {
        if let Some(base_url) = &config.base_url {
            validate_backend_url(base_url, config.is_local)
                .map_err(|e| format!("SSRF Guard: {}", e))?;
            let client = reqwest::Client::new();
            let show_url = format!("{}/api/show", base_url);
            let show_req = client.post(&show_url).json(&serde_json::json!({
                "name": config.model
            }));
            if let Ok(show_res) = show_req.send().await {
                if show_res.status().is_success() {
                    if let Ok(show_data) = show_res.json::<OllamaShowResponse>().await {
                        let mut found_ctx = None;
                        if let Some(params) = show_data.parameters {
                            for line in params.lines() {
                                if line.contains("num_ctx") {
                                    if let Some(val_str) = line.split_whitespace().nth(1) {
                                        if let Ok(val) = val_str.parse::<i64>() {
                                            found_ctx = Some(val);
                                        }
                                    }
                                }
                            }
                        }
                        if found_ctx.is_none() {
                            if let Some(modelfile) = show_data.modelfile {
                                for line in modelfile.lines() {
                                    if line.contains("num_ctx") {
                                        if let Some(val_str) = line.split_whitespace().nth(1) {
                                            if let Ok(val) = val_str.parse::<i64>() {
                                                found_ctx = Some(val);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(ctx) = found_ctx {
                            let mut updated_config = config.clone();
                            updated_config.context_window_tokens = ctx;
                            let _ = state.db.insert_backend(updated_config).await;
                        }
                    }
                }
            }
        }
    }

    let backend: Arc<dyn LlmBackend> = match config.kind {
        BackendKind::OpenAiCompatible => Arc::new(OpenAiCompatibleBackend {
            base_url: config.base_url.unwrap_or_default(),
            model: config.model,
            api_key_ref: config.api_key_ref,
            is_local: config.is_local,
        }),
        BackendKind::Bedrock => {
            let sdk_config =
                aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
            let client = aws_sdk_bedrockruntime::Client::new(&sdk_config);
            Arc::new(BedrockBackend { client, model_id: config.model })
        }
    };

    let start = std::time::Instant::now();
    let messages = vec![ChatMessage { role: "user".into(), content: "ping".into() }];
    let mut stream = backend.chat_stream(messages, vec![]).await.map_err(|e| e.to_string())?;

    while let Some(chunk_res) = stream.next().await {
        let _chunk = chunk_res.map_err(|e| e.to_string())?;
    }

    Ok(start.elapsed().as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_openai_stream_success() {
        let server = MockServer::start().await;

        let response_body = "data: {\"choices\": [{\"delta\": {\"content\": \"hello \"}}]}\n\
                             data: {\"choices\": [{\"delta\": {\"content\": \"world\"}}]}\n\
                             data: {\"choices\": [{\"delta\": {\"content\": \"!\"}}]}\n\
                             data: [DONE]\n";

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
            .mount(&server)
            .await;

        let backend = OpenAiCompatibleBackend {
            base_url: server.uri(),
            model: "test-model".into(),
            api_key_ref: None,
            is_local: true,
        };

        let messages = vec![ChatMessage { role: "user".into(), content: "hi".into() }];

        let mut stream = backend.chat_stream(messages, vec![]).await.unwrap();
        let mut chunks = Vec::new();
        while let Some(res) = stream.next().await {
            let chunk = res.unwrap();
            if !chunk.content.is_empty() {
                chunks.push(chunk.content);
            }
        }

        assert_eq!(chunks, vec!["hello ", "world", "!"]);
    }

    #[tokio::test]
    async fn test_openai_stream_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let backend = OpenAiCompatibleBackend {
            base_url: server.uri(),
            model: "test-model".into(),
            api_key_ref: None,
            is_local: true,
        };

        let messages = vec![ChatMessage { role: "user".into(), content: "hi".into() }];

        let mut stream = backend.chat_stream(messages, vec![]).await.unwrap();
        let res = stream.next().await.unwrap();
        assert!(res.is_err());
        if let Err(err) = res {
            assert!(matches!(err, LlmError::AuthError));
        }
    }

    #[test]
    fn test_url_validation() {
        // Local validation tests
        assert!(validate_backend_url("http://127.0.0.1:11434", true).is_ok());
        assert!(validate_backend_url("http://localhost:11434", true).is_ok());
        assert!(validate_backend_url("http://127.0.0.2:11434", true).is_ok());
        assert!(validate_backend_url("http://[::1]:11434", true).is_ok());
        assert!(validate_backend_url("http://google.com", true).is_err());
        assert!(validate_backend_url("https://127.0.0.1:11434", true).is_ok());

        // Remote validation tests
        assert!(validate_backend_url("https://api.openai.com", false).is_ok());
        assert!(validate_backend_url("http://api.openai.com", false).is_err()); // Remote must be HTTPS
        assert!(validate_backend_url("ftp://api.openai.com", false).is_err());
    }
}
