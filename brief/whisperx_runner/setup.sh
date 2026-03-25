#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"

# Pre-flight checks
if ! command -v python3 &> /dev/null; then
  echo "Error: python3 not found. Install Python 3.9+ first." >&2
  echo "  Ubuntu/Debian: sudo apt-get install -y python3 python3-venv python3-pip" >&2
  exit 1
fi

PYTHON_VERSION=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
if python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)' 2>/dev/null; then
  echo "Python ${PYTHON_VERSION} detected."
else
  echo "Error: Python 3.9+ required, found ${PYTHON_VERSION}." >&2
  exit 1
fi

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
