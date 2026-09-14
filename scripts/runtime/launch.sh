#!/bin/zsh
# 常驻服务的统一启动入口。launchd 调用它，参数是要跑的二进制名。
#
#   launch.sh linggan-api
#   launch.sh linggan-worker
#   launch.sh linggan-media-worker
#
# 三个服务共用同一份同步逻辑，因此它们永远跑同一个 revision。
#
# 整段包在大括号里，理由同 sync.sh：同步会覆盖包括本文件在内的工作树，大括号让 zsh
# 先完整解析再执行。
{
set -euo pipefail

binary="${1:?用法: launch.sh <linggan-api|linggan-worker|linggan-media-worker>}"

: "${LINGGAN_RUNTIME_DIR:=$HOME/Library/Application Support/Linggan Intelligence/runtime-main}"
: "${LINGGAN_SUPPORT_DIR:=$HOME/Library/Application Support/Linggan Intelligence}"
export LINGGAN_RUNTIME_DIR

# 同步跑在**临时副本**上。它会 reset --hard 覆盖工作树里的自己，而 zsh 读脚本是惰性的；
# 从副本执行就没有这个窗口。
sync_copy="$(mktemp -t linggan-sync)"
trap 'rm -f "$sync_copy"' EXIT
cp "$LINGGAN_RUNTIME_DIR/scripts/runtime/sync.sh" "$sync_copy"
/bin/zsh "$sync_copy"

cd "$LINGGAN_RUNTIME_DIR"
set -a
source ./.env
set +a

export LINGGAN_LOCAL_DATABASE_URL="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT}/${POSTGRES_DB}"
export LINGGAN_LOCAL_MEDIA_ROOT="${LINGGAN_SUPPORT_DIR}/media"

# 端口只对 API 有意义；worker 读到也无害。
: "${LINGGAN_LOCAL_PORT:=3000}"
export LINGGAN_LOCAL_PORT

if [[ "$binary" == "linggan-api" ]]; then
  # Do not expose a derivation-version read contract until its deterministic inputs are ready.
  # This command has no model/provider path; a bounded failure stops the new API before it binds.
  ./target/debug/linggan-comment-worker --derive-current
fi

# Resolve the fixed Node executable independently of launchd PATH.
: "${LINGGAN_PI_NODE:=$HOME/.nvm/versions/node/v$(cat .nvmrc)/bin/node}"
export LINGGAN_PI_NODE
./scripts/runtime/prepare-pi-adapter.sh --check
if [[ "$binary" == "linggan-worker" ]]; then
  # Explicit artifact verification only: no model download or profile mutation during a Run.
  ./scripts/runtime/prepare-wemm-embedding.sh --check
fi
exec "./target/debug/${binary}"
}
