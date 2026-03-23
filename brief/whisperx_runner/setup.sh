#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
python3 -m venv .venv
# shellcheck disable=SC1091
source .venv/bin/activate
pip install --upgrade pip
pip install whisperx
echo "WhisperX dependencies installed."

if [ -n "${HF_TOKEN:-}" ]; then
  echo "Downloading models (HF_TOKEN detected)…"
  python download_models.py
  echo "WhisperX ready (models bundled)."
else
  echo "HF_TOKEN not set — skipping model download."
  echo "To download models: export HF_TOKEN=\"hf_...\" && python download_models.py"
fi
