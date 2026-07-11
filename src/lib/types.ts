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
