# Brick

Brick is a macOS-only, local-first terminal emulator built with Tauri v2, Rust,
Svelte, TypeScript, and Vite.

## Prerequisites

- macOS with Xcode Command Line Tools (`xcode-select -p`)
- Rust stable
- Node.js and Yarn
- Tauri CLI v2 (`cargo install tauri-cli --version "^2" --locked`)

## Development

```sh
yarn install
cargo build --workspace
yarn dev
cargo tauri dev
```

The development window is titled `Brick` and uses `/bin/zsh` as the primary
shell.

## Shell Hook

Brick includes a zsh hook at `scripts/brick_hook.zsh`. The app sets
`BRICK_PANE_ID` and `BRICK_SHELL_SOCKET` for spawned shells. To install manually:

```sh
mkdir -p ~/.config/brick
cp scripts/brick_hook.zsh ~/.config/brick/brick_hook.zsh
printf '\nsource ~/.config/brick/brick_hook.zsh\n' >> ~/.zshrc
```

The hook reports command blocks to:

```text
~/Library/Application Support/dev.brick.app/shell.sock
```

## Local Data

Brick stores SQLite data locally at:

```text
~/Library/Application Support/dev.brick.app/brick.db
```

There is no forced cloud dependency. Normal workspace mode is the default and
agentic calls are blocked by the Rust core guard.

## Ollama Backend

Backend persistence is scaffolded in `llm_backends`. For an Ollama-compatible
backend, use a local base URL such as:

```text
http://localhost:11434
```

The full streaming adapter and connection test flow are represented by core
types and UI placeholders in this scaffold, with no outbound call made unless
an agentic backend is explicitly configured and invoked.

## Verification

```sh
yarn test
yarn build
yarn run check
cargo build --workspace
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

## Known Limitations

- v1 implementation is scaffolded and tested for the core local-first spine.
- Full `alacritty_terminal`-backed rendering, LLM streaming adapters, Playwright
  E2E, spill reaper, and Instruments memory capture still need completion.
- Windows, Linux, fish shell, session sharing, telemetry, auto-update,
  RAG/embeddings, and Go sidecars are intentionally out of scope.
