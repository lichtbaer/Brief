#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
python3 -m venv .venv
# shellcheck disable=SC1091
source .venv/bin/activate
pip install whisperx
echo "WhisperX bereit."
