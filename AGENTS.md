# AGENTS.md

Instructions for AI coding agents (Cursor, Antigravity, Grok Bot) working on this repository. Read this before changing code. Prefer `README.md`, `docs/architecture.md`, `docs/design.md`, and `docs/roadmap.md` for detail.

## Project summary

Forgeline is a **local-first multi-agent AI workspace** (Tauri 2 + React desktop, Rust core). A **project** is a conversation that can hold several source repos (e.g. C# backend + React web).

**Phase 1** (current): app window, create/rename/delete projects, import/connect sources (local path or git URL), Graphify-like per-source analysis (tree-sitter + optional Roslyn / JS-TS module resolution), and a unified cross-source project graph view (exact edges only; cross-source link kinds are reserved, not yet emitted).

Layout:

- `crates/forgeline-core` — projects, sources, SQLite, overview
- `crates/forgeline-graph` — tree-sitter extractors, JS/TS resolution, Roslyn merge
- `crates/forgeline-cli` — `forgeline` binary
- `apps/desktop` — React + Vite + Tauri 2
- `tools/forgeline-roslyn` — .NET 8 Roslyn helper (optional exact C# layer)
- `examples/sample`, `docs/`

## Non-negotiable decisions

1. **Exact edges only.** Store only confirmed/exact edges (`provenance=exact`, `confidence=1.0`). Never invent inferred or guessed links. Unresolved references are **counted**, not stored as edges.
2. **Focus languages:** C# (ASP.NET-style backend) and JavaScript/TypeScript React web. Python may still parse; do not expand it unless asked.
3. **Project names** are unique case-insensitively (`COLLATE NOCASE`). Duplicate/invalid names must surface a clear **Vietnamese** error.
4. **One shared SQLite database** (`forgeline.db`) for all projects under the data dir (override with `FORGELINE_DATA_DIR` or CLI `--db`).
5. **Agent summaries are not bilingual** — one language only (project → source → module → symbol).
6. **Git clones:** `git clone --depth 1`, no submodules; no automatic fetch on re-index; never install git for the user.
7. **UI copy is Vietnamese.** Code, comments, commit messages, and PR text are **English**.

## Security

- Persist local data only under `FORGELINE_DATA_DIR` (defaults: Linux `~/.local/share/forgeline`, macOS `~/Library/Application Support/forgeline`, Windows `%APPDATA%\forgeline`).
- No telemetry, no auto-update, no network beyond user-initiated git clone of a vetted URL.
- Never spawn commands through a shell; use `Command` + explicit argv (git, Roslyn). Parsers must not execute analyzed code.
- No secrets in the repo, logs, or commit history. Reject `https` URLs with userinfo.

## How to build / test

Rust CI (default members: the three crates; desktop is in the workspace but not default-members):

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Frontend typecheck + bundle (no full Tauri WebView deps required for this check):

```bash
cd apps/desktop && npm install && npm run build
```

Optional Roslyn helper: `dotnet build tools/forgeline-roslyn -c Release` (.NET SDK 8).

CLI examples (from repo root):

```bash
cargo run -p forgeline-cli -- index examples/sample --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- stats --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- explain App --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- callers Find --project sample --db .forgeline/forgeline.db
cargo run -p forgeline-cli -- overview sample --project sample --db .forgeline/forgeline.db
```

`callers` / `overview` return exact edges only. Omit `--db` to use the OS data directory (same as the desktop app).

## Multi-agent workflow

- Branch names: `cursor/...`, `antigravity/...`, or the agent’s own prefix. Keep PRs small and targeted at `main`.
- Rebase onto latest `main` before opening/updating a PR. Never force-push `main`.
- One agent per area at a time (avoid parallel edits to the same crate/feature).
- UI work must follow `docs/design.md`.
- PR description: what you touched, why, and how you verified. No `Co-authored-by` trailers.
- Commits authored only by the repo owner’s GitHub identity or the agent’s own identity.

## Definition of done

- CI green (`fmt`, `clippy -D warnings`, `cargo test`).
- Tests cover new behavior; docs (`README` / `docs/*` / this file) updated when behavior or workflow changes.
- When touching the graph engine, report accuracy numbers: **exact edges vs unresolved** (and ambiguous if relevant) for the languages you changed.
