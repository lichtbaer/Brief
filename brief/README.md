# Brief — lokale Meeting Intelligence

Tauri 2 + React + TypeScript.

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
