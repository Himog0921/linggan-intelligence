#!/usr/bin/env bash
# WEB-REACT-001（DEC-0010）：为当前 checkout 准备 React 前端产物（apps/web/dist）。
# 不碰数据库、不启动服务。同一个 revision 已构建过就直接复用，因此离线重启不需要网络；
# 只有锁文件变化时才重新安装依赖（npm ci 需要网络或已缓存的包）。
#
#   prepare-web.sh --build <revision>   需要时安装依赖并构建，最后核对产物版本
#   prepare-web.sh --check <revision>   只核对产物是否存在且版本一致
set -euo pipefail
web_repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
web_mode="${1:-}"
web_revision="${2:-}"
[[ ( "$web_mode" == --build || "$web_mode" == --check ) && -n "$web_revision" ]] \
  || { echo 'usage: prepare-web.sh [--build|--check] <revision>' >&2; exit 1; }
[[ "$web_revision" =~ ^[A-Za-z0-9._-]{1,64}$ ]] || { echo "web: invalid revision '$web_revision'" >&2; exit 1; }

# 固定的 Node，与 Pi Adapter 同一个，不依赖 launchd 的 PATH。
web_node_version="$(cat "$web_repo/.nvmrc")"
web_node="${LINGGAN_WEB_NODE:-${LINGGAN_PI_NODE:-$HOME/.nvm/versions/node/v${web_node_version}/bin/node}}"
[[ -x "$web_node" ]] || { echo "web: install Node ${web_node_version} or set LINGGAN_WEB_NODE to its absolute executable path" >&2; exit 1; }
[[ "$("$web_node" --version)" == "v${web_node_version}" ]] || { echo "web: Node version must match .nvmrc (${web_node_version})" >&2; exit 1; }
web_npm="$(dirname "$web_node")/npm"
[[ -f "$web_npm" ]] || { echo 'web: npm is missing beside fixed Node' >&2; exit 1; }
export PATH="$(dirname "$web_node"):$PATH"

cd "$web_repo/apps/web"

built_revision() {
  "$web_node" -e "try{const i=JSON.parse(require('fs').readFileSync('dist/build-info.json','utf8'));process.stdout.write(String(i.revision??''))}catch{}"
}
dist_matches() {
  [[ -f dist/index.html && "$(built_revision)" == "$web_revision" ]]
}

if [[ "$web_mode" == --build ]] && ! dist_matches; then
  web_lock_hash="$(shasum -a 256 package-lock.json | awk '{print $1}')"
  if [[ "$(cat node_modules/.linggan-lock-sha256 2>/dev/null || true)" != "$web_lock_hash" ]]; then
    "$web_node" "$web_npm" ci --ignore-scripts --prefer-offline --no-audit --no-fund
    printf '%s\n' "$web_lock_hash" > node_modules/.linggan-lock-sha256
  fi
  LINGGAN_WEB_BUILD_REVISION="$web_revision" "$web_node" "$web_npm" run build
fi

if ! dist_matches; then
  echo "web: apps/web/dist does not match revision ${web_revision} (built: '$(built_revision)'); run scripts/runtime/prepare-web.sh --build ${web_revision}" >&2
  exit 1
fi
printf '%s\n' "web: React build for revision ${web_revision} ready"
