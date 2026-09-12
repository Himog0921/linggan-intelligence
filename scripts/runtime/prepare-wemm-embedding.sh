#!/usr/bin/env bash
set -euo pipefail

# Installs the one fixed local embedding runtime.  It is intentionally explicit: a worker start
# checks this artifact but never downloads a model, so a user-created Run cannot turn into an
# unbounded network or disk operation.

support_dir="${LINGGAN_SUPPORT_DIR:-$HOME/Library/Application Support/Linggan Intelligence}"
runtime_dir="${LINGGAN_WEMM_RUNTIME_DIR:-$support_dir/wemm-embedding-2b}"
model_dir="$runtime_dir/model"
venv_dir="$runtime_dir/venv"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
model_id="Tencent/WeMM-Embedding-2B"
model_revision="bbd6cd4bf52cfc6716f752a2df80b2706720bd95"

check() {
  [[ -x "$venv_dir/bin/python" ]] || { echo "WeMM venv missing: $venv_dir" >&2; return 1; }
  [[ -f "$model_dir/model.safetensors" ]] || { echo "WeMM model snapshot missing: $model_dir" >&2; return 1; }
  "$venv_dir/bin/python" - <<'PY'
import torch, transformers, sentence_transformers, torchvision, qwen_vl_utils
assert torch.backends.mps.is_built()
assert transformers.__version__ == "5.2.0"
PY
  echo "WeMM local runtime ready: $model_id@$model_revision"
}

case "${1:-}" in
  --check) check ;;
  --install)
    command -v uv >/dev/null || { echo "uv is required for the local WeMM runtime" >&2; exit 1; }
    mkdir -p "$runtime_dir"
    uv venv --python 3.12 "$venv_dir"
    uv pip install --python "$venv_dir/bin/python" -r "$repo_root/apps/pi-adapter/wemm-requirements.txt"
    "$venv_dir/bin/python" - <<PY
from huggingface_hub import snapshot_download
snapshot_download(repo_id="$model_id", revision="$model_revision", local_dir="$model_dir")
PY
    check
    ;;
  *) echo "usage: $0 {--check|--install}" >&2; exit 2 ;;
esac
