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
| State | Zustand, TanStack Query |
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

---

## Project Structure
```
brief/
├── src/                        # React frontend
│   ├── i18n/                   # i18next setup + locale files
│   │   └── locales/de|en/common.json
│   ├── views/                  # RecordingView, OutputView, HistoryView, SettingsView
│   ├── components/             # Shared UI components
│   ├── store/                  # Zustand stores
│   └── types/index.ts          # Shared TypeScript types (Meeting, Segment, etc.)
├── src-tauri/src/              # Rust backend
│   ├── lib.rs                  # All Tauri command registrations
│   ├── audio.rs                # CPAL microphone capture + WAV writing
│   ├── transcribe.rs           # WhisperX subprocess integration
│   └── storage.rs              # SQLCipher database operations
├── whisperx_runner/            # Python AI pipeline
│   ├── whisperx_runner.py      # Transcription + diarization script
│   └── setup.sh                # venv + pip install
└── docs/
    └── DEVELOPMENT.md          # Full developer documentation
```

---

## Tauri Commands

| Command | Description | File |
|---|---|---|
| `start_recording(meeting_type)` | Start microphone capture, returns `session_id` | `audio.rs` |
| `stop_recording(session_id)` | Stop capture, write WAV to temp, returns `audio_path` | `audio.rs` |
| `process_meeting(session_id, audio_path)` | Run WhisperX, returns JSON with segments | `transcribe.rs` |
| `get_meeting(id)` | Load meeting from DB, returns Meeting JSON | `storage.rs` |

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
