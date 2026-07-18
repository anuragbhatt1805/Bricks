# Architecture

Brick is structured as a [Tauri v2](https://tauri.app/) application — a thin native shell wrapping a Svelte frontend over a Rust backend. All persistent state lives on-device; there are no required external services.

---

## High-Level Overview

```
┌─────────────────────────────────────────────────────┐
│                   Brick.app                          │
│                                                      │
│  ┌────────────────┐        ┌───────────────────────┐ │
│  │  Svelte UI     │◄──IPC──►  Rust Core (Tauri)    │ │
│  │  (WebView)     │        │                       │ │
│  │                │        │  pty.rs   agent.rs    │ │
│  │  Terminal pane │        │  llm.rs   risk.rs     │ │
│  │  Agent panel   │        │  redact.rs database.rs│ │
│  └────────────────┘        └──────────┬────────────┘ │
│                                       │               │
│                            ┌──────────▼────────────┐ │
│                            │  SQLite (brick.db)     │ │
│                            │  ~/Library/App Support │ │
│                            └───────────────────────┘ │
└─────────────────────────────────────────────────────┘
         ▲                          │
         │ zsh preexec/precmd       │ HTTP (local only)
         │ Unix socket              ▼
  ┌──────┴──────┐          ┌────────────────┐
  │  Your Shell  │          │  Ollama / LLM  │
  │  (zsh hook)  │          │  (optional)    │
  └─────────────┘          └────────────────┘
```

---

## Directory Structure

```
brick/
├── src/                        # Svelte frontend
│   ├── lib/
│   │   ├── AgentPanel.svelte   # AI agent chat + approval UI
│   │   ├── BlockList.svelte    # Scrollable command history
│   │   └── *.test.ts           # Vitest unit tests
│   └── App.svelte
│
├── src-tauri/                  # Rust backend
│   ├── src/
│   │   ├── lib.rs              # Tauri app entry; command registration
│   │   ├── agent.rs            # AgentOrchestrator; run_turn loop
│   │   ├── database.rs         # SQLite wrapper (rusqlite + migrations)
│   │   ├── llm.rs              # LLM backend trait + adapters
│   │   ├── pty.rs              # PTY session management (portable-pty)
│   │   ├── redact.rs           # Secret scrubbing (regex + entropy)
│   │   ├── risk.rs             # Command risk classifier (Safe/Confirm/Blocked)
│   │   ├── shell_hook.rs       # Unix socket server for zsh hook
│   │   ├── suggestions.rs      # Fuzzy command autocomplete
│   │   ├── prompt_info.rs      # Shell prompt context builder
│   │   └── term_grid.rs        # Alacritty terminal grid helpers
│   ├── migrations/             # SQL migration files (applied at startup)
│   └── tauri.conf.json         # App config (window, CSP, bundle targets)
│
├── scripts/
│   └── brick_hook.zsh          # zsh preexec/precmd hook
│
└── .github/workflows/
    ├── ci.yml                  # PR checks: lint, test, typecheck, build
    └── release.yml             # Tag push → signed .dmg → GitHub Release
```

---

## Rust Modules

### `lib.rs` — App Wiring
Registers all Tauri IPC commands, initialises `AppState` (DB, PTY manager, LLM registry, orchestrator), and spawns the Unix socket server for the shell hook. The `AppState` is shared across command handlers via `Arc<RwLock<Option<AppState>>>`.

### `pty.rs` — PTY Manager
Wraps `portable-pty` to spawn native shell sessions. Each pane gets a UUID. Output is read in a blocking thread and forwarded to the WebView via Tauri events (`pty_output`). Resize commands update the PTY master directly.

### `database.rs` — Persistence
A thin async wrapper around `rusqlite::Connection` protected by a `tokio::sync::Mutex`. The schema has six tables: `blocks`, `panes`, `command_frequency`, `llm_backends`, `agent_sessions`, `settings`. SQL migrations are embedded at compile time and applied idempotently on startup.

### `shell_hook.rs` — Shell Integration
Listens on a Unix domain socket at `~/Library/Application Support/dev.brick.app/shell.sock`. The zsh hook connects after each command and POSTs a JSON payload containing command text, CWD, exit code, duration, and git state. The server inserts a `Block` row and emits `block_finished` to the UI.

### `llm.rs` — LLM Backends
Defines the `LlmBackend` async trait with `chat_stream()`. Two concrete adapters:

| Adapter | Protocol |
|---|---|
| `OpenAiCompatibleBackend` | HTTP SSE to any OpenAI-compatible endpoint (Ollama, LM Studio, etc.) |
| `BedrockBackend` | AWS SDK streaming to `bedrock-runtime` |

API keys are stored in the OS keychain (via `keyring`) and referenced by name — never stored in the DB. A `validate_backend_url()` guard prevents SSRF by restricting local backends to loopback addresses and requiring HTTPS for remote backends.

### `agent.rs` — AI Orchestrator
`AgentOrchestrator::run_turn()` is the core agentic loop:

```
1. Assert pane is in Agentic mode (guard in lib.rs)
2. Load configured LLM backend from registry
3. Build context: recent blocks (token-budget trimmed) + chat history
4. Redact secrets from all text before sending to LLM
5. Stream LLM response → emit delta chunks to UI via agent_stream_chunk
6. On tool call (run_command):
     Safe    → execute immediately via PTY
     Confirm → pause, emit agent_proposed_command, await user approval
     Blocked → emit agent_blocked_command, skip
7. Save assistant message to agent_sessions (excluding system prompt)
```

### `redact.rs` — Secret Scrubbing
Runs before every LLM prompt assembly. Detects and replaces:
- AWS access keys (`AKIA…`)
- AWS secret keys (`aws_secret…`)
- Environment variable secrets (`TOKEN=…`, `API_KEY=…`, etc.)
- GitHub PATs (`ghp_…`, `gho_…`, `github_pat_…`)
- PEM private keys
- High-entropy strings (Shannon entropy > 4.5 bits/char)

### `risk.rs` — Command Classifier
Classifies each proposed shell command before execution into three tiers:

| Tier | Action |
|---|---|
| `Safe` | Auto-execute without prompt |
| `Confirm` | Show approval card to user; wait for response |
| `Blocked` | Refuse; show blocked card |

The classifier splits on `;`, `&&`, `\|\|`, `\|` and scores each segment. Dangerous patterns (`rm -rf /`, `dd of=/dev/…`, fork bombs, `curl … \| sh`) are always `Blocked`.

---

## Frontend (Svelte)

### `AgentPanel.svelte`
Resizable right-side drawer. Streams LLM deltas as they arrive via `agent_stream_chunk` Tauri events. Renders three card types:
- **Streaming text** — typewriter-effect markdown output
- **Proposed command** — approve / edit / reject buttons wired to `agent_approve_command` / `agent_reject_command`
- **Blocked command** — read-only red card with reason

### `BlockList.svelte`
Virtualised list of terminal blocks. Receives `block_finished` events and prepends new items without re-rendering the whole list.

---

## Data Flow: Command Execution

```
User types in terminal
        │
        ▼
PTY write → zsh executes command
        │
        ▼ (on exit)
brick_hook.zsh POSTs JSON to Unix socket
        │
        ▼
shell_hook.rs::handle_stream()
  → database.insert_block()
  → database.upsert_command_frequency()
  → app.emit("block_finished", { block_id, pane_id })
        │
        ▼
BlockList.svelte receives event → renders block
```

## Data Flow: Agent Turn

```
User sends message in AgentPanel
        │
        ▼ invoke("agent_run_turn", { pane_id, message })
agent.rs::run_turn()
  → redact prompt
  → stream to LLM backend
  → emit agent_stream_chunk events (UI renders delta)
  → on tool_call(run_command):
       risk.classify(command)
         Safe    → pty.write(command) → poll for block completion
         Confirm → emit agent_proposed_command
                   ← user approves/edits/rejects
         Blocked → emit agent_blocked_command
```

---

## Security Model

| Concern | Mitigation |
|---|---|
| Secrets in LLM prompts | `redact.rs` scrubs before assembly |
| SSRF via user-provided LLM URL | `validate_backend_url()` enforces loopback for local, HTTPS for remote |
| Dangerous agent commands | `risk.rs` blocks destructive patterns; Confirm tier requires explicit user approval |
| XSS in WebView | Strict CSP: `default-src 'self'; script-src 'self'; connect-src 'self' http://127.0.0.1:* http://localhost:*` |
| Shell hook injection | Socket permissions set to `0o700` (owner-only) after bind |
| Unsafe code | `unsafe_code = "forbid"` in workspace `Cargo.toml` |
