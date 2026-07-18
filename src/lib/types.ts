export type WorkspaceMode = "normal" | "agentic";

export interface Block {
  id: string;
  command: string;
  cwd: string;
  exit_code: number | null;
  duration_ms: number;
  output: string;
  is_interactive?: boolean;
}

export interface SuggestedCommand {
  command: string;
  score: number;
  run_count: number;
  cwd_local: boolean;
}

export interface LlmBackendConfig {
  id: string;
  name: string;
  kind: "openai" | "bedrock";
  base_url: string | null;
  model: string;
  api_key_ref: string | null;
  is_default: boolean;
  is_local: boolean;
  created_at: number;
  context_window_tokens: number;
}
