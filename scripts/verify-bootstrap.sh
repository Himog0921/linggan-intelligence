#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

shasum -a 256 -c references/SOURCE-MANIFEST.sha256

required=(
  "docs/context/START-HERE.md"
  "docs/product/PRD.md"
  "docs/architecture/target-architecture.md"
  "docs/migration/action-plan.md"
  "references/current-v2/plugin/source/manifest.json"
  "references/current-v2/workbench/source/src/lib/evidence/ingress/evidence-ingress.ts"
)

for path in "${required[@]}"; do
  if [[ ! -f "$path" ]]; then
    echo "missing required bootstrap file: $path" >&2
    exit 1
  fi
done

if {
  git ls-files
  git ls-files --others --exclude-standard
} | grep -E '(^|/)(\.env(\.[^/]*)?|[^/]+\.(dump|backup|pem|key))$' | grep -q .; then
  echo "forbidden private artifact is visible to Git" >&2
  exit 1
fi

echo "bootstrap verification passed"
