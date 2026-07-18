<div align="center">

# 🧱 Brick

**A local-first, AI-assisted terminal emulator for macOS.**

[![CI](https://github.com/anuragbhatt1805/brick/actions/workflows/ci.yml/badge.svg)](https://github.com/anuragbhatt1805/brick/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/platform-macOS%2012%2B-lightgrey?logo=apple)](https://www.apple.com/macos/)
[![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust)](https://www.rust-lang.org/)

</div>

---

Brick is a macOS terminal emulator built on [Tauri v2](https://tauri.app/), Rust, Svelte, and SQLite. It logs every command you run locally, gives you AI-assisted suggestions powered by a model of your choice (Ollama, OpenAI-compatible, or AWS Bedrock), and executes agent tasks with a human-in-the-loop approval gate — all without sending a single byte to the cloud unless you explicitly configure it to.

## ✨ Features

- **Full PTY terminal** — native shell sessions with resize support
- **Local-first history** — every block (command + output + metadata) stored in SQLite on your machine
- **AI Agent mode** — natural-language task execution with risk classification and per-command approval
- **Multi-backend LLM** — plug in Ollama, any OpenAI-compatible endpoint, or AWS Bedrock
- **Secret redaction** — API keys and tokens scrubbed from prompts before they reach the LLM
- **Zero telemetry** — no analytics, no cloud sync, no phone home

## 📦 Download & Install

Pre-built `.dmg` releases are available on the [Releases page](https://github.com/anuragbhatt1805/brick/releases).

| Architecture | File |
|---|---|
| Apple Silicon (M1/M2/M3) | `Brick_vX.Y.Z_aarch64.dmg` |
| Intel | `Brick_vX.Y.Z_x64.dmg` |

### Installation steps

1. Download the `.dmg` for your chip and open it
2. Drag **Brick.app** to your **Applications** folder
3. On first launch, macOS Gatekeeper will block the app with a warning like *"Apple cannot verify this app"* — this is expected for apps distributed outside the App Store

**To allow Brick to run:**

> **System Settings → Privacy & Security** → scroll down → click **"Open Anyway"** next to Brick

You'll only need to do this once. After that, Brick launches normally.

> Requires **macOS 12 Monterey** or later.

## 🚀 Quick Start (Development)

See [SETUP.md](SETUP.md) for the full local development guide.

```sh
# Clone
git clone https://github.com/anuragbhatt1805/brick.git
cd brick

# Install dependencies
yarn install

# Start dev server (Rust + Svelte hot-reload)
cargo tauri dev
```

## 🏗️ Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for a deep dive into the codebase structure, data flow, and component responsibilities.

## 🤝 Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow — from setting up your dev environment to submitting a PR.

## 📄 License

[MIT](LICENSE) © Brick Contributors
