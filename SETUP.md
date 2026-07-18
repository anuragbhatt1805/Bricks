# Setup Guide

This guide covers everything needed to run Brick locally for development or personal use.

---

## System Requirements

| Requirement | Version |
|---|---|
| macOS | 12 Monterey or later |
| Xcode Command Line Tools | latest |
| Rust | stable (≥ 1.80) |
| Node.js | 24.x |
| Yarn | 1.x (classic) |
| Python 3 | 3.9+ (for the zsh hook) |

> **Apple Silicon and Intel are both supported.** Release builds are separate `.dmg` files per architecture.
> **Note:** Release builds are unsigned. On first launch you will need to allow the app via **System Settings → Privacy & Security → Open Anyway**.

---

## 1. Install Prerequisites

### Xcode Command Line Tools

```sh
xcode-select --install
```

### Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup update stable
```

### Node.js via `nvm` (recommended)

```sh
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
nvm install 24
nvm use 24
```

### Yarn

```sh
npm install -g yarn
```

### Tauri CLI v2

```sh
cargo install tauri-cli --version "^2" --locked
```

---

## 2. Clone the Repository

```sh
git clone https://github.com/anuragbhatt1805/brick.git
cd brick
```

---

## 3. Install Dependencies

```sh
# Node / Svelte dependencies
yarn install

# Rust dependencies are fetched automatically on first build
```

---

## 4. Run in Development Mode

```sh
cargo tauri dev
```

This single command:
1. Starts the Vite dev server at `http://127.0.0.1:1420` (hot-reload for Svelte)
2. Compiles the Rust backend
3. Opens the Brick app window

> **First run takes 3–5 minutes** while Cargo compiles dependencies. Subsequent runs use the build cache and start in < 30 seconds.

---

## 5. Install the Shell Hook (Optional but Recommended)

The shell hook enables Brick to capture command history, exit codes, and git context from your terminal sessions.

```sh
mkdir -p ~/.config/brick
cp scripts/brick_hook.zsh ~/.config/brick/brick_hook.zsh

# Add to your ~/.zshrc:
echo '\nsource ~/.config/brick/brick_hook.zsh' >> ~/.zshrc

# Reload your shell:
source ~/.zshrc
```

The hook is **zero-impact when Brick is not running** — it checks for `BRICK_SHELL_SOCKET` before attempting any connection.

---

## 6. Configure an LLM Backend (Optional)

Brick works without an LLM. To enable AI agent features:

### Ollama (Local, Recommended)

1. Install [Ollama](https://ollama.ai): `brew install ollama`
2. Pull a model: `ollama pull llama3.2` (or any supported model)
3. Start Ollama: `ollama serve`
4. In Brick's settings, add a backend:
   - **Type**: OpenAI Compatible
   - **Base URL**: `http://localhost:11434`
   - **Model**: `llama3.2`
   - **Local**: ✅

### OpenAI-Compatible Remote Endpoint

- **Base URL**: `https://api.openai.com`
- **Model**: `gpt-4o`
- **API Key**: stored securely in macOS Keychain, never on disk

### AWS Bedrock

Uses your AWS credentials from `~/.aws/credentials` or environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`). No additional setup needed in Brick.

---

## 7. Local Data Location

All data is stored locally and never synced to the cloud:

| Data | Location |
|---|---|
| SQLite database | `~/Library/Application Support/dev.brick.app/brick.db` |
| Shell hook socket | `~/Library/Application Support/dev.brick.app/shell.sock` |

To reset all data: `rm ~/Library/Application\ Support/dev.brick.app/brick.db`

---

## 8. Running Tests

```sh
# Rust unit tests (13 tests)
cargo test --manifest-path src-tauri/Cargo.toml --workspace

# Frontend tests (Vitest)
yarn test

# Svelte type checking
yarn run check

# Rust lint
cargo clippy --manifest-path src-tauri/Cargo.toml --lib --bins -- -D warnings

# Rust format check
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

---

## Troubleshooting

**`cargo tauri dev` fails with "linker not found"**
→ Run `xcode-select --install`

**Vite starts but the app window is blank**
→ Wait 10–15 seconds on first launch for the Svelte build to complete. If still blank, check the Tauri DevTools console (`Cmd+Option+I`).

**Shell hook not capturing commands**
→ Ensure `BRICK_SHELL_SOCKET` is set: `echo $BRICK_SHELL_SOCKET`. If empty, Brick is not running or the hook wasn't sourced correctly. Re-run `source ~/.zshrc`.

**"Backend not found" error in Agent panel**
→ No LLM backend is configured. Go to Settings → Backends → Add Backend.
