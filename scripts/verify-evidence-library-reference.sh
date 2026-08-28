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
  "地址断言"
  "declared Bundle"
  "候选顺序不推断组件"
  "槽位级来源观察组"
  "candidateRef"
  "primary"
  "sourceField"
  "observedAt"
  "expiresAtState"
  "expiresAt"
  "failureReason"
  "startedAt"
  "endedAt"
  "terminal"
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
rg -q --fixed-strings 'const scenarioData = {' "$reference"
rg -q --fixed-strings 'const renderWorkRows = () =>' "$reference"
rg -q --fixed-strings '<div class="work-list" id="work-list" role="listbox" aria-label="来源作品材料集合"></div>' "$reference"
rg -q --fixed-strings "row.hidden = !matches" "$reference"
rg -q --fixed-strings "filters.forEach((button) => button.addEventListener('click', () => applyFilter(button.dataset.filter)))" "$reference"
rg -q --fixed-strings "document.getElementById('context-count').textContent = String(count)" "$reference"
rg -q --fixed-strings "setScenario('results');" "$reference"
rg -q --fixed-strings 'id="inspector-title"></h2>' "$reference"
rg -q --fixed-strings 'id="inspector-overview" role="tabpanel" aria-labelledby="tab-overview"></section>' "$reference"
rg -q --fixed-strings 'id="inspector-discussion" role="tabpanel" aria-labelledby="tab-discussion"></section>' "$reference"
rg -q --fixed-strings 'id="inspector-media" role="tabpanel" aria-labelledby="tab-media"></section>' "$reference"
rg -q --fixed-strings 'id="inspector-provenance" role="tabpanel" aria-labelledby="tab-provenance"></section>' "$reference"
rg -q --fixed-strings 'item.tabIndex = isSelected ? 0 : -1' "$reference"
rg -q --fixed-strings "if (event.key === 'ArrowRight')" "$reference"
rg -q --fixed-strings "if (event.key === 'ArrowLeft')" "$reference"
rg -q --fixed-strings "if (event.key === 'Home')" "$reference"
rg -q --fixed-strings "if (event.key === 'End')" "$reference"
rg -q --fixed-strings "if (event.key === 'Home') next = visibleRows[0]" "$reference"
rg -q --fixed-strings "if (event.key === 'End') next = visibleRows[visibleRows.length - 1]" "$reference"
rg -q --fixed-strings 'selectedRow.focus({ preventScroll: true })' "$reference"
rg -q --fixed-strings '.work-row:not([aria-selected="true"]):hover' "$reference"
rg -q --fixed-strings '.scenario-control button:not([aria-pressed="true"]):hover' "$reference"
rg -q --fixed-strings 'origin-group/slot-006/g3' "$reference"
rg -q --fixed-strings 'download/still/synthetic-006 → candidate/still/A' "$reference"
rg -q --fixed-strings 'download/motion/synthetic-006 → candidate/motion/M1' "$reference"
rg -q --fixed-strings 'primary false' "$reference"
rg -q --fixed-strings 'expiresAtState UNKNOWN / expiresAt null' "$reference"
rg -q --fixed-strings 'failureReason source_unavailable · terminal FAILED' "$reference"

if rg -q --fixed-strings '.work-row:not([aria-selected="true"]):hover { background: var(--lgi-canvas-low); transform:' "$reference" ||
   rg -q --fixed-strings '.work-row:not([aria-selected="true"]):hover { background: var(--lgi-canvas-low); box-shadow: var(--lgi-shadow-brutal)' "$reference"; then
  echo "ordinary list row hover must not move or use the 4px hard shadow" >&2
  exit 1
fi

if rg -q --fixed-strings 'const itemCopy' "$reference" ||
   rg -q --fixed-strings 'origin-group/still/synthetic-006' "$reference" ||
   rg -q --fixed-strings 'origin-group/motion/synthetic-006' "$reference" ||
   rg -q --fixed-strings '等价地址' "$reference"; then
  echo "reference must use one scenario source and one current slot origin group; candidate assertions are not inferred equivalents" >&2
  exit 1
fi

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
