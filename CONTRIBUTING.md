# Contributing to Brick

Thank you for your interest in contributing! This guide covers everything you need to go from zero to a merged pull request.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Testing Requirements](#testing-requirements)
- [Pull Request Process](#pull-request-process)
- [Code Style](#code-style)
- [Good First Issues](#good-first-issues)

---

## Code of Conduct

Be kind and respectful. We have zero tolerance for harassment, discrimination, or personal attacks. All contributors are expected to follow the standard [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).

---

## Development Setup

Follow [SETUP.md](SETUP.md) to get your local development environment running. You need:

- macOS 12+
- Rust stable (≥ 1.80)
- Node.js 24 + Yarn
- Tauri CLI v2

```sh
git clone https://github.com/anuragbhatt1805/brick.git
cd brick
yarn install
cargo tauri dev
```

---

## Project Structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for a detailed breakdown. The short version:

| Path | Language | Purpose |
|---|---|---|
| `src/` | Svelte + TypeScript | Frontend UI |
| `src-tauri/src/` | Rust | Backend logic (PTY, DB, LLM, agent) |
| `src-tauri/migrations/` | SQL | Database schema migrations |
| `scripts/` | zsh | Shell integration hook |
| `.github/workflows/` | YAML | CI / release pipelines |

---

## Making Changes

### 1. Fork and branch

```sh
# Fork on GitHub, then:
git clone https://github.com/YOUR_USERNAME/brick.git
cd brick
git checkout -b feat/your-feature-name
# or
git checkout -b fix/short-description
```

Branch naming conventions:
- `feat/` — new features
- `fix/` — bug fixes
- `chore/` — tooling, deps, refactors
- `docs/` — documentation only

### 2. Make your changes

Keep commits small and focused. Each commit should leave the project in a buildable, passing state.

```sh
git add -p   # stage only what you intend to commit
git commit -m "feat(agent): add tool call retry on timeout"
```

Commit message format: `type(scope): short imperative description`

### 3. Keep your branch up to date

```sh
git fetch upstream
git rebase upstream/main
```

---

## Testing Requirements

**Every PR must pass all of the following.** CI will fail if any check is red.

### Rust

```sh
# Format (auto-fix with: cargo fmt --manifest-path src-tauri/Cargo.toml)
cargo fmt --manifest-path src-tauri/Cargo.toml --check

# Lint (no warnings allowed in production code)
cargo clippy --manifest-path src-tauri/Cargo.toml --lib --bins -- -D warnings

# Unit tests (must all pass)
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

### Frontend

```sh
# Unit tests
yarn test

# Svelte type checker (0 errors required)
yarn run check

# Production build (must succeed)
yarn build
```

### When Adding New Rust Functionality

- Add a `#[cfg(test)]` module with at minimum a happy-path and an error-path test.
- Mock network calls with `wiremock` (already a dev-dependency).
- Avoid `.unwrap()` and `.expect()` in non-test production code — the workspace enforces `clippy::unwrap_used = "deny"`.

### When Adding New Svelte Components

- Add a corresponding `*.test.ts` file using Vitest + `@testing-library/svelte`.
- Mock Tauri IPC calls with `vi.mock('@tauri-apps/api/core', ...)`.

---

## Pull Request Process

1. **Open a draft PR early** — this lets maintainers give feedback before you invest too much time.

2. **Fill out the PR template** — describe what you changed, why, and how to test it.

3. **Ensure CI is green** — all jobs in `ci.yml` must pass:
   - `rust-lint` — `cargo fmt` + `cargo clippy`
   - `rust-test` — `cargo test`
   - `rust-audit` — `cargo audit` (no critical CVEs)
   - `frontend-test` — `yarn test` + `yarn check` + `yarn build`
   - `tauri-build-check` — full debug build

4. **Request a review** — once CI is green and you're happy with the changes, mark the PR as ready and request a review from a maintainer.

5. **Address review comments** — push fixup commits; don't force-push (it loses comment context).

6. **Squash and merge** — a maintainer will squash your commits into one clean commit on `main`.

---

## Code Style

### Rust

- Follow the standard Rust style enforced by `rustfmt`. Config in [`rustfmt.toml`](rustfmt.toml).
- Use `anyhow::Result` for fallible functions that cross module boundaries.
- Use `thiserror` for typed errors in public APIs.
- All `async` functions interacting with the DB or PTY must be `tokio`-async.
- **No `unsafe` code.** The workspace bans it with `unsafe_code = "forbid"`.

### Svelte / TypeScript

- Use `<script lang="ts">` — TypeScript is required, plain JS is not accepted.
- Component files: `PascalCase.svelte`
- Keep business logic out of `.svelte` files — extract into `.ts` modules.
- Prefer `on:event` over inline handlers for testability.

### SQL Migrations

- Add new migrations as `src-tauri/migrations/NNN_description.sql`.
- Never modify an existing migration — always add a new one.
- Reference the new migration in `database.rs::MIGRATIONS`.

---

## Good First Issues

Look for issues tagged [`good first issue`](https://github.com/anuragbhatt1805/brick/issues?q=label%3A%22good+first+issue%22) on GitHub. These are intentionally small and well-scoped for new contributors.

Some areas where contributions are especially welcome:

- **Shell hooks** — fish shell support (`brick_hook.fish`)
- **Risk classifier** — add more `Confirm` patterns for dangerous but common commands
- **UI polish** — accessibility improvements in `AgentPanel.svelte` (there's a known a11y warning on the drag handle)
- **Tests** — increase coverage in `database.rs` and `agent.rs`
- **Documentation** — improve inline code comments and doc-comments (`///`)

---

## Questions?

Open a [GitHub Discussion](https://github.com/anuragbhatt1805/brick/discussions) or file an [issue](https://github.com/anuragbhatt1805/brick/issues). We're happy to help.
