# Brief — Developer Documentation

## Architecture Overview

Brief is a Tauri 2 desktop application combining a Rust backend with a React/TypeScript frontend.

### Stack

- **Frontend:** React 18, TypeScript 5, Vite 5, Zustand, TanStack Query, i18next
- **Backend:** Rust, Tauri 2
- **AI Pipeline:** WhisperX (Python subprocess) for transcription + diarization, Ollama (llama3.1:8b) for analysis
- **Storage:** SQLite + SQLCipher (encrypted local database)

### Data Flow

```
Microphone → CPAL (Rust) → WAV file (temp)
    → whisperx_runner.py (Python subprocess)
    → JSON segments {speaker, start, end, text}
    → process_meeting Tauri command
    → Ollama llama3.1:8b (summarization, BRIEF-004)
    → MeetingOutput (stored in SQLCipher DB)
```

### Tauri Commands (Backend Contract)

| Command | Input | Output | Status |
|---|---|---|---|
| `start_recording` | `meeting_type: String` | `session_id: String` | ✅ BRIEF-002 |
| `stop_recording` | `session_id: String` | `audio_path: String` | ✅ BRIEF-002 |
| `process_meeting` | `session_id, audio_path` | JSON with segments | ✅ BRIEF-003 |
| `get_meeting` | `id: String` | Meeting JSON | ✅ BRIEF-004 |

## Local Setup

### Prerequisites

- Rust (stable): https://rustup.rs
- Node.js 20+
- Python 3.10+
- Ollama: https://ollama.ai — pull `llama3.1:8b`
- macOS: `brew install sqlcipher`

### WhisperX Setup

```bash
cd brief/whisperx_runner
bash setup.sh
```

### Run in Development

```bash
cd brief
npm install
npm run tauri dev
```

### Type Check

```bash
npm run typecheck       # tsc --noEmit
cargo build             # Rust compile check
```

Run these from `brief/` for npm. For Rust, use `brief/src-tauri` as the working directory (e.g. `cd brief/src-tauri && cargo build`).

## Project Structure

```
brief/
├── src/                    # React frontend
│   ├── i18n/               # Internationalization (i18next)
│   ├── views/              # Main views
│   ├── components/         # Shared components
│   ├── store/              # Zustand state
│   └── types/index.ts      # Shared TypeScript types
├── src-tauri/src/          # Rust backend
│   ├── lib.rs              # Tauri commands
│   ├── audio.rs            # CPAL microphone capture
│   ├── transcribe.rs       # WhisperX subprocess
│   └── storage.rs          # SQLCipher database
└── whisperx_runner/        # Python AI pipeline
    ├── whisperx_runner.py
    └── setup.sh
```

## Conventions

* **Code comments:** English only
* **User-facing strings:** Via i18n keys (src/i18n/locales/)
* **Error handling:** No panics in Tauri commands — always return `Result<T, String>`
* **Privacy:** No network calls except localhost (Ollama, WhisperX)
* **Audio files:** Deleted after transcription by default (opt-in retention)

## Key Decisions (ADRs)

* **ADR-009:** Tauri/Rust stack instead of NexCore standard (Python/FastAPI) — privacy + native desktop requirement
* **ADR-010:** WhisperX as transcription backend — Ollama no longer provides official Whisper model (March 2026)

## Known Issues & Debt

See Linear project "Brief" for open tickets and technical debt.
