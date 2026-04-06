# Brief — Developer documentation (legacy single page)

This file is kept for deep links and external references. **Prefer the MkDocs site** (start at [index.md](index.md)) for navigation, search, and up-to-date command lists.

## Architecture overview

Brief is a Tauri 2 desktop application: Rust backend + React/TypeScript frontend. See [Architecture](architecture.md) for the current stack table and data-flow diagram.

### Data flow (text)

```
Microphone → CPAL (Rust) → WAV file (temp)
    → whisperx_runner.py (Python subprocess)
    → JSON segments { speaker, start, end, text }
    → process_meeting Tauri command
    → Ollama (default local; configurable URL)
    → MeetingOutput (stored in SQLCipher DB)
```

## Tauri commands

The authoritative list is in [Tauri commands](tauri-commands.md) (generated from `brief/src-tauri/src/lib.rs` registration).

## LLM models (8 GB / MacBook-class hardware)

At startup, Brief reads installed RAM and, unless the user has set a manual override, writes the recommended Ollama model id into `settings.llm_model` (`llama3.2:3b` when RAM ≤ 8 GB, otherwise `llama3.1:8b`). Users still run `ollama pull …` themselves.

## Local setup

See [Setup](setup.md) and [WhisperX & models](whisperx.md).

## Project structure

```
brief/
├── src/                    # React frontend
│   ├── i18n/
│   ├── views/
│   ├── components/
│   └── types/index.ts
├── src-tauri/src/
│   ├── lib.rs
│   ├── commands/           # Tauri command handlers
│   ├── audio.rs
│   ├── transcribe.rs
│   ├── storage.rs
│   └── ...
└── whisperx_runner/
    ├── whisperx_runner.py
    └── setup.sh
```

## Conventions

See [Conventions](conventions.md).

## Key decisions (ADRs)

See [ADRs](adrs.md).

## Known issues & debt

See Linear project "Brief" for open tickets and technical debt.
