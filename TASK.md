Bricks

Build "Brick" — a macOS-only, local-first terminal emulator. Implement the following plan in full, task by task, verifying tests pass before moving to the next task.

STACK:
- Tauri v2 (Rust backend + macOS WebView frontend)
- Svelte + TypeScript + Vite (frontend)
- Yarn as package manager (npm fallback)
- Rust stable (fresh install) — all dependencies workspace-scoped via Cargo.toml
- SQLite via rusqlite (bundled feature)
- macOS only, zsh primary shell, bash secondary

CONSTRAINTS:
- Zero forced cloud dependency — every feature works fully offline
- Secrets never leave the machine unintentionally — mandatory redaction before any LLM call
- Target <150MB idle memory
- Normal workspace mode = hard guarantee of zero LLM/network calls from that tab, enforced in Rust core
- Confirm-tier commands (git push, remote deploy, package install, data deletion) always require explicit user approval — no config path to bypass
- Out of scope for v1: Windows/Linux, fish shell, session sharing, telemetry, auto-update, RAG/embeddings, Go sidecar

KEY CRATES:
- alacritty_terminal (VTE parsing + grid state)
- portable-pty (PTY spawning)
- tokio (async runtime)
- serde/serde_json (serialization)
- rusqlite with bundled feature (SQLite)
- keyring (macOS Keychain)
- reqwest with stream feature (LLM HTTP)
- fuzzy-matcher skim algorithm (auto-suggestion)
- tiktoken-rs (token counting)
- notify (file watcher)
- zstd (compression)
- nix (Unix signals)
- insta (snapshot testing)
- wiremock (mock HTTP server for tests)
- async-trait, futures, tokio-stream
- aws-sdk-bedrockruntime (Bedrock adapter)

TASKS:

Task 1: Environment bootstrap and project scaffold
- Verify xcode-select -p returns a valid path
- Install Tauri CLI: cargo install tauri-cli --version "^2" --locked
- Scaffold with cargo tauri init using Svelte + Vite template. identifier = "dev.brick.app", productName = "Brick"
- Set up yarn, confirm yarn dev and cargo tauri dev launch the window
- Set up rustfmt.toml and clippy config in workspace root
- Add .gitignore covering target/, node_modules/, dist/, *.db, spill/
- Configure Cargo workspace Cargo.toml with [workspace] members
Tests: xcode-select -p exits 0, cargo build exits 0, yarn installs cleanly, Tauri window opens
Demo: macOS window titled "Brick" opens showing default scaffold

Task 2: PTY spawning and raw terminal I/O
- Add portable-pty, tokio (full features), serde/serde_json to src-tauri/Cargo.toml
- Implement PtyManager struct: spawn_shell(shell_path, cwd) -> PaneId
- PTY read loop in tokio::spawn; raw bytes sent to frontend via tauri::Emitter as pty_output events tagged with pane_id
- Tauri command pty_write(pane_id, data: Vec<u8>) forwards keystrokes to PTY stdin
- Tauri command pty_resize(pane_id, cols, rows)
- Frontend: minimal textarea-like input and <pre> displaying raw bytes as UTF-8
- Handle PTY process exit: emit pty_exited event with exit code
Tests: spawn PTY running /bin/zsh -c "echo hello", assert output contains "hello" and exit code 0; resize test; non-existent shell returns error not panic
Demo: type commands in Brick window, see raw output, shell stays alive

Task 3: VTE parsing and terminal grid rendering
- Add alacritty_terminal, wrap as TermGrid
- Compute grid diff after each PTY read: { pane_id, cells: [{row, col, char, fg, bg, flags}] } — only changed cells
- 60Hz IPC flush loop: tokio::time::interval(16ms) per pane, coalesces diffs into one grid_update event per tick. Background panes at 4Hz
- Detect ALT_SCREEN mode toggle, emit alt_screen_entered/alt_screen_exited. In alt-screen mode send full grid
- Frontend: CSS-grid-based terminal renderer, each cell a <span> with inline color styles, Svelte reactive stores
Tests: insta snapshot test for known ANSI sequences; vim and htop alt-screen lifecycle tests; IPC throttle test (yes for 1s, assert ≤65 and ≥55 emit events)
Demo: ls --color, git log --oneline, vim, htop all render correctly

Task 4: SQLite persistence layer and migrations
- Add rusqlite with bundled feature
- Database struct wrapping rusqlite::Connection in tokio::sync::Mutex
- Migration system: numbered SQL files in src-tauri/migrations/, tracked in schema_migrations table
- 001_initial.sql: blocks, panes, command_frequency, llm_backends, agent_sessions, settings tables + indexes
- 002_seed_settings.sql: default settings (spill.max_file_size_mb=10, spill.global_budget_mb=5000, spill.retention_days=30, spill.compress_after_days=7, agent.autorun_timeout_seconds=30)
- Typed query helpers: insert_block(), update_block_exit(), query_blocks_fuzzy(), upsert_command_frequency(), get_setting(), set_setting(), insert_pane(), update_pane_mode()
- DB at ~/Library/Application Support/dev.brick.app/brick.db via Tauri app_data_dir()
Tests: migrations on in-memory DB, idempotency, insert+fuzzy query, run_count increment, default settings
Demo: sqlite3 shows all 6 tables after app startup

Task 5: Shell hook integration and block data capture
- Shell hook protocol: JSON line emitted after each command to Unix domain socket at ~/Library/Application Support/dev.brick.app/shell.sock
- JSON schema: { "command": str, "cwd": str, "exit_code": int, "duration_ms": int, "started_at": int, "git_branch": str|null, "git_dirty": bool, "shell": "zsh" }
- Rust: tokio::net::UnixListener task, parse JSON → Block, call insert_block() and upsert_command_frequency()
- zsh hook script at scripts/brick_hook.zsh using precmd and preexec
- BRICK_PANE_ID env var set on PTY shell at spawn time
- initiated_by defaults to 'user' for hook-reported commands
- panes table lifecycle: create on PTY spawn, update last_active_at on each block, delete on pane close
Tests: simulate zsh precmd payload over test socket, assert Block written correctly; two panes route correctly; malformed JSON discarded without panic; panes row created with workspace_mode='normal'
Demo: run cd /tmp && git status, observe blocks row in sqlite3 with correct fields; ls twice shows run_count=2

Task 6: Block-based UI rendering
- BlockList.svelte: virtualized list, only render blocks within viewport ± one screen buffer. Smooth with 10,000+ blocks
- Block.svelte: header row (command, cwd badge, exit code, duration), output area (collapsible after 50 lines), hover action bar (copy command, copy output, re-run, ask agent)
- Exit code: green ✓ if 0 (hidden unless hovering), red ✗ with code if nonzero
- Alt-screen blocks (is_interactive=1): compact card with no output content
- Connect to Rust: accumulate grid_update events between block_started and block_finished events
- Workspace mode indicator: persistent pill badge on each tab, "Normal" (grey) or "Agentic" (amber + ⚡). Static "Normal" for now, toggle wired in Task 9
Tests: BlockList with 10,000 mock blocks, assert ~20-40 DOM nodes only; exit code display; is_interactive compact card; show/hide more toggle
Demo: 5-10 commands appear as distinct blocks, long output collapses, vim session shows compact card, 10,000 blocks scroll smoothly

Task 7: Prompt info bar and file watcher
- Add notify crate
- On each cd (from shell hook cwd change), watch new directory for .git/HEAD, .git/index, .nvmrc, .node-version, pyproject.toml, .python-version, Cargo.toml, package.json
- DirectoryMetaCache: HashMap<PathBuf, DirMeta> behind RwLock, lazy recompute in tokio::task on miss or invalidation
- Git info: async child processes for git rev-parse and git status --porcelain with 500ms timeout
- Tool version detection: read version strings directly from version files, not shelling out to node --version etc.
- Emit prompt_info IPC event to frontend on cache refresh
- PromptInfoBar.svelte: cwd truncated to last 2-3 segments with … prefix, click opens Tauri dialog. Git branch as ⎇ main with • dirty indicator. Tool version badges only if version file detected. Exit code hidden if 0
Tests: cache returns cached value on second call; .git/index change triggers invalidation within 200ms; .nvmrc "18.0.0" produces Node badge; git probe times out after 500ms; deep path truncation; dirty indicator toggle
Demo: cd into Node project shows Node version badge; cd into dirty git repo shows branch + dot; cd / shows no badges

Task 8: Fuzzy auto-suggestion
- Add fuzzy-matcher crate (skim algorithm)
- Tauri command suggest_command(partial: String, cwd: String) -> Vec<SuggestedCommand>
- Query command_frequency + blocks, cwd-local first then global, sort by score DESC + run_count DESC + last_run_at DESC, top 5, LIMIT 200 pre-filter before fuzzy scoring
- CommandInput.svelte: ghost-text span showing top suggestion suffix, Tab accepts, Esc dismisses, ArrowDown cycles dropdown
- Debounce 50ms from last keystroke
Tests: "git s" returns "git status" ranked first from ["git status", "git diff", "git log"]; cwd locality bias; empty partial returns most-frequent; 100k rows <10ms; Tab fills input in component test
Demo: type gi — ghost text shows t status, Tab fills, arrow keys cycle 5 suggestions, all instant

Task 9: Workspace mode toggle (Normal vs Agentic)
- Tauri command set_pane_mode(pane_id: String, mode: WorkspaceMode): update panes.workspace_mode, emit pane_mode_changed event
- Implement assert_agentic_mode(pane_id) guard: returns Err(AgentError::DisabledInNormalMode) for Normal-mode panes. All agent orchestrator functions (Task 14) call this first
- TabBar.svelte: pill badge per tab, click or Cmd+Shift+A calls set_pane_mode. Normal = grey, Agentic = amber + ⚡
- AgentPanel.svelte placeholder: div shown/hidden based on mode
- Switching to Normal emits agent_panel_hidden, frontend hides agent panel immediately
Tests: assert_agentic_mode returns Err for Normal, Ok for Agentic, no restart needed; two panes independent; mode persists across simulated app restart; component badge toggle; Cmd+Shift+A keybind
Demo: two tabs, one toggled to Agentic (amber ⚡), one stays Normal, both independent, agent panel placeholder appears/disappears

Task 10: Settings screen and shell hook install UI
- Settings.svelte: tabbed layout — Backends, Shell Integration, Data, Appearance
- Shell Integration tab: check_shell_hook_installed(shell) command, Install button appends source line to ~/.zshrc, Uninstall removes it, green ✓ / red ✗ status, copies hook script to ~/.config/brick/brick_hook.zsh
- Backends tab (stub): list backends, Add backend form (kind, base URL, model name, API key via keyring crate), test-connection button placeholder (wired in Task 11)
- Data tab: DB path + size, spill dir size, spill settings as editable inputs, Purge now button, Export JSON button
- Appearance tab: font family, font size slider, light/dark toggle
- Output spill reaper as background tokio task: on startup + every 6 hours, compress files older than compress_after_days (zstd), delete files older than retention_days, evict oldest-first if over global_budget_mb, update blocks.is_compressed, null out stdout_path/stderr_path, keep blocks row
Tests: check_shell_hook_installed false on fresh system, true after install; idempotent install; uninstall leaves rest of .zshrc intact; reaper compresses/deletes/evicts correctly, blocks row survives with stdout_path=NULL; export produces valid JSON
Demo: Install for zsh turns green, Purge now removes aged files, Export downloads JSON

Task 11: LLM backend adapters
- Add reqwest (stream + json features), aws-sdk-bedrockruntime, async-trait, futures, tokio-stream
- Core types in llm module: ChatMessage, ToolDefinition, ChatStreamChunk, LlmError
- LlmBackend trait: chat_stream() -> BoxStream<Result<ChatStreamChunk, LlmError>>, supports_tools() -> bool, backend_kind() -> BackendKind
- OpenAiCompatibleBackend: POST to {base_url}/v1/chat/completions with stream:true, parse SSE data lines, handle [DONE], read api_key from keyring if api_key_ref set
- BedrockBackend: aws-sdk-bedrockruntime InvokeModelWithResponseStream, convert ChatMessage to Bedrock Messages API format, AWS credentials from standard chain
- BackendRegistry: loaded from llm_backends table, get_default() and get_by_id()
- Tauri command test_backend_connection(backend_id): ping, return Ok(latency_ms) or Err(message)
- Migration 003_backend_context_window.sql: add context_window_tokens column to llm_backends. Auto-detect for Ollama via GET {base_url}/api/show
Tests (wiremock): mock SSE endpoint collects 3 chunks + [DONE] correctly; interrupted stream returns Err(StreamInterrupted); 401 returns Err(AuthError); Bedrock request shape snapshot test; test_backend_connection latency and error cases
Demo: Add Ollama backend, Test Connection shows latency; wrong URL shows clear error

Task 12: Secret redaction layer
- redact module, pure Rust, no async
- Passes: (1) AWS access key AKIA[0-9A-Z]{16} → [REDACTED:AWS_ACCESS_KEY], (2) AWS secret key adjacent to aws_secret context → [REDACTED:AWS_SECRET_KEY], (3) (?i)(SECRET|TOKEN|PASSWORD|API_KEY|PRIVATE_KEY)\s*=\s*\S+ → [REDACTED:ENV_SECRET], (4) ghp_/gho_/github_pat_ prefix → [REDACTED:GITHUB_TOKEN], (5) -----BEGIN * PRIVATE KEY----- blocks → [REDACTED:PRIVATE_KEY], (6) Shannon entropy >4.5 bits/char on tokens >20 chars → [FLAGGED:HIGH_ENTROPY]
- Returns RedactResult { output: String, redactions: Vec<RedactionEvent> }
- PromptBuilder only accepts &RedactResult, never raw strings — enforced via type system
- Agent session transcripts stored as post-redaction versions
Tests: fake AWS key redacted; MY_TOKEN=abc123 redacted; ghp_ token redacted; multi-line PEM block fully redacted; const x = 1 + 2 passes unchanged; base64 32-byte string triggers high-entropy flag; PromptBuilder with raw string fails to compile; integration test confirms [REDACTED:ENV_SECRET] in wiremock HTTP log not original value
Demo: context with GITHUB_TOKEN=ghp_fake shows redaction badge in agent panel; wiremock confirms raw token never in request body

Task 13: Risk classifier
- risk module: classify(command: &str) -> RiskTier (Safe | Confirm | Blocked)
- Tokenize on |, &&, ||, ; — chain inherits highest tier of any component
- Blocked (hardcoded, non-removable): rm -rf / variants, dd of=/dev/, curl|sh and wget|sh, mkfs., fork-bomb patterns, chmod -R 777 /
- Confirm (hardcoded, non-removable except git commit): git push (any flags), git commit (configurable via settings.git_commit_tier), scp/rsync to remote (: in destination), ssh with trailing command, curl/wget with upload flags, npm install/uninstall, yarn add/remove, pnpm add/remove, pip install/uninstall, cargo add/remove, apt, brew install/uninstall, rm/rmdir/git clean/git reset --hard, kubectl apply/delete, docker push/run, aws s3 cp/rm/deploy, sudo (any), database migration commands
- Safe: everything else — read-only commands, creating new files, writing/editing local files
- When settings.git_commit_tier = 'safe', git commit reclassified to Safe. No other confirm-tier item is configurable
- Expose classify() as Tauri command for frontend tier badge display
Tests (table-driven): all blocked patterns return Blocked; all confirm patterns return Confirm; ls -la, cat, git status, git diff, grep, touch, echo > ./out.txt return Safe; git push && rm -rf / → Blocked; sudo ls → Confirm; git commit default Confirm, with git_commit_tier=safe → Safe; git push --force → Confirm; adversarial: RM -RF / uppercase → Blocked, extra spaces → correct tier
Demo: agent proposes cat package.json → auto-executes; npm install → confirm card; rm -rf / → blocked card with no approve button

Task 14: Agent orchestrator and execution loop
- AgentOrchestrator: owns BackendRegistry, Database, PtyManager, risk classifier. Behind tokio::sync::Mutex per pane
- Every public method calls assert_agentic_mode(pane_id) first
- Context assembly via PromptBuilder (Task 12): system prompt + tool definitions + recent blocks from DB + current user message, capped by context_window_tokens from backend DB row. Budget: subtract system+tools+current message floor, fill remainder with blocks newest-first, drop oldest entirely when budget exhausted, apply head(30%)+tail(50%) truncation on single oversized blocks with [... ~M tokens truncated ...] marker. Token estimates via tiktoken-rs. If any truncation occurred, include context_trimmed: true in IPC event
- Tool definitions: run_command(command), read_file(path), list_directory(path), write_file(path, content)
- Agent loop per turn:
  1. Assemble prompt, call backend.chat_stream(), stream chunks to frontend via agent_stream_chunk IPC events
  2. Tool call response → extract command → classify via risk::classify()
  3. Safe tier: execute via PtyManager, block with initiated_by='agent', feed output back, continue loop
  4. Confirm tier: emit agent_proposed_command { command, risk_tier, block_id } → wait for user Approve/Edit/Reject IPC response
  5. Blocked tier: emit agent_blocked_command { command, reason }, never execute
  6. Plain text response: stream to frontend, loop ends
- Watchdog for safe-tier auto-run: tokio::time::sleep(autorun_timeout_seconds), if fires before exit emit agent_autorun_timeout { block_id } — do NOT auto-kill
- Process control: Tauri command send_signal(block_id, signal: sigint|sigterm|sigkill) → nix::sys::signal::killpg(pgid, sig)
- Cancel: Tauri command cancel_agent_turn(pane_id) drops stream future, emits agent_cancelled
Tests (wiremock): mock LLM returns run_command("ls -la") → auto-executed, no confirmation event; mock returns run_command("npm install lodash") → agent_proposed_command emitted, command NOT executed until approval; mock returns run_command("rm -rf /") → agent_blocked_command, never executed; full 2-turn loop test; watchdog fires agent_autorun_timeout after threshold, send_signal(sigint) terminates process group including child subprocesses; Normal-mode pane → Err, wiremock received-calls=0; oversized history against 12k-token mock → within budget, truncation marker present, context_trimmed:true
Demo: ask "what files are in this directory?" → agent auto-runs list_directory, replies; ask "install lodash" → confirm card, Approve → runs; ask "delete all files" → blocked card; switch to Normal → agent panel gone, no LLM call made

Task 15: Agent panel UI
- AgentPanel.svelte: right-side toggleable panel (not modal), resizable via drag handle (width persisted in settings)
- Transcript area: scrollable list of turns. User message right-aligned, assistant message left-aligned with streaming typewriter effect from agent_stream_chunk events
- Proposed command card (confirm-tier): command in code block, risk tier badge, Approve (green) / Edit (inline editor) / Reject (red) buttons. Edit → re-classify before executing
- Auto-run card (safe-tier): command + output inline, "auto-run" badge, stop button for duration running
- Blocked card: red "Blocked" badge + one-line explanation, no approve button, copy command for manual use
- Backend selector: dropdown in panel header showing active backend + "🔒 local" or "☁ remote" badge
- Context trimmed indicator: subtle "Context trimmed" info chip on assistant message when context_trimmed:true, tooltip explaining what was dropped
- Input area: multi-line textarea, Enter sends, Shift+Enter for newline, spinner while streaming, Cancel button calls cancel_agent_turn
Tests: confirm card renders all 3 buttons, Reject emits correct IPC event; blocked card has no approve button; context_trimmed chip shows/hides; streaming chunks append in order; E2E Playwright: type prompt → proposed-command card appears → Approve → command appears as block in terminal
Demo: full end-to-end agent interaction — streaming, approve flow, blocked flow, backend badge, resize panel, switch backends from dropdown

Task 16: Performance pass and memory profiling
- Stress test BlockList with 50,000 seeded blocks, profile DOM node count and scroll FPS
- Re-run IPC throttle benchmark with yes for 10s, assert ≤650 events (60Hz × 10s + 10% margin)
- Memory profile with macOS Instruments: measure at app launch, 5 tabs open, 200 commands run, long agent session. Document in PERFORMANCE.md
- Verify background pane drops to 4Hz IPC cadence
- EXPLAIN QUERY PLAN on fuzzy suggestion query with 100k rows, add missing indexes if any
- Run yarn build, report JS bundle size, confirm no unintended large dependencies
- Launch app with no backend configured, verify zero outbound connections via lsof -i
Tests: BlockList scroll handler <16ms with 50k blocks; IPC throttle ≤650 events in 10s burst; suggest_command <10ms P99 with 100k rows; idle memory measured and logged
Demo: benchmark suite runs and shows numbers, Instruments shows frame rate >55fps at 50k blocks, PERFORMANCE.md documents actual idle memory

Task 17: Manual QA checklist, CI setup, and v1 wrap-up
- GitHub Actions CI: rust-test job (cargo test --workspace + cargo clippy -- -D warnings + cargo fmt --check), frontend-test job (yarn test), e2e job (cargo tauri build in test mode + Playwright on macos-latest runner)
- QA_CHECKLIST.md: full Section 7.3 checklist as GitHub-flavoured markdown checkboxes with verification instructions per item
- README.md: prerequisites, build steps (cargo tauri dev, cargo tauri build), shell hook install instructions, how to configure Ollama backend, known limitations
- PERFORMANCE.md: idle memory, IPC benchmark results, scroll benchmark results (updated from Task 16)
- Final acceptance criteria check against Section 9: go through each criterion, verify or document the test covering it
Tests: all cargo test --workspace pass, all yarn test pass, all Playwright E2E pass on macOS runner, cargo clippy -- -D warnings produces no warnings
Demo: push to GitHub, CI green on all 3 jobs, cargo tauri build produces .app bundle, QA checklist walkable by new contributor
