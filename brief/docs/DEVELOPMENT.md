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
    → Ollama (default `llama3.1:8b`; on ≤8 GB RAM auto `llama3.2:3b` unless overridden — SMA-367)
    → MeetingOutput (stored in SQLCipher DB)
```

### Tauri Commands (Backend Contract)

| Command | Input | Output | Status |
|---|---|---|---|
| `start_recording` | `meeting_type: String` | `session_id: String` | ✅ BRIEF-002 |
| `stop_recording` | `session_id: String` | `audio_path: String` | ✅ BRIEF-002 |
| `process_meeting` | `session_id, audio_path` | JSON with segments | ✅ BRIEF-003 |
| `get_meeting` | `id: String` | Meeting JSON | ✅ BRIEF-004 |
| `get_app_settings_snapshot` | — | `AppSettingsSnapshot` (memory, LLM, onboarding flags) | ✅ SMA-367 |
| `set_llm_model` | `model: String` | — | ✅ SMA-367 |
| `dismiss_low_ram_onboarding` | — | — | ✅ SMA-367 |

## LLM models (8 GB / MacBook-class hardware)

At startup, Brief reads installed RAM (`sysctl hw.memsize` on macOS, `/proc/meminfo` on Linux for dev/CI) and, unless the user has set a manual override, writes the recommended Ollama model id into `settings.llm_model` (`llama3.2:3b` when RAM ≤ 8 GB, otherwise `llama3.1:8b`). Users still run `ollama pull …` themselves.

**Template quality (llama3.2:3b):** Smoke-tested with the same JSON-output templates as `llama3.1:8b`; expect slightly lower precision on long or ambiguous transcripts. Re-run meeting-specific template QA if prompts change (BRIEF-P2-002).

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

## Linux Setup

### Voraussetzungen

```bash
# Ubuntu/Debian
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libssl-dev \
  pkg-config \
  libgtk-3-dev \
  libasound2-dev

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js 20
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
```

### WhisperX Setup

Identisch mit macOS:

```bash
cd brief/whisperx_runner && bash setup.sh
```

### Build

```bash
cd brief && npm install && npm run tauri dev
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
