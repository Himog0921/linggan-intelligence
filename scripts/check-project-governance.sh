#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

errors=0

report_error() {
  echo "governance error: $1" >&2
  errors=$((errors + 1))
}

required_files=(
  "AGENTS.md"
  "README.md"
  "docs/README.md"
  "docs/current-state.md"
  "docs/governance/file-placement-standard.md"
  "docs/governance/agent-collaboration.md"
  "docs/governance/generated-artifacts-registry.md"
  "docs/progress/README.md"
)

for path in "${required_files[@]}"; do
  if [[ ! -f "$path" ]]; then
    report_error "missing required governance file: $path"
  fi
done

while IFS= read -r path; do
  name="${path#./}"
  case "$name" in
    README.md|AGENTS.md|CODEX.md|CLAUDE.md) ;;
    *) report_error "unexpected root Markdown file: $name" ;;
  esac
done < <(find . -maxdepth 1 -type f -name '*.md' -print | sort)

allowed_status='权威当前|代码事实优先|活跃计划|已完成计划|一次性报告|历史归档|草案'

while IFS= read -r path; do
  relative="${path#docs/}"

  if [[ "$path" != "docs/README.md" ]] && ! grep -Fq "]($relative)" docs/README.md; then
    report_error "document is not indexed in docs/README.md: $path"
  fi

  header="$(sed -n '1,12p' "$path")"
  for field in '状态:' '最后核对:' '适用范围:' '事实来源:' '冲突时以谁为准:'; do
    if ! grep -Fq "> $field" <<<"$header"; then
      report_error "missing status header field '$field' in $path"
    fi
  done

  if ! grep -Eq "^> 状态: ($allowed_status)(  )?$" <<<"$header"; then
    report_error "invalid document status in $path"
  fi

  case "$path" in
    docs/plans/active/*.md)
      if ! grep -Eq '^> 状态: 活跃计划(  )?$' <<<"$header"; then
        report_error "active plan must use status 活跃计划: $path"
      fi
      ;;
    docs/plans/completed/*.md)
      if ! grep -Eq '^> 状态: 已完成计划(  )?$' <<<"$header"; then
        report_error "completed plan must use status 已完成计划: $path"
      fi
      ;;
  esac

  base_name="$(basename "$path")"
  if [[ "$base_name" =~ (最新版|最终版|final-final|copy|temp|draft[0-9]+) ]]; then
    report_error "unstable lifecycle name: $path"
  fi
done < <(find docs -type f -name '*.md' -print | sort)

while IFS= read -r path; do
  report_error "tracked generated/private artifact is forbidden: $path"
done < <(
  git ls-files |
    grep -Ev '^references/' |
    grep -E '(^|/)(\.DS_Store|target/|dist/|build/|coverage/|tmp/|temp/|artifacts/private/|database/backups/|database/restores/)' || true
)

link_files=(README.md AGENTS.md)
while IFS= read -r path; do link_files+=("$path"); done < <(find docs database -type f -name '*.md' -print | sort)
link_files+=(references/README.md)

for path in "${link_files[@]}"; do
  [[ -f "$path" ]] || continue
  while IFS= read -r raw_link; do
    link="$(sed -E 's/^\]\(([^)]+)\)$/\1/' <<<"$raw_link")"
    link="${link%%#*}"
    case "$link" in
      ''|http://*|https://*|mailto:*|\#*) continue ;;
    esac
    if [[ "$link" == /* ]]; then
      continue
    fi
    candidate="$(dirname "$path")/$link"
    if [[ ! -e "$candidate" ]]; then
      report_error "broken local link in $path: $link"
    fi
  done < <(grep -Eo '\]\([^)]+\)' "$path" || true)
done

if [[ "$#" -gt 0 ]]; then
  base_ref="$1"
elif upstream_ref="$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null)"; then
  base_ref="$upstream_ref"
else
  base_ref="HEAD"
fi
if ! git rev-parse --verify "$base_ref" >/dev/null 2>&1; then
  report_error "unknown comparison ref: $base_ref"
else
  changed_files="$({
    git diff --name-only "$base_ref" --
    git ls-files --others --exclude-standard
  } | sort -u)"

  material_changes="$(grep -Ev '^(docs/progress/|$)' <<<"$changed_files" || true)"
  progress_change="$(grep -E '^docs/progress/[0-9]{4}-[0-9]{2}\.md$' <<<"$changed_files" || true)"

  if [[ -n "$material_changes" && -z "$progress_change" ]]; then
    report_error "material changes require a monthly docs/progress/YYYY-MM.md entry"
  fi
fi

if [[ "$errors" -ne 0 ]]; then
  echo "project governance check failed with $errors error(s)" >&2
  exit 1
fi

echo "project governance check passed"
