use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::WorkspaceMode;

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_initial", include_str!("../migrations/001_initial.sql")),
    ("002_seed_settings", include_str!("../migrations/002_seed_settings.sql")),
    ("003_backend_context_window", include_str!("../migrations/003_backend_context_window.sql")),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub pane_id: String,
    pub command: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
    pub started_at: i64,
    pub git_branch: Option<String>,
    pub git_dirty: bool,
    pub shell: String,
    pub initiated_by: String,
    pub is_interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandFrequency {
    pub command: String,
    pub cwd: String,
    pub run_count: i64,
    pub last_run_at: i64,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate().await?;
        Ok(db)
    }

    pub async fn in_memory() -> anyhow::Result<Self> {
        let db = Self { conn: Mutex::new(Connection::open_in_memory()?) };
        db.migrate().await?;
        Ok(db)
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
              version TEXT PRIMARY KEY,
              applied_at INTEGER NOT NULL
            );",
        )?;
        for (version, sql) in MIGRATIONS {
            let exists: Option<String> = conn
                .query_row(
                    "SELECT version FROM schema_migrations WHERE version = ?1",
                    params![version],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                match conn.execute_batch(sql) {
                    Ok(()) => {
                        conn.execute(
                            "INSERT INTO schema_migrations(version, applied_at) VALUES(?1, ?2)",
                            params![version, Utc::now().timestamp_millis()],
                        )?;
                    }
                    Err(error)
                        if version == &"003_backend_context_window"
                            && error.to_string().contains("duplicate column name") =>
                    {
                        conn.execute(
                            "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES(?1, ?2)",
                            params![version, Utc::now().timestamp_millis()],
                        )?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(())
    }

    pub async fn insert_block(&self, mut block: Block) -> anyhow::Result<String> {
        if block.id.is_empty() {
            block.id = Uuid::new_v4().to_string();
        }
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO blocks(
              id, pane_id, command, cwd, exit_code, duration_ms, started_at,
              git_branch, git_dirty, shell, initiated_by, is_interactive
            ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                block.id,
                block.pane_id,
                block.command,
                block.cwd,
                block.exit_code,
                block.duration_ms,
                block.started_at,
                block.git_branch,
                i32::from(block.git_dirty),
                block.shell,
                block.initiated_by,
                i32::from(block.is_interactive)
            ],
        )?;
        Ok(block.id)
    }

    pub async fn update_block_exit(&self, id: &str, exit_code: i32) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE blocks SET exit_code = ?2, finished_at = ?3 WHERE id = ?1",
            params![id, exit_code, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub async fn query_blocks_fuzzy(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<Block>> {
        let pattern = format!("%{query}%");
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, pane_id, command, cwd, exit_code, duration_ms, started_at,
                    git_branch, git_dirty, shell, initiated_by, is_interactive
             FROM blocks
             WHERE command LIKE ?1 OR cwd LIKE ?1
             ORDER BY started_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], row_to_block)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub async fn recent_blocks(&self, pane_id: &str, limit: usize) -> anyhow::Result<Vec<Block>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, pane_id, command, cwd, exit_code, duration_ms, started_at,
                    git_branch, git_dirty, shell, initiated_by, is_interactive
             FROM blocks WHERE pane_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pane_id, limit as i64], row_to_block)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub async fn upsert_command_frequency(&self, command: &str, cwd: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO command_frequency(command, cwd, run_count, last_run_at)
             VALUES(?1, ?2, 1, ?3)
             ON CONFLICT(command, cwd) DO UPDATE SET
               run_count = run_count + 1,
               last_run_at = excluded.last_run_at",
            params![command, cwd, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub async fn command_candidates(
        &self,
        cwd: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CommandFrequency>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT command, cwd, run_count, last_run_at FROM command_frequency
             WHERE cwd = ?1 OR cwd = ''
             ORDER BY CASE WHEN cwd = ?1 THEN 0 ELSE 1 END, run_count DESC, last_run_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![cwd, limit as i64], |row| {
            Ok(CommandFrequency {
                command: row.get(0)?,
                cwd: row.get(1)?,
                run_count: row.get(2)?,
                last_run_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().await;
        conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
            .optional()
            .map_err(Into::into)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub async fn insert_pane(
        &self,
        id: &str,
        cwd: &str,
        shell: &str,
        mode: WorkspaceMode,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO panes(id, cwd, shell, workspace_mode, created_at, last_active_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, cwd, shell, mode_to_db(&mode), now],
        )?;
        Ok(())
    }

    pub async fn touch_pane(&self, id: &str, cwd: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE panes SET cwd = ?2, last_active_at = ?3 WHERE id = ?1",
            params![id, cwd, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub async fn update_pane_mode(&self, id: &str, mode: WorkspaceMode) -> anyhow::Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE panes SET workspace_mode = ?2 WHERE id = ?1",
            params![id, mode_to_db(&mode)],
        )?;
        Ok(())
    }

    pub async fn get_pane_mode(&self, id: &str) -> anyhow::Result<Option<WorkspaceMode>> {
        let conn = self.conn.lock().await;
        let raw: Option<String> = conn
            .query_row("SELECT workspace_mode FROM panes WHERE id = ?1", params![id], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(raw.as_deref().map(mode_from_db))
    }
}

fn row_to_block(row: &rusqlite::Row<'_>) -> rusqlite::Result<Block> {
    Ok(Block {
        id: row.get(0)?,
        pane_id: row.get(1)?,
        command: row.get(2)?,
        cwd: row.get(3)?,
        exit_code: row.get(4)?,
        duration_ms: row.get(5)?,
        started_at: row.get(6)?,
        git_branch: row.get(7)?,
        git_dirty: row.get::<_, i32>(8)? != 0,
        shell: row.get(9)?,
        initiated_by: row.get(10)?,
        is_interactive: row.get::<_, i32>(11)? != 0,
    })
}

fn mode_to_db(mode: &WorkspaceMode) -> &'static str {
    match mode {
        WorkspaceMode::Normal => "normal",
        WorkspaceMode::Agentic => "agentic",
    }
}

fn mode_from_db(raw: &str) -> WorkspaceMode {
    match raw {
        "agentic" => WorkspaceMode::Agentic,
        _ => WorkspaceMode::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_are_idempotent_and_seed_defaults() {
        let db = Database::in_memory().await.unwrap();
        db.migrate().await.unwrap();
        assert_eq!(db.get_setting("spill.max_file_size_mb").await.unwrap().as_deref(), Some("10"));
    }

    #[tokio::test]
    async fn insert_query_and_frequency_increment() {
        let db = Database::in_memory().await.unwrap();
        db.insert_pane("pane", "/tmp", "/bin/zsh", WorkspaceMode::Normal).await.unwrap();
        db.insert_block(Block {
            id: String::new(),
            pane_id: "pane".into(),
            command: "git status".into(),
            cwd: "/tmp".into(),
            exit_code: Some(0),
            duration_ms: 12,
            started_at: 1,
            git_branch: Some("main".into()),
            git_dirty: false,
            shell: "zsh".into(),
            initiated_by: "user".into(),
            is_interactive: false,
        })
        .await
        .unwrap();
        assert_eq!(db.query_blocks_fuzzy("git", 10).await.unwrap().len(), 1);
        db.upsert_command_frequency("ls", "/tmp").await.unwrap();
        db.upsert_command_frequency("ls", "/tmp").await.unwrap();
        assert_eq!(db.command_candidates("/tmp", 10).await.unwrap()[0].run_count, 2);
    }
}
