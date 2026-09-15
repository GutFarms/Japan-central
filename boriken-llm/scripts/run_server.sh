#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
export PATH="${HOME}/.local/bin:${PATH}"
cd "$ROOT"

python3 train/build_dataset.py
python3 train/train_lm.py --steps "${STEPS:-800}"
python3 -m unittest discover -s tests -v
exec python3 -m uvicorn api.server:app --host 0.0.0.0 --port "${PORT:-8080}"
