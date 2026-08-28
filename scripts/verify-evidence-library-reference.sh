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
)

for term in "${required_terms[@]}"; do
  rg -q --fixed-strings "$term" "$page_spec" "$reference"
done

rg -q --fixed-strings "../../../apps/api/src/local_web/lids_tokens.css" "$reference"
rg -q --fixed-strings "SYNTHETIC REFERENCE · NOT LIVE · NO SIDE EFFECTS" "$reference"
rg -q --fixed-strings "prefers-reduced-motion: reduce" "$reference"
rg -q --fixed-strings "aria-selected=\"true\"" "$reference"
rg -q --fixed-strings "没有本地副本就不显示 CDN" "$reference"

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
