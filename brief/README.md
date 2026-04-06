# Brief — lokale Meeting Intelligence

Tauri 2 + React + TypeScript.

## Entwicklerdokumentation

Aus dem **Repository-Root** (nicht aus `brief/`):

```bash
python3 -m pip install -r requirements-docs.txt
python3 -m mkdocs serve
```

Strukturierte Doku (Architektur, alle Tauri-Commands, CI): Quellen unter [`docs/`](docs/index.md) — gebaute Version mit Suche und Navigation über MkDocs.

## Voraussetzungen

- Rust (stable)
- Node.js 20+
- Ollama (https://ollama.ai) mit passenden Chat-Modellen (z. B. `llama3.1:8b`; bei wenig RAM siehe Doku zu `llama3.2:3b`)
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
```

### Modelle herunterladen (einmalig)

Die pyannote-Diarization-Modelle liegen hinter einer HuggingFace-Lizenzschranke.
Akzeptiere die Lizenzen und lade die Modelle einmalig mit deinem HF-Token herunter:

1. Lizenzen akzeptieren: [pyannote/speaker-diarization-3.1](https://huggingface.co/pyannote/speaker-diarization-3.1) und [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0)
2. Token erstellen: https://huggingface.co/settings/tokens

```bash
cd brief/whisperx_runner
source .venv/bin/activate
export HF_TOKEN="hf_..."
python download_models.py
```

Die Modelle landen in `whisperx_runner/models/` (~170 MB) und werden im App-Bundle mitgeliefert.
Endbenutzer brauchen keinen HuggingFace-Account.

---

## Tauri Template-Hinweise

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
