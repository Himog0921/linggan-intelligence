#!/usr/bin/env bash
set -euo pipefail

page_spec="docs/design/pages/evidence-library-page.md"
reference="docs/design/pages/evidence-library-multi-material-reference.html"
manifest="docs/design/changes/evidence-page-002-multi-material-ui-change-manifest.md"
acceptance="docs/design/acceptance/evidence-page-002-multi-material-reference-acceptance.md"

for required in "$page_spec" "$reference" "$manifest" "$acceptance"; do
  test -f "$required"
done

required_terms=(
  "一个稳定来源作品在当前 Linggan 中可核验的材料集合"
  "NOT_REQUESTED"
  "RISK_CONTROL"
  "BYTES_CLEANED"
  "PROCESSING"
  "WITHDRAWN_OR_RESTRICTED"
  "READ_PROJECTION_UNAVAILABLE"
  "SYNTHETIC_REFERENCE"
  "评论与回复"
  "作者"
  "媒体与派生"
  "来源与限制"
  "来源代次"
  "Blob"
  "处理版本"
  "Checkpoint"
  "Live Photo"
  "候选 URI"
  "Bundle 关系"
  "候选顺序不推断组件"
)

for term in "${required_terms[@]}"; do
  rg -q --fixed-strings "$term" "$page_spec" "$reference"
done

rg -q --fixed-strings "../../../apps/api/src/local_web/lids_tokens.css" "$reference"
rg -q --fixed-strings "SYNTHETIC REFERENCE · NOT LIVE · NO SIDE EFFECTS" "$reference"
rg -q --fixed-strings "prefers-reduced-motion: reduce" "$reference"
rg -q --fixed-strings "aria-selected=\"true\"" "$reference"
rg -q --fixed-strings "没有本地副本就不显示 CDN" "$reference"
rg -q --fixed-strings 'role="listbox"' "$reference"
rg -q --fixed-strings 'role="option"' "$reference"
rg -q --fixed-strings 'id="back-to-results"' "$reference"
rg -q --fixed-strings 'inspector.hidden = true' "$reference"
rg -q --fixed-strings '.results[hidden], .work-row[hidden], .inspector[hidden] { display: none !important; }' "$reference"
rg -q --fixed-strings "document.getElementById('inspector-overview').replaceChildren()" "$reference"
rg -q --fixed-strings "shell.dataset.inspector = 'false'" "$reference"
rg -q --fixed-strings 'const applyFilter = (filter)' "$reference"
rg -q --fixed-strings "row.hidden = !matches" "$reference"
rg -q --fixed-strings "filters.forEach((button) => button.addEventListener('click', () => applyFilter(button.dataset.filter)))" "$reference"
rg -q --fixed-strings "document.getElementById('context-count').textContent = String(count)" "$reference"
rg -q --fixed-strings "setScenario('results');" "$reference"
rg -q --fixed-strings 'id="inspector-title"></h2>' "$reference"
rg -q --fixed-strings 'id="inspector-overview" role="tabpanel" aria-labelledby="tab-overview"></section>' "$reference"
rg -q --fixed-strings 'id="inspector-discussion" role="tabpanel" aria-labelledby="tab-discussion"></section>' "$reference"
rg -q --fixed-strings 'id="inspector-media" role="tabpanel" aria-labelledby="tab-media"></section>' "$reference"
rg -q --fixed-strings 'id="inspector-provenance" role="tabpanel" aria-labelledby="tab-provenance"></section>' "$reference"

for filter in all partial risk cleaned processing; do
  rg -q --fixed-strings "data-filter=\"$filter\"" "$reference"
done

if rg -q --fixed-strings '<button class="work-row"' "$reference"; then
  echo "work rows must use legal listbox/option semantics, not aria-selected buttons" >&2
  exit 1
fi

if ! rg -q --fixed-strings '.filter:not([disabled]):not([aria-pressed="true"]):hover' "$reference" ||
   ! rg -q --fixed-strings '.inspector-tabs button:not([aria-selected="true"]):hover' "$reference"; then
  echo "hover states must exclude selected filters and tabs" >&2
  exit 1
fi

if rg -q -- "--lgi-[a-z0-9-]+\s*:" "$reference"; then
  echo "reference must consume, not redeclare, LIDS tokens" >&2
  exit 1
fi

if rg -q -- "https?://|fetch\(|XMLHttpRequest|WebSocket" "$reference"; then
  echo "reference must not call remote resources or services" >&2
  exit 1
fi

if rg -q --fixed-strings "完整度" "$reference"; then
  echo "reference must not collapse lanes into an overall completeness score" >&2
  exit 1
fi

if rg -q --fixed-strings "评论 0" "$reference"; then
  echo "unknown comments must not become zero" >&2
  exit 1
fi

echo "Evidence Library multi-material reference verification passed."
