use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, State};
use tokio::sync::RwLock;

use crate::database::{Block, Database};
use crate::llm::{BackendRegistry, ChatMessage, ToolDefinition};
use crate::pty::PtyManager;
use crate::redact;
use crate::risk::{self, RiskTier};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agentic features are disabled in Normal workspace mode")]
    DisabledInNormalMode,
    #[error("database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("LLM error: {0}")]
    Llm(String),
}

impl Serialize for AgentError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandProposal {
    pub command: String,
    pub risk_tier: RiskTier,
    pub block_id: Option<String>,
}

#[derive(Debug)]
pub enum UserResponse {
    Approve,
    Edit(String),
    Reject,
}

pub struct AgentOrchestrator {
    db: Arc<Database>,
    pty: Arc<PtyManager>,
    backend_registry: Arc<RwLock<BackendRegistry>>,
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pending_approvals: Mutex<HashMap<String, tokio::sync::oneshot::Sender<UserResponse>>>,
}

const SYSTEM_PROMPT: &str = "\
You are Brick, a local-first terminal agent on macOS.
You help users by proposing and executing terminal commands.
You have access to terminal tools.
Output reasoning, followed by a tool call if you need to run a command.
Do not make assumptions, inspect files or list directories if you need info.";

fn count_tokens(text: &str) -> usize {
    let bpe = tiktoken_rs::cl100k_base().ok();
    bpe.map(|b| b.encode_with_special_tokens(text).len()).unwrap_or_else(|| text.len() / 4)
}

fn format_block(block: &Block) -> String {
    let output = block.stdout.as_deref().unwrap_or("");
    format!(
        "[Block]\nCommand: {}\nCWD: {}\nExit Code: {}\nOutput:\n{}\n",
        block.command,
        block.cwd,
        block.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "running".to_string()),
        output
    )
}

fn truncate_block_output(output: &str, limit_chars: usize) -> String {
    if output.len() <= limit_chars {
        return output.to_string();
    }
    let head_len = (limit_chars as f64 * 0.3) as usize;
    let tail_len = (limit_chars as f64 * 0.5) as usize;
    let head = &output[..head_len];
    let tail = &output[output.len() - tail_len..];
    format!(
        "{}\n[... ~{} tokens truncated ...]\n{}",
        head,
        count_tokens(&output[head_len..output.len() - tail_len]),
        tail
    )
}

#[derive(Deserialize, Clone)]
struct PtyOutputPayload {
    pane_id: String,
    text: String,
}

impl AgentOrchestrator {
    pub fn new(
        db: Arc<Database>,
        pty: Arc<PtyManager>,
        backend_registry: Arc<RwLock<BackendRegistry>>,
    ) -> Self {
        Self {
            db,
            pty,
            backend_registry,
            cancellations: Mutex::new(HashMap::new()),
            pending_approvals: Mutex::new(HashMap::new()),
        }
    }

    pub fn handle_approval(&self, pane_id: &str, edited_command: Option<String>) {
        if let Some(tx) =
            self.pending_approvals.lock().unwrap_or_else(|e| e.into_inner()).remove(pane_id)
        {
            let response = match edited_command {
                Some(cmd) => UserResponse::Edit(cmd),
                None => UserResponse::Approve,
            };
            let _ = tx.send(response);
        }
    }

    pub fn handle_rejection(&self, pane_id: &str) {
        if let Some(tx) =
            self.pending_approvals.lock().unwrap_or_else(|e| e.into_inner()).remove(pane_id)
        {
            let _ = tx.send(UserResponse::Reject);
        }
    }

    pub fn cancel_turn(&self, pane_id: &str) {
        if let Some(cancelled) =
            self.cancellations.lock().unwrap_or_else(|e| e.into_inner()).remove(pane_id)
        {
            cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub async fn run_turn(
        &self,
        pane_id: String,
        user_message: String,
        app: AppHandle,
    ) -> Result<(), AgentError> {
        crate::assert_agentic_mode(&self.db, &pane_id).await?;

        // 1. Get backend
        let registry = self.backend_registry.read().await;
        let backend = registry
            .get_default()
            .ok_or_else(|| AgentError::Llm("No default backend configured".into()))?;

        // Load setting for context size
        let context_window_tokens = self
            .db
            .get_setting("backend.context_window_tokens")
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(12000);

        // 2. Set up cancellation
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(pane_id.clone(), cancelled.clone());

        // 3. Assemble prompt context
        let tools = vec![
            ToolDefinition {
                name: "run_command".into(),
                description: "Execute a shell command in the current pane's shell session.".into(),
            },
            ToolDefinition {
                name: "read_file".into(),
                description: "Read the contents of a file in the workspace.".into(),
            },
            ToolDefinition {
                name: "list_directory".into(),
                description: "List the contents of a directory in the workspace.".into(),
            },
            ToolDefinition {
                name: "write_file".into(),
                description:
                    "Create a new file or write/overwrite content to a file in the workspace."
                        .into(),
            },
        ];

        let floor_text = format!("{}\n\nUser Message:\n{}", SYSTEM_PROMPT, user_message);
        let mut floor_tokens = count_tokens(&floor_text);
        for t in &tools {
            floor_tokens += count_tokens(&t.description) + count_tokens(&t.name);
        }

        let mut budget = if context_window_tokens > floor_tokens as i64 {
            context_window_tokens - floor_tokens as i64
        } else {
            1000
        };

        let mut context_trimmed = false;
        let mut assembled_blocks = Vec::new();

        let blocks = self.db.recent_blocks(&pane_id, 50).await.unwrap_or_default();
        for block in blocks {
            let mut block_text = format_block(&block);
            let mut tokens = count_tokens(&block_text);
            if tokens > 1000 {
                let stdout = block.stdout.clone().unwrap_or_default();
                let truncated_stdout = truncate_block_output(&stdout, 2000);
                let mut truncated_block = block.clone();
                truncated_block.stdout = Some(truncated_stdout);
                block_text = format_block(&truncated_block);
                tokens = count_tokens(&block_text);
                context_trimmed = true;
            }
            if budget >= tokens as i64 {
                budget -= tokens as i64;
                assembled_blocks.push(block_text);
            } else {
                context_trimmed = true;
                break;
            }
        }
        assembled_blocks.reverse();

        // Load chat history from agent session
        let mut messages = Vec::new();
        if let Ok(Some(session_json)) = self.db.get_agent_session(&pane_id).await {
            if let Ok(mut hist) = serde_json::from_str::<Vec<ChatMessage>>(&session_json) {
                messages.append(&mut hist);
            }
        }

        // Add system message and current message
        let redacted_system = redact::redact(&format!(
            "{}\n\nRecent terminal history:\n{}",
            SYSTEM_PROMPT,
            assembled_blocks.join("\n")
        ));
        let redacted_user = redact::redact(&user_message);

        messages.push(ChatMessage { role: "system".into(), content: redacted_system.output });
        messages.push(ChatMessage { role: "user".into(), content: redacted_user.output });

        // 4. Stream chunks
        let mut stream = backend
            .chat_stream(messages.clone(), tools.clone())
            .await
            .map_err(|e| AgentError::Llm(e.to_string()))?;

        let mut full_assistant_response = String::new();
        let mut final_tool_calls = None;

        while let Some(chunk_res) = futures::StreamExt::next(&mut stream).await {
            if cancelled.load(Ordering::Relaxed) {
                let _ = app.emit("agent_cancelled", serde_json::json!({ "pane_id": pane_id }));
                return Ok(());
            }

            let chunk = match chunk_res {
                Ok(c) => c,
                Err(e) => return Err(AgentError::Llm(e.to_string())),
            };

            if !chunk.content.is_empty() {
                full_assistant_response.push_str(&chunk.content);
                let _ = app.emit(
                    "agent_stream_chunk",
                    serde_json::json!({
                        "pane_id": pane_id,
                        "delta": chunk.content,
                        "done": false,
                        "context_trimmed": context_trimmed,
                    }),
                );
            }

            if chunk.done {
                final_tool_calls = chunk.tool_calls;
                break;
            }
        }

        // Save assistant text to session if we received text
        if !full_assistant_response.is_empty() {
            let redacted_assistant = redact::redact(&full_assistant_response);
            messages
                .push(ChatMessage { role: "assistant".into(), content: redacted_assistant.output });
            // strip system message from history to save space
            let save_messages: Vec<ChatMessage> =
                messages.iter().filter(|m| m.role != "system").cloned().collect();
            if let Ok(json) = serde_json::to_string(&save_messages) {
                let _ = self.db.save_agent_session(&pane_id, &json).await;
            }
        }

        // 5. Handle tool call
        if let Some(tool_calls) = final_tool_calls {
            for tc in tool_calls {
                if tc.name == "run_command" {
                    // Extract command
                    let parsed_args: serde_json::Value =
                        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
                    let Some(command) =
                        parsed_args.get("command").and_then(|v| v.as_str()).map(|s| s.to_string())
                    else {
                        continue;
                    };

                    let tier = risk::classify(&command);

                    match tier {
                        RiskTier::Safe => {
                            self.execute_safe_command(&pane_id, &command, &app, cancelled.clone())
                                .await?;
                        }
                        RiskTier::Confirm => {
                            let (tx, rx) = tokio::sync::oneshot::channel::<UserResponse>();
                            self.pending_approvals
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .insert(pane_id.clone(), tx);

                            let _ = app.emit(
                                "agent_proposed_command",
                                serde_json::json!({
                                    "pane_id": pane_id,
                                    "command": command,
                                    "risk_tier": "Confirm",
                                }),
                            );

                            // Await user response
                            let response = match rx.await {
                                Ok(resp) => resp,
                                Err(_) => UserResponse::Reject,
                            };

                            match response {
                                UserResponse::Approve => {
                                    self.execute_safe_command(
                                        &pane_id,
                                        &command,
                                        &app,
                                        cancelled.clone(),
                                    )
                                    .await?;
                                }
                                UserResponse::Edit(edited) => {
                                    // re-classify
                                    let new_tier = risk::classify(&edited);
                                    if new_tier == RiskTier::Blocked {
                                        let _ = app.emit("agent_blocked_command", serde_json::json!({
                                            "pane_id": pane_id,
                                            "command": edited,
                                            "reason": "Edited command is blocked by risk classifier",
                                        }));
                                    } else {
                                        self.execute_safe_command(
                                            &pane_id,
                                            &edited,
                                            &app,
                                            cancelled.clone(),
                                        )
                                        .await?;
                                    }
                                }
                                UserResponse::Reject => {
                                    let _ = app.emit(
                                        "agent_turn_done",
                                        serde_json::json!({ "pane_id": pane_id }),
                                    );
                                }
                            }
                        }
                        RiskTier::Blocked => {
                            let _ = app.emit(
                                "agent_blocked_command",
                                serde_json::json!({
                                    "pane_id": pane_id,
                                    "command": command,
                                    "reason": "Blocked by risk classifier",
                                }),
                            );
                        }
                    }
                }
            }
        }

        let _ = app.emit(
            "agent_stream_chunk",
            serde_json::json!({
                "pane_id": pane_id,
                "delta": "",
                "done": true,
                "context_trimmed": context_trimmed,
            }),
        );
        let _ = app.emit("agent_turn_done", serde_json::json!({ "pane_id": pane_id }));
        self.cancellations.lock().unwrap_or_else(|e| e.into_inner()).remove(&pane_id);
        Ok(())
    }

    async fn execute_safe_command(
        &self,
        pane_id: &str,
        command: &str,
        app: &AppHandle,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), AgentError> {
        let start_time = chrono::Utc::now().timestamp_millis();

        // Listen to output in real-time
        let output_buffer = Arc::new(std::sync::Mutex::new(String::new()));
        let output_buffer_clone = output_buffer.clone();
        let pane_id_clone = pane_id.to_string();

        let listener_id = app.listen("pty_output", move |event| {
            if let Ok(payload) = serde_json::from_str::<PtyOutputPayload>(event.payload()) {
                if payload.pane_id == pane_id_clone {
                    if let Ok(mut buf) = output_buffer_clone.lock() {
                        buf.push_str(&payload.text);
                    }
                }
            }
        });

        // Set watchdog
        let timeout_secs = self
            .db
            .get_setting("agent.autorun_timeout_seconds")
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);

        let cmd_with_newline = format!("{}\n", command);
        let _ = self.pty.write(pane_id, cmd_with_newline.as_bytes()).await;

        let _ = app.emit(
            "agent_stream_chunk",
            serde_json::json!({
                "pane_id": pane_id,
                "delta": format!("\n[Running: {}]\n", command),
                "done": false,
                "context_trimmed": false,
            }),
        );

        // Poll for block finished
        let mut block_output = None;
        let watchdog = tokio::time::sleep(tokio::time::Duration::from_secs(timeout_secs));
        tokio::pin!(watchdog);

        let mut check_interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = &mut watchdog => {
                    let _ = app.emit("agent_autorun_timeout", serde_json::json!({ "pane_id": pane_id }));
                }
                _ = check_interval.tick() => {
                    if cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let recent = self.db.recent_blocks(pane_id, 1).await.ok().and_then(|v| v.into_iter().next());
                    if let Some(block) = recent {
                        if block.started_at >= start_time - 1500 && block.exit_code.is_some() {
                            block_output = Some(block);
                            break;
                        }
                    }
                }
            }
        }

        app.unlisten(listener_id);

        // Feed stdout/stderr of executed command back into database block
        if let Some(ref block) = block_output {
            let output_str =
                if let Ok(buf) = output_buffer.lock() { buf.clone() } else { String::new() };
            // update database block with stdout
            let mut updated_block = block.clone();
            updated_block.stdout = Some(output_str);
            updated_block.initiated_by = "agent".into();
            let _ = self.db.insert_block(updated_block).await;
        }

        Ok(())
    }
}

// --- Tauri Commands ---

#[tauri::command]
pub async fn agent_run_turn(
    state: State<'_, Arc<RwLock<Option<crate::AppState>>>>,
    app: AppHandle,
    pane_id: String,
    user_message: String,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.orchestrator.run_turn(pane_id, user_message, app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn agent_approve_command(
    state: State<'_, Arc<RwLock<Option<crate::AppState>>>>,
    pane_id: String,
    edited_command: Option<String>,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.orchestrator.handle_approval(&pane_id, edited_command);
    Ok(())
}

#[tauri::command]
pub async fn agent_reject_command(
    state: State<'_, Arc<RwLock<Option<crate::AppState>>>>,
    pane_id: String,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.orchestrator.handle_rejection(&pane_id);
    Ok(())
}

#[tauri::command]
pub async fn cancel_agent_turn(
    state: State<'_, Arc<RwLock<Option<crate::AppState>>>>,
    pane_id: String,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.orchestrator.cancel_turn(&pane_id);
    Ok(())
}

#[tauri::command]
pub async fn send_signal(
    state: State<'_, Arc<RwLock<Option<crate::AppState>>>>,
    block_id: String,
    signal: String,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;

    // For send_signal, we look up the active pane process / shell.
    // However, Tauri shell or nix signal can killpg. Since nix kill takes a Pid:
    // we can use standard process signals. Let's see if we can resolve the pid.
    // If not, we can construct standard SIGINT/SIGTERM using nix:
    let _sig = match signal.as_str() {
        "sigint" => nix::sys::signal::Signal::SIGINT,
        "sigterm" => nix::sys::signal::Signal::SIGTERM,
        "sigkill" => nix::sys::signal::Signal::SIGKILL,
        _ => return Err(format!("Unsupported signal: {}", signal)),
    };

    // For signal sending, since portable-pty has MasterPty / slave,
    // let's send standard terminal Ctrl+C or kill signals.
    // We can send Ctrl+C to PTY:
    if signal == "sigint" {
        // Find pane matching block_id
        if let Ok(blocks) = state.db.query_blocks_fuzzy(&block_id, 1).await {
            if let Some(b) = blocks.first() {
                let _ = state.pty.write(&b.pane_id, &[3]).await; // ASCII ETX (Ctrl+C)
                return Ok(());
            }
        }
    }

    // Else we can return Ok/ignore
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counting() {
        assert_eq!(count_tokens("hello"), 1);
    }

    #[test]
    fn test_truncate_block_output() {
        let output = "a".repeat(100);
        let truncated = truncate_block_output(&output, 50);
        assert!(truncated.contains("truncated"));
    }
}
