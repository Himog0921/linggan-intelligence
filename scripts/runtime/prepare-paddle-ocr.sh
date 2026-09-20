#!/usr/bin/env bash
# Prepare and verify the fixed local PaddleOCR runtime used by linggan-media-worker.
# The worker never installs packages or fetches models. This script does not migrate a database,
# enqueue historical work, start a worker, or contact a content platform.
set -euo pipefail

support_dir="${LINGGAN_SUPPORT_DIR:-$HOME/Library/Application Support/Linggan Intelligence}"
runtime_dir="$support_dir/paddle-ocr"
venv_dir="$runtime_dir/venv"
cache_dir="$runtime_dir/cache"

check() {
  [[ -x "$venv_dir/bin/python" ]] || { echo "Paddle OCR venv missing: $venv_dir" >&2; return 1; }
  [[ -d "$cache_dir/official_models/PP-OCRv4_mobile_det" ]] || { echo "Paddle OCR detection model missing" >&2; return 1; }
  [[ -d "$cache_dir/official_models/PP-OCRv4_mobile_rec" ]] || { echo "Paddle OCR recognition model missing" >&2; return 1; }
  PADDLE_PDX_CACHE_HOME="$cache_dir" "$venv_dir/bin/python" - <<'PY'
import paddle
import paddleocr
from PIL import Image

assert paddle.__version__ == "3.3.1"
assert paddleocr.__version__ == "3.7.0"
assert Image.__version__.split(".")[0] in {"10", "11"}
PY
  echo "Paddle OCR local runtime ready: paddle=3.3.1 paddleocr=3.7.0"
}

find_supported_python() {
  local candidate candidate_path
  for candidate in "${LINGGAN_PADDLE_PYTHON_COMMAND:-}" /opt/homebrew/bin/python3.13 /opt/homebrew/bin/python3.12 python3.13 python3.12 python3.11 python3.10 python3.9; do
    [[ -n "$candidate" ]] || continue
    candidate_path="$(command -v "$candidate" 2>/dev/null || true)"
    [[ -n "$candidate_path" ]] || continue
    if "$candidate_path" - <<'PY' >/dev/null 2>&1
import sys
raise SystemExit(not ((3, 9) <= sys.version_info[:2] <= (3, 13)))
PY
    then
      printf '%s\n' "$candidate_path"
      return 0
    fi
  done
  echo "Paddle OCR requires Python 3.9 through 3.13; set LINGGAN_PADDLE_PYTHON_COMMAND to a compatible interpreter" >&2
  return 1
}

install() {
  local python_command
  python_command="$(find_supported_python)"
  mkdir -p "$runtime_dir"
  "$python_command" -m venv "$venv_dir"
  "$venv_dir/bin/python" -m pip install --upgrade pip
  "$venv_dir/bin/python" -m pip install \
    "paddlepaddle==3.3.1" \
    "paddleocr==3.7.0" \
    "Pillow>=10,<12"
  # Construct the same pipeline as the bridge so model downloads happen before any leased job.
  PADDLE_PDX_CACHE_HOME="$cache_dir" "$venv_dir/bin/python" - <<'PY'
from paddleocr import PaddleOCR

PaddleOCR(
    lang="ch",
    ocr_version="PP-OCRv4",
    use_doc_orientation_classify=False,
    use_doc_unwarping=False,
    use_textline_orientation=False,
)
PY
  check
}

case "${1:-}" in
  --install) install ;;
  --check) check ;;
  *) echo "usage: $0 {--install|--check}" >&2; exit 2 ;;
esac
