# Setup

## Prerequisites

- **Rust** (stable) — [rustup](https://rustup.rs)
- **Node.js 20+**
- **Python 3.10+** (CI uses 3.11; see [Testing & CI](testing-ci.md))
- **Ollama** — [ollama.ai](https://ollama.ai); pull the models you plan to use (e.g. `llama3.1:8b`, `llama3.2:3b`)
- **SQLCipher** — native library for encrypted SQLite (platform-specific)

### SQLCipher

- **macOS:** `brew install sqlcipher`
- **Debian/Ubuntu:** `libsqlcipher-dev` (or system SQLCipher with correct `pkg-config` paths)
- **Windows:** see [SQLCipher Windows build](https://github.com/sqlcipher/sqlcipher)

Release builds can use `libsqlite3-sys` with `bundled-sqlcipher` when the build environment provides the needed crypto pieces; see `brief/README.md` for notes.

### Linux desktop dependencies (Tauri / WebKit / audio)

Ubuntu/Debian example:

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libssl-dev \
  pkg-config \
  libgtk-3-dev \
  libasound2-dev
```

The GitHub Actions workflow installs a slightly larger set (including appindicator and `patchelf`); mirror that for a full desktop parity build.

## WhisperX environment

```bash
cd brief/whisperx_runner
bash setup.sh
```

Model download and Hugging Face tokens are covered in [WhisperX & models](whisperx.md).

## Run the app (development)

```bash
cd brief
npm install
npm run tauri dev
```

### Optional: runner path override

Set **`BRIEF_WHISPERX_RUNNER`** to a custom path to the WhisperX script during development.

## Type checks

From `brief/`:

```bash
npm run typecheck    # tsc --noEmit
```

From `brief/src-tauri/`:

```bash
cargo build          # compile check
cargo test           # Rust tests
```

## LLM defaults (low RAM)

Brief reads installed memory (macOS: `sysctl hw.memsize`; Linux: `/proc/meminfo`) and writes a recommended Ollama model into settings when the user has not set a manual override (`llama3.2:3b` when RAM ≤ 8 GiB, otherwise `llama3.1:8b`). Users still run `ollama pull …` themselves.

For template quality notes when using the smaller model, see the product backlog / QA tickets referenced in legacy `DEVELOPMENT.md`.
