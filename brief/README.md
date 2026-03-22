# Brief — lokale Meeting Intelligence

Tauri 2 + React + TypeScript.

## Voraussetzungen

- Rust (stable)
- Node.js 20+
- Ollama (https://ollama.ai) mit `whisper` und `llama3.1:8b` Modellen
- **SQLCipher** (native Bibliothek; wird für die verschlüsselte SQLite-DB gebunden):
  - macOS: `brew install sqlcipher`
  - Debian/Ubuntu: `libsqlcipher-dev` (oder System-SQLCipher mit passenden `pkg-config`-Pfaden)
  - Windows: [SQLCipher Windows Build](https://github.com/sqlcipher/sqlcipher)

Das Release nutzt `libsqlite3-sys` mit Feature `bundled-sqlcipher`, sodass SQLCipher mitgebaut werden kann, sofern die Build-Umgebung (z. B. OpenSSL-Entwicklerpakete) passt.

## WhisperX Setup (Entwicklung)

Transkription und Speaker-Diarization laufen über WhisperX (Python). Einrichtung:

```bash
cd brief/whisperx_runner
bash setup.sh
# Anschließend ggf. die venv aktivieren: source .venv/bin/activate
```

Das Modell `base` wird beim ersten Aufruf heruntergeladen (~150MB). Für pyannote-Diarization kann ein Hugging Face Token nötig sein (`HF_TOKEN` / `huggingface-cli login`).

---

## Tauri Template-Hinweise

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
