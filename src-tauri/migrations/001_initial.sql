CREATE TABLE IF NOT EXISTS blocks (
  id TEXT PRIMARY KEY,
  pane_id TEXT NOT NULL,
  command TEXT NOT NULL,
  cwd TEXT NOT NULL,
  exit_code INTEGER,
  duration_ms INTEGER NOT NULL DEFAULT 0,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  git_branch TEXT,
  git_dirty INTEGER NOT NULL DEFAULT 0,
  shell TEXT NOT NULL,
  initiated_by TEXT NOT NULL DEFAULT 'user',
  stdout TEXT,
  stderr TEXT,
  stdout_path TEXT,
  stderr_path TEXT,
  is_interactive INTEGER NOT NULL DEFAULT 0,
  is_compressed INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS panes (
  id TEXT PRIMARY KEY,
  cwd TEXT NOT NULL,
  shell TEXT NOT NULL,
  workspace_mode TEXT NOT NULL DEFAULT 'normal',
  created_at INTEGER NOT NULL,
  last_active_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS command_frequency (
  command TEXT NOT NULL,
  cwd TEXT NOT NULL DEFAULT '',
  run_count INTEGER NOT NULL DEFAULT 0,
  last_run_at INTEGER NOT NULL,
  PRIMARY KEY (command, cwd)
);

CREATE TABLE IF NOT EXISTS llm_backends (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  base_url TEXT,
  model TEXT NOT NULL,
  api_key_ref TEXT,
  is_default INTEGER NOT NULL DEFAULT 0,
  is_local INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_sessions (
  id TEXT PRIMARY KEY,
  pane_id TEXT NOT NULL,
  backend_id TEXT,
  transcript_json TEXT NOT NULL DEFAULT '[]',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_blocks_pane_started ON blocks(pane_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_blocks_command ON blocks(command);
CREATE INDEX IF NOT EXISTS idx_command_frequency_cwd ON command_frequency(cwd, run_count DESC, last_run_at DESC);
CREATE INDEX IF NOT EXISTS idx_panes_mode ON panes(workspace_mode);
