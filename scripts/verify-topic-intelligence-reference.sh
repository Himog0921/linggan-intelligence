#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

errors=0

report_error() {
  echo "topic intelligence reference error: $1" >&2
  errors=$((errors + 1))
}

required_files=(
  "docs/plans/active/design-002-topic-intelligence-reference-page.md"
  "docs/pages/topic-intelligence-surface.md"
  "docs/design/foundation/topic-intelligence-visual-language.md"
  "docs/design/patterns/evidence-candidate-and-boundary-patterns.md"
  "docs/design/components/component-promotion.md"
  "docs/design/pages/topic-intelligence-reference-page.md"
  "docs/design/pages/topic-intelligence-reference-acceptance.md"
  "docs/design/pages/topic-intelligence-reference.html"
)

for path in "${required_files[@]}"; do
  if [[ ! -f "$path" ]]; then
    report_error "missing DESIGN-002 artifact: $path"
  fi
done

markdown_files=(
  "docs/plans/active/design-002-topic-intelligence-reference-page.md"
  "docs/pages/topic-intelligence-surface.md"
  "docs/design/foundation/topic-intelligence-visual-language.md"
  "docs/design/patterns/evidence-candidate-and-boundary-patterns.md"
  "docs/design/components/component-promotion.md"
  "docs/design/pages/topic-intelligence-reference-page.md"
  "docs/design/pages/topic-intelligence-reference-acceptance.md"
)

for path in "${markdown_files[@]}"; do
  [[ -f "$path" ]] || continue
  header="$(sed -n '1,12p' "$path")"
  for field in '状态:' '最后核对:' '适用范围:' '事实来源:' '冲突时以谁为准:'; do
    if ! grep -Fq "> $field" <<<"$header"; then
      report_error "missing status header field '$field' in $path"
    fi
  done
done

for path in \
  "design/foundation/topic-intelligence-visual-language.md" \
  "design/patterns/evidence-candidate-and-boundary-patterns.md" \
  "design/components/component-promotion.md" \
  "design/pages/topic-intelligence-reference-page.md" \
  "design/pages/topic-intelligence-reference-acceptance.md" \
  "pages/topic-intelligence-surface.md" \
  "plans/active/design-002-topic-intelligence-reference-page.md"; do
  if ! grep -Fq "]($path)" docs/README.md; then
    report_error "DESIGN-002 document is not indexed in docs/README.md: $path"
  fi
done

html="docs/design/pages/topic-intelligence-reference.html"
if [[ -f "$html" ]]; then
  for token in \
    'data-reference-mode="synthetic"' \
    'SYNTHETIC / NOT LIVE' \
    'SOURCE_INCOMPLETE' \
    'LOCAL_INTENT_ONLY' \
    'NO TREND CLAIM' \
    'prefers-reduced-motion' \
    'data-window' \
    'data-intent'; do
    if ! grep -Fq "$token" "$html"; then
      report_error "static reference page is missing required boundary or interaction marker: $token"
    fi
  done

  if grep -Fq 'fetch(' "$html" || grep -Fq 'XMLHttpRequest' "$html"; then
    report_error "static reference page must not request live data"
  fi
fi

if ! grep -Fq '第二个独立页面' docs/design/components/component-promotion.md; then
  report_error "component promotion rule must require a second independent page"
fi

if ! grep -Fq '页面没有增长箭头、百分比变化、趋势线' docs/pages/topic-intelligence-surface.md; then
  report_error "product page does not explicitly prohibit unsupported trend expression"
fi

if [[ "$errors" -ne 0 ]]; then
  echo "Topic Intelligence reference check failed with $errors error(s)" >&2
  exit 1
fi

echo "Topic Intelligence reference check passed"
