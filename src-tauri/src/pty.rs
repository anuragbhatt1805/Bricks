use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::task;
use uuid::Uuid;

pub type PaneId = String;

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("pane not found: {0}")]
    PaneNotFound(String),
    #[error("pty error: {0}")]
    Pty(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Serialize)]
struct PtyOutputEvent {
    pane_id: PaneId,
    data: Vec<u8>,
    text: String,
}

#[derive(Clone, Serialize)]
struct PtyExitedEvent {
    pane_id: PaneId,
    exit_code: Option<i32>,
}

struct PtyPane {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
}

#[derive(Default)]
pub struct PtyManager {
    panes: tokio::sync::RwLock<HashMap<PaneId, Arc<PtyPane>>>,
}

impl PtyManager {
    pub async fn spawn_shell(
        &self,
        app: &AppHandle,
        shell_path: &str,
        cwd: PathBuf,
    ) -> Result<PaneId, PtyError> {
        if !std::path::Path::new(shell_path).exists() {
            return Err(PtyError::Pty(format!("shell does not exist: {shell_path}")));
        }
        let pane_id = Uuid::new_v4().to_string();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .map_err(|error| PtyError::Pty(error.to_string()))?;
        let mut cmd = CommandBuilder::new(shell_path);
        cmd.cwd(cwd);
        cmd.env("BRICK_PANE_ID", &pane_id);
        let app_data = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".brick-data"));
        cmd.env("BRICK_SHELL_SOCKET", app_data.join("shell.sock"));
        let mut child =
            pair.slave.spawn_command(cmd).map_err(|error| PtyError::Pty(error.to_string()))?;
        let mut reader =
            pair.master.try_clone_reader().map_err(|error| PtyError::Pty(error.to_string()))?;
        let writer = pair.master.take_writer().map_err(|error| PtyError::Pty(error.to_string()))?;
        let pane =
            Arc::new(PtyPane { writer: Mutex::new(writer), master: Mutex::new(pair.master) });
        self.panes.write().await.insert(pane_id.clone(), pane);

        let output_app = app.clone();
        let output_pane_id = pane_id.clone();
        task::spawn_blocking(move || {
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        let text = String::from_utf8_lossy(&data).to_string();
                        let _ = output_app.emit(
                            "pty_output",
                            PtyOutputEvent { pane_id: output_pane_id.clone(), data, text },
                        );
                    }
                    Err(_) => break,
                }
            }
        });

        let exit_app = app.clone();
        let exit_pane_id = pane_id.clone();
        task::spawn_blocking(move || {
            let status = child.wait().ok();
            let exit_code = status.and_then(|status| i32::try_from(status.exit_code()).ok());
            let _ =
                exit_app.emit("pty_exited", PtyExitedEvent { pane_id: exit_pane_id, exit_code });
        });
        Ok(pane_id)
    }

    pub async fn write(&self, pane_id: &str, data: &[u8]) -> Result<(), PtyError> {
        let panes = self.panes.read().await;
        let pane = panes.get(pane_id).ok_or_else(|| PtyError::PaneNotFound(pane_id.to_string()))?;
        let mut writer =
            pane.writer.lock().map_err(|_| PtyError::Pty("writer lock poisoned".into()))?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    pub async fn resize(&self, pane_id: &str, cols: u16, rows: u16) -> Result<(), PtyError> {
        let panes = self.panes.read().await;
        let pane = panes.get(pane_id).ok_or_else(|| PtyError::PaneNotFound(pane_id.to_string()))?;
        let master =
            pane.master.lock().map_err(|_| PtyError::Pty("master lock poisoned".into()))?;
        master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|error| PtyError::Pty(error.to_string()))
    }
}
