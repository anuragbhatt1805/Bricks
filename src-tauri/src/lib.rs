pub mod agent;
pub mod database;
pub mod llm;
pub mod prompt_info;
pub mod pty;
pub mod redact;
pub mod risk;
pub mod shell_hook;
pub mod suggestions;
pub mod term_grid;

use std::path::PathBuf;
use std::sync::Arc;

use database::Database;
use pty::PtyManager;
use risk::RiskTier;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub pty: Arc<PtyManager>,
    pub backend_registry: Arc<RwLock<llm::BackendRegistry>>,
    pub orchestrator: Arc<agent::AgentOrchestrator>,
    pub app_data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceMode {
    Normal,
    Agentic,
}

#[tauri::command]
async fn spawn_shell(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    app: tauri::AppHandle,
    shell_path: String,
    cwd: String,
) -> Result<String, String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    let id = state
        .pty
        .spawn_shell(&app, &shell_path, PathBuf::from(cwd.clone()))
        .await
        .map_err(|error| error.to_string())?;
    state
        .db
        .insert_pane(&id, &cwd, &shell_path, WorkspaceMode::Normal)
        .await
        .map_err(|error| error.to_string())?;
    Ok(id)
}

#[tauri::command]
async fn pty_write(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    pane_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.pty.write(&pane_id, &data).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn pty_resize(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    pane_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.pty.resize(&pane_id, cols, rows).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn set_pane_mode(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    app: tauri::AppHandle,
    pane_id: String,
    mode: WorkspaceMode,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.db.update_pane_mode(&pane_id, mode.clone()).await.map_err(|e| e.to_string())?;
    let _ = app.emit("pane_mode_changed", serde_json::json!({ "pane_id": pane_id, "mode": mode }));
    if mode == WorkspaceMode::Normal {
        let _ = app.emit("agent_panel_hidden", serde_json::json!({}));
    }
    Ok(())
}

#[tauri::command]
async fn classify_command(command: String, git_commit_tier: Option<String>) -> RiskTier {
    risk::classify_with_git_commit_tier(&command, git_commit_tier.as_deref())
}

#[tauri::command]
async fn suggest_command(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    partial: String,
    cwd: String,
) -> Result<Vec<suggestions::SuggestedCommand>, String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    suggestions::suggest_command(&state.db, &partial, &cwd).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_setting(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    key: String,
) -> Result<Option<String>, String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.db.get_setting(&key).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn set_setting(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.db.set_setting(&key, &value).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_backends(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
) -> Result<Vec<database::LlmBackendConfig>, String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;
    state.db.list_backends().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_backend(
    state: State<'_, Arc<RwLock<Option<AppState>>>>,
    config: database::LlmBackendConfig,
) -> Result<(), String> {
    let state = state.read().await;
    let state = state.as_ref().ok_or("app state not ready")?;

    // Save to database
    state.db.insert_backend(config).await.map_err(|e| e.to_string())?;

    // Reload registry immediately
    let new_registry = llm::BackendRegistry::load(&state.db).await.map_err(|e| e.to_string())?;
    *state.backend_registry.write().await = new_registry;
    Ok(())
}

#[tauri::command]
async fn redact_text(input: String) -> redact::RedactResult {
    redact::redact(&input)
}

pub fn run() {
    let pending_state: Arc<RwLock<Option<AppState>>> = Arc::new(RwLock::new(None));
    let managed_state = pending_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(managed_state)
        .setup(move |app| {
            let app_data_dir =
                app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".brick-data"));
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("brick.db");
            let db = tauri::async_runtime::block_on(Database::open(db_path))?;
            let db = Arc::new(db);
            let pty = Arc::new(PtyManager::default());
            let registry = tauri::async_runtime::block_on(llm::BackendRegistry::load(&db))?;
            let backend_registry = Arc::new(RwLock::new(registry));
            let orchestrator = Arc::new(agent::AgentOrchestrator::new(
                db.clone(),
                pty.clone(),
                backend_registry.clone(),
            ));
            let app_state = AppState {
                db: db.clone(),
                pty,
                backend_registry,
                orchestrator,
                app_data_dir: app_data_dir.clone(),
            };
            let pending_state = pending_state.clone();
            tauri::async_runtime::block_on(async move {
                *pending_state.write().await = Some(app_state);
            });
            let socket_path = app_data_dir.join("shell.sock");
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(shell_hook::serve(socket_path, db, app_handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            spawn_shell,
            pty_write,
            pty_resize,
            set_pane_mode,
            classify_command,
            suggest_command,
            get_setting,
            set_setting,
            redact_text,
            list_backends,
            save_backend,
            llm::test_backend_connection,
            agent::agent_run_turn,
            agent::agent_approve_command,
            agent::agent_reject_command,
            agent::cancel_agent_turn,
            agent::send_signal
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| eprintln!("error while running Brick: {error}"));
}

pub async fn assert_agentic_mode(db: &Database, pane_id: &str) -> Result<(), agent::AgentError> {
    match db.get_pane_mode(pane_id).await.map_err(agent::AgentError::Database)? {
        Some(WorkspaceMode::Agentic) => Ok(()),
        _ => Err(agent::AgentError::DisabledInNormalMode),
    }
}
