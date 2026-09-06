#!/usr/bin/env bash
# MODEL-PI-001: fixed local dependency preparation, no database or service mutation.
set -euo pipefail
model_repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
model_mode="${1:---check}"
[[ "$model_mode" == --check || "$model_mode" == --install ]] || { echo 'usage: prepare-pi-adapter.sh [--check|--install]' >&2; exit 1; }
model_node_version="$(cat "$model_repo/.nvmrc")"
model_node="${LINGGAN_PI_NODE:-$HOME/.nvm/versions/node/v${model_node_version}/bin/node}"
[[ -x "$model_node" ]] || { echo "Pi adapter: install Node ${model_node_version} or set LINGGAN_PI_NODE to its absolute executable path" >&2; exit 1; }
[[ "$("$model_node" --version)" == "v${model_node_version}" ]] || { echo "Pi adapter: Node version must match .nvmrc (${model_node_version})" >&2; exit 1; }
cd "$model_repo/apps/pi-adapter"
model_lock_hash="$(shasum -a 256 package-lock.json | awk '{print $1}')"
model_old_hash="$(cat node_modules/.linggan-lock-sha256 2>/dev/null || true)"
if [[ "$model_mode" == --install && "$model_old_hash" != "$model_lock_hash" ]]; then
  model_npm="$(dirname "$model_node")/npm"
  [[ -f "$model_npm" ]] || { echo 'Pi adapter: npm is missing beside fixed Node' >&2; exit 1; }
  PATH="$(dirname "$model_node"):$PATH" "$model_node" "$model_npm" ci --ignore-scripts --no-audit --no-fund
  printf '%s\n' "$model_lock_hash" > node_modules/.linggan-lock-sha256
fi
[[ "$(cat node_modules/.linggan-lock-sha256 2>/dev/null || true)" == "$model_lock_hash" ]] || { echo 'Pi adapter: lock dependencies are not prepared; run scripts/runtime/prepare-pi-adapter.sh --install' >&2; exit 1; }
"$model_node" --input-type=module - <<'JS'
import fs from 'node:fs';
for (const name of ['pi-ai','pi-agent-core']) {
  const pkg=JSON.parse(fs.readFileSync(`node_modules/@earendil-works/${name}/package.json`));
  if(pkg.version!=='0.85.1')throw new Error('Pi adapter dependency version mismatch');
}
await import('./src/adapter.mjs');
JS
printf '%s\n' 'Pi adapter: fixed Node and Pi 0.85.1 lock dependencies ready; no model call performed'
