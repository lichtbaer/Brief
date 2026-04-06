# CLAUDE.md — Brief Agent Context

This file is read automatically by Cursor and other AI coding agents.
Always follow these rules. Do not deviate without explicit instruction.

---

## Project Overview

**Brief** is a local-first meeting intelligence desktop app built with Tauri 2 (Rust backend + React/TypeScript frontend). All audio processing and AI inference runs locally — no data ever leaves the device.

- **Repo:** https://github.com/lichtbaer/Brief
- **Linear Project:** Brief (workspace: Smartml)
- **Architecture Decision:** ADR-009 (Tauri/Rust stack), ADR-010 (WhisperX transcription)

---

## Stack

| Layer | Technology |
|---|---|
| Frontend | React 18, TypeScript 5 (strict), Vite 5 |
| State | React hooks (useReducer, useState) |
| i18n | i18next + react-i18next |
| Backend | Rust, Tauri 2 |
| Audio | CPAL 0.15 (microphone capture) |
| AI Pipeline | WhisperX (Python subprocess) + Ollama llama3.1:8b |
| Storage | SQLite + SQLCipher (encrypted) |

---

## Critical Rules

### 1. Privacy — Non-Negotiable
- **No external network calls.** Ollama runs on `localhost:11434`. WhisperX runs as a local subprocess.
- Never add analytics, telemetry, or any outbound HTTP call to external services.
- Audio files are deleted after transcription by default.

### 2. No Panics in Tauri Commands
Every `#[tauri::command]` must return `Result<T, String>`. Never use `.unwrap()` or `.expect()` in command handlers — use `?` or `.map_err()`.

### 3. English Comments Only
All code comments (Rust `//`, Python `#`, TypeScript `//`) must be in English.
User-facing strings go in `src/i18n/locales/{de,en}/common.json` — never hardcoded.

### 4. i18n for All User-Facing Strings
Use `t("key")` from `useTranslation()` for every string shown to the user.
Add new keys to both `de/common.json` AND `en/common.json`.

### 5. TypeScript Strict Mode
`tsconfig.json` has `"strict": true`. No `any` types without explicit justification.

### 6. Code Must Be Commented
All non-trivial code must have inline comments explaining **why**, not just **what**.

**Required comments:**
- Every `pub fn` and `#[tauri::command]` in Rust: doc comment (`///`) explaining purpose, parameters, return value
- Every module (`//!` at top of each `.rs` file): what this module does
- Non-obvious logic blocks: why this approach was chosen
- All error handling decisions: why this error is handled this way
- Python functions in `whisperx_runner.py`: docstrings
- React components: JSDoc comment explaining purpose and props
- State stores: comment on each slice explaining its role

**What NOT to comment:**
- Self-explanatory variable assignments
- Simple getters/setters
- Code that already reads like plain English

**Example (Rust):**
```rust
/// Resamples audio from the device's native sample rate to 16kHz mono.
/// WhisperX requires 16kHz mono input — without resampling, transcription
/// quality degrades significantly on devices with 44.1kHz or 48kHz default rates.
fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
```

**Example (TypeScript):**

```typescript
/**
 * RecordingView — main recording interface.
 * Handles the full recording lifecycle: idle → recording → processing → done/error.
 * Communicates with Rust backend via Tauri invoke() calls.
 */
export function RecordingView() {
```

---

## Project Structure
```
brief/
├── src/                        # React frontend
│   ├── i18n/                   # i18next setup + locale files
│   │   └── locales/de|en/common.json
│   ├── views/                  # RecordingView, OutputView, HistoryView, SettingsView
│   ├── components/             # Shared UI components
│   ├── store/                  # State management (future)
│   └── types/index.ts          # Shared TypeScript types (Meeting, Segment, etc.)
├── src-tauri/src/              # Rust backend
│   ├── lib.rs                  # App setup + command registration
│   ├── commands/               # Tauri command handlers (recording, export, meetings, settings, health)
│   ├── audio.rs                # CPAL microphone capture + WAV writing
│   ├── transcribe.rs           # WhisperX subprocess integration
│   └── storage.rs              # SQLCipher database operations
├── whisperx_runner/            # Python AI pipeline
│   ├── whisperx_runner.py      # Transcription + diarization script
│   └── setup.sh                # venv + pip install
└── docs/                       # MkDocs sources (build from repo root: mkdocs.yml)
    ├── index.md
    ├── tauri-commands.md       # Full invoke API (keep in sync with lib.rs)
    └── DEVELOPMENT.md          # Legacy single-page overview
```

---

## Tauri Commands

Handlers live in `src-tauri/src/commands/` and are registered in `lib.rs`. The **authoritative list** is in `docs/tauri-commands.md` (MkDocs). Legacy summary:

| Command | Description | Module |
|---|---|---|
| `start_recording(meeting_type)` | Start microphone capture, returns `session_id` | `commands/recording.rs` |
| `stop_recording(session_id)` | Stop capture, write WAV to temp, returns `audio_path` | `commands/recording.rs` |
| `process_meeting(...)` | Run WhisperX + summarization, persist meeting | `commands/recording.rs` |
| `get_meeting(id)` | Load meeting from DB, returns Meeting JSON | `commands/meetings.rs` |

---

## Data Types

Core types are defined in `src/types/index.ts`:
- `Meeting` — full meeting record
- `MeetingOutput` — AI-generated analysis
- `DiarizedSegment` — `{ speaker, start, end, text }`
- `MeetingType` — `"consulting" | "legal" | "internal" | "custom"`

---

## WhisperX Setup (Development)

```bash
cd whisperx_runner
bash setup.sh
# First run downloads ~150MB base model automatically
```

Set `BRIEF_WHISPERX_RUNNER` env var to override script path during development.

---

## Common Pitfalls

1. `cpal::Stream` is not `Send` on Linux — audio recording uses a dedicated thread with a channel. Do not move Stream into AppState directly.
2. **Tauri 2 has no** `shell-open` feature flag — it's a separate plugin. Use `tauri = { version = "2", features = [] }`.
3. `reqwest` uses `rustls-tls` — avoid `openssl-sys` dependency. Keep `default-features = false`.
4. **SQLCipher requires system library** — `brew install sqlcipher` on macOS before building.
5. **WhisperX** `DiarizationPipeline` is imported from `whisperx.diarize`, not `whisperx` directly.

---

## Ticket Workflow

Tickets are managed in Linear (project: Brief, workspace: Smartml).

* Always reference the ticket ID in commit messages: `feat(SMA-352): ...`
* Branch naming: `cursor/SMA-{number}-short-description`
* Post implementation summary as a comment on the Linear ticket when done
