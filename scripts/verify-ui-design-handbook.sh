#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

errors=0

report_error() {
  echo "ui design handbook error: $1" >&2
  errors=$((errors + 1))
}

required_files=(
  "docs/design/README.md"
  "docs/design/design-governance.md"
  "docs/design/reference-register.md"
  "docs/design/lids/README.md"
  "docs/design/lids/system.md"
  "docs/design/lids/tokens.md"
  "docs/design/lids/primitives.md"
  "docs/design/lids/patterns.md"
  "docs/design/lids/agent-execution-guide.md"
  "docs/design/lids/prototype-audit.md"
  "docs/design/lids/migration-log.md"
  "docs/design/templates/page-spec-form.md"
  "docs/design/templates/component-spec-form.md"
  "docs/design/templates/ui-change-manifest-form.md"
  "docs/design/templates/visual-acceptance-form.md"
  "docs/agents/ui-execution-contract.md"
)

for path in "${required_files[@]}"; do
  if [[ ! -f "$path" ]]; then
    report_error "missing required handbook file: $path"
  fi
done

for path in "${required_files[@]}"; do
  [[ -f "$path" ]] || continue
  header="$(sed -n '1,12p' "$path")"
  for field in '状态:' '最后核对:' '适用范围:' '事实来源:' '冲突时以谁为准:'; do
    if ! grep -Fq "> $field" <<<"$header"; then
      report_error "missing status header field '$field' in $path"
    fi
  done
done

indexed_paths=(
  "agents/ui-execution-contract.md"
  "design/README.md"
  "design/design-governance.md"
  "design/reference-register.md"
  "design/lids/README.md"
  "design/lids/system.md"
  "design/lids/tokens.md"
  "design/lids/primitives.md"
  "design/lids/patterns.md"
  "design/lids/agent-execution-guide.md"
  "design/lids/prototype-audit.md"
  "design/lids/migration-log.md"
  "design/templates/page-spec-form.md"
  "design/templates/component-spec-form.md"
  "design/templates/ui-change-manifest-form.md"
  "design/templates/visual-acceptance-form.md"
)

for path in "${indexed_paths[@]}"; do
  if ! grep -Fq "]($path)" docs/README.md; then
    report_error "handbook document is not indexed in docs/README.md: $path"
  fi
done

if ! grep -Fq "docs/agents/ui-execution-contract.md" AGENTS.md; then
  report_error "AGENTS.md does not require the UI execution contract"
fi

if ! grep -Fq "docs/design/lids/README.md" AGENTS.md; then
  report_error "AGENTS.md does not require the LIDS entrypoint"
fi

if ! grep -Fq '| `docs/design/` |' docs/governance/file-placement-standard.md; then
  report_error "file-placement standard does not define the docs/design/ shelf"
fi

if ! grep -Fq "DECISION_REQUIRED" docs/design/design-governance.md; then
  report_error "design governance is missing the required decision stop state"
fi

if ! grep -Fq "闭集执行" docs/agents/ui-execution-contract.md; then
  report_error "UI execution contract is missing the closed-world execution rule"
fi

if ! grep -Fq 'Token → Primitive → Component → Pattern → Page' docs/design/lids/system.md; then
  report_error "LIDS system is missing the mandatory five-layer architecture"
fi

if ! grep -Fq 'PARTIAL + VALID' docs/design/lids/system.md; then
  report_error "LIDS system is missing the PARTIAL + VALID rule"
fi

if ! grep -Fq 'PROPOSED' docs/design/lids/README.md; then
  report_error "LIDS entrypoint must state its current maturity"
fi

while IFS= read -r directory; do
  report_error "empty design directory is forbidden until it has an approved document: $directory"
done < <(find docs/design -type d -empty -print | sort)

if [[ "$errors" -ne 0 ]]; then
  echo "UI design handbook check failed with $errors error(s)" >&2
  exit 1
fi

echo "UI design handbook check passed"
