use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::database::{Block, Database};

#[derive(Debug, Deserialize)]
struct HookPayload {
    pane_id: Option<String>,
    command: String,
    cwd: String,
    exit_code: i32,
    duration_ms: i64,
    started_at: i64,
    git_branch: Option<String>,
    git_dirty: bool,
    shell: String,
}

pub async fn serve(path: PathBuf, db: Arc<Database>, app: AppHandle) {
    let _ = std::fs::remove_file(&path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
            listener
        }
        Err(error) => {
            eprintln!("failed to bind shell hook socket {}: {error}", path.display());
            return;
        }
    };
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let db = db.clone();
                let app = app.clone();
                tokio::spawn(async move {
                    handle_stream(stream, db, app).await;
                });
            }
            Err(error) => eprintln!("shell hook accept error: {error}"),
        }
    }
}

async fn handle_stream(stream: UnixStream, db: Arc<Database>, app: AppHandle) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(payload) = serde_json::from_str::<HookPayload>(&line) else {
            continue;
        };
        let pane_id = payload.pane_id.unwrap_or_else(|| "unknown".to_string());
        let block = Block {
            id: String::new(),
            pane_id: pane_id.clone(),
            command: payload.command.clone(),
            cwd: payload.cwd.clone(),
            exit_code: Some(payload.exit_code),
            duration_ms: payload.duration_ms,
            started_at: payload.started_at,
            git_branch: payload.git_branch,
            git_dirty: payload.git_dirty,
            shell: payload.shell,
            initiated_by: "user".into(),
            is_interactive: false,
            stdout: None,
            stderr: None,
        };
        if let Ok(block_id) = db.insert_block(block).await {
            let _ = db.upsert_command_frequency(&payload.command, &payload.cwd).await;
            let _ = db.touch_pane(&pane_id, &payload.cwd).await;
            let _ = app.emit(
                "block_finished",
                serde_json::json!({ "block_id": block_id, "pane_id": pane_id }),
            );
        }
    }
}
