# Architecture

## Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 18, TypeScript 5 (strict), Vite 5 |
| UI state | React hooks (`useState`, `useReducer`, …) |
| i18n | i18next + react-i18next (`brief/src/i18n/locales/{de,en}/`) |
| Desktop shell | Tauri 2 |
| Audio capture | CPAL (Rust, `brief/src-tauri/src/audio.rs`) |
| Transcription | WhisperX via Python subprocess (`whisperx_runner/`) |
| Summaries | Ollama HTTP API (configurable URL; default local) |
| Storage | SQLite + SQLCipher (`brief/src-tauri/src/storage.rs`) |

Tauri plugins in use include **dialog**, **fs**, and **os** (see `brief/src-tauri/Cargo.toml` and `lib.rs`).

## Data flow (high level)

```text
Microphone → CPAL (Rust) → temp WAV → whisperx_runner.py → segments JSON
                                                      ↓
                                            Ollama (summary)
                                                      ↓
                                              SQLCipher DB
```

1. **Record** — Rust records to a temporary WAV via CPAL (`start_recording` / `stop_recording` in `commands/recording.rs`).
2. **Transcribe** — `process_meeting` runs WhisperX, merges diarized segments, then calls Ollama for structured output.
3. **Persist** — Meeting JSON is stored in the encrypted database; WAV may be deleted or kept per **retain audio** settings.

## Rust module layout (conceptual)

| Area | Typical modules |
|------|-----------------|
| Tauri entry & state | `lib.rs`, `state.rs` |
| Commands (IPC surface) | `commands/{recording,meetings,settings,export,health,audio}.rs` |
| Audio / pipeline | `audio.rs`, `transcribe.rs`, `summarize.rs`, `templates.rs` |
| Persistence | `storage.rs`, `crypto_key.rs` |
| Recovery / UX edge cases | `recovery.rs`, `memory.rs`, `defaults.rs` |

The authoritative list of exposed `invoke` handlers is registered in `brief/src-tauri/src/lib.rs` and documented in [Tauri commands](tauri-commands.md).

## LLM model selection (RAM)

At startup the backend reads available RAM and, unless the user has overridden the model, applies a recommended Ollama model id (smaller model on low-RAM machines). Details and UI hooks are described in [Setup — LLM defaults](setup.md#llm-defaults-low-ram).
