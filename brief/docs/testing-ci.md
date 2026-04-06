# Testing & CI

## GitHub Actions (`.github/workflows/ci.yml`)

On each push to `main` and on pull requests, CI runs:

1. **Rust** — `cargo test` in `brief/src-tauri` (after Linux packages for WebKit, GTK, ALSA, SSL, etc.).
2. **Python** — Python **3.11**, `pip install -r requirements-dev.txt`, `pytest` in `brief/whisperx_runner`.
3. **Node** — Node **20**, `npm ci` and `npm run test` (Vitest) in `brief/`.

Mirror those versions locally when debugging “works on my machine” failures.

## Local commands (summary)

| Layer | Working directory | Command |
|-------|-------------------|---------|
| Frontend unit tests | `brief/` | `npm run test` |
| TypeScript | `brief/` | `npm run typecheck` |
| Rust | `brief/src-tauri/` | `cargo test` |
| Python | `brief/whisperx_runner/` | `python3 -m pytest` (after venv + dev deps) |

## Documentation build

From the **repository root**:

```bash
python3 -m pip install -r requirements-docs.txt
mkdocs build --strict
```

HTML output is written to `site/` (ignored by git). Use `mkdocs serve` for a local preview with live reload.
