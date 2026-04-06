# Brief — Developer documentation

**Brief** is a local-first desktop app for meeting capture, transcription, and analysis. The UI is **Tauri 2** (Rust + WebView); the frontend is **React 18** and **TypeScript** (Vite). Transcription and speaker diarization run in a local **WhisperX** Python subprocess; summaries use **Ollama** on the machine (default `localhost`).

!!! note "Privacy"
    The product design assumes **no outbound analytics or cloud APIs**: audio and models stay on the device except where you explicitly configure Ollama to listen on another host.

## Where things live in the repo

| Path | Role |
|------|------|
| `brief/src/` | React app, views, components, i18n |
| `brief/src-tauri/src/` | Rust backend, Tauri commands, audio, DB, WhisperX bridge |
| `brief/whisperx_runner/` | Python runner, tests, model download scripts |
| `brief/README.md` | Short setup for app developers (also linked from the repo root) |
| `brief/CLAUDE.md` | Rules and context for AI coding agents (keep in sync with conventions here) |

## Quick start

```bash
cd brief
npm install
npm run tauri dev
```

Prerequisites and WhisperX setup are in [Setup](setup.md) and [WhisperX & models](whisperx.md).

## Browse this site

- [Architecture](architecture.md) — stack and data flow  
- [Tauri commands](tauri-commands.md) — full `invoke` API surface  
- [Testing & CI](testing-ci.md) — how the pipeline maps to local commands  
- [Conventions](conventions.md) — i18n, errors, comments  
- [ADRs](adrs.md) — recorded architecture decisions  

The legacy single-file overview remains available as [DEVELOPMENT.md](DEVELOPMENT.md) (subset of these pages; may lag behind [Tauri commands](tauri-commands.md)).
