#!/usr/bin/env bash
# Prepare the one local, version-pinned PaddleOCR runtime used by linggan-media-worker.
#
# This script deliberately does not start a worker, migrate a database, enqueue historical work,
# or call a remote provider. It only installs the local OCR dependency under the support directory.
set -euo pipefail

if [[ -n "${LINGGAN_SUPPORT_DIR:-}" ]]; then
  support_dir="$LINGGAN_SUPPORT_DIR"
else
  support_dir="$HOME/Library/Application Support/Linggan Intelligence"
fi
python_command="${LINGGAN_PYTHON_COMMAND:-python3}"
venv_dir="$support_dir/paddle-ocr/venv"

if ! command -v "$python_command" >/dev/null 2>&1; then
  echo "Paddle OCR setup failed: Python command is unavailable: $python_command" >&2
  exit 1
fi

mkdir -p "$support_dir/paddle-ocr"
if [[ ! -x "$venv_dir/bin/python" ]]; then
  "$python_command" -m venv "$venv_dir"
fi

"$venv_dir/bin/python" -m pip install --upgrade pip
"$venv_dir/bin/python" -m pip install \
  "paddlepaddle==3.3.1" \
  "paddleocr==3.7.0" \
  "Pillow>=10,<12"
"$venv_dir/bin/python" - <<'PY'
import paddle
import paddleocr
from PIL import Image

print(f"Paddle OCR runtime ready: paddle={paddle.__version__} paddleocr={paddleocr.__version__} pillow={Image.__version__}")
PY

