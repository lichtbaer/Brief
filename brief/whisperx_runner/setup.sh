#!/bin/bash
# Entwicklungs-Setup — für Produktion wird gebündelt
set -euo pipefail
cd "$(dirname "$0")"
python3 -m venv .venv
# shellcheck disable=SC1091
source .venv/bin/activate
pip install whisperx
echo "WhisperX bereit. Start: python whisperx_runner.py <wav_path> [language] [model_size]"
