# Conventions

## Privacy

- No analytics, telemetry, or unsolicited outbound HTTP to third parties.
- Ollama is expected on a user-controlled URL (default local). WhisperX runs as a local subprocess.
- Meeting audio is removed after processing unless the user enables retention.

## Rust (`#[tauri::command]`)

- Handlers return **`Result<T, String>`** (or another `Result` that maps to a string error for the frontend). Avoid `unwrap()` / `expect()` in command paths.
- Prefer **`?`** and structured `AppError` conversion.

## Comments & docs

- **Code comments** (Rust, TypeScript, Python): **English** only.
- Explain **why** for non-obvious logic; skip noise on trivial lines.
- Rust: `///` on public commands and functions; `//!` on modules where helpful.

## Internationalization (UI)

- All user-visible strings go through **i18next** (`useTranslation`, `t("key")`).
- Add keys to **`brief/src/i18n/locales/de/common.json`** and **`en/common.json`** together — never hardcode UI copy in components.

## TypeScript

- **`strict: true`** — avoid `any` without an explicit, documented reason.

## Agent-specific rules

`brief/CLAUDE.md` at the repository root duplicates some of this for AI assistants and includes ticket workflow (Linear). Keep behavioral rules aligned with this page when they change.
