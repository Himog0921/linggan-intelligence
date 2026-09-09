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
# worktree 只能建在 <仓库>/.worktrees/ 下。
#
# 受保护交付要求专属 worktree，但从不说建在哪，于是每个 Agent 工具按自己的默认值散建。
# 2026-09-03 清理时是 31 个 worktree、六个位置、33GB，其中多个分支只存在于本地——
# 散落的真正代价不是磁盘，是没人答得上来「哪些工作还没推」。
#
# 唯一例外是常驻服务的运行目录，它的生命周期与部署绑定，不是交付用的 worktree。
# The sanctioned path is anchored on the MAIN worktree, not on whichever worktree is running
# this check. `git rev-parse --show-toplevel` returns the current worktree, so running the check
# from inside .worktrees/<slug>/ reported the main repository as a stray worktree -- failing the
# check in exactly the situation AGENTS.md requires (protected delivery works in a worktree and
# must run this script before committing). The first entry of `git worktree list` is the main
# worktree.
main_worktree="$(git worktree list --porcelain | awk '/^worktree /{print substr($0,10); exit}')"
runtime_worktree="$HOME/Library/Application Support/Linggan Intelligence/runtime-main"
while IFS= read -r worktree_path; do
  [[ "$worktree_path" == "$main_worktree" ]] && continue
  [[ "$worktree_path" == "$runtime_worktree" ]] && continue
  [[ "$worktree_path" == "$main_worktree/.worktrees/"* ]] && continue
  report_error "worktree outside the sanctioned path (see AGENTS.md): $worktree_path"
done < <(git worktree list --porcelain | awk '/^worktree /{print substr($0,10)}')

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

# 人可读时间只有一种写法：YYYY-MM-DD HH:MM，由数据库函数 linggan_human_moment 统一给出。
#
# 2026-09-09 之前全项目并存五种写法（缺年份的、带毫秒的、带时区偏移的、ISO 的、直接
# ::text 倒出来的），同一个页面上并排出现三种，读的人得先判断这是哪一种。
#
# 机器合同（/health、Producer 契约、凭据回执）仍用 ISO：那里精度与偏移都有意义，因此
# 只禁止在读取层新写人可读格式，不禁止 ISO。
human_moment_offenders="$(grep -rn "to_char(" crates/evidence/src apps/api/src --include='*.rs' \
  | grep -v 'YYYY-MM-DD\\"T\\"' | grep -v 'HH24:MI:SSOF' | grep -v "'YYYY-MM-DD'" || true)"
if [[ -n "$human_moment_offenders" ]]; then
  report_error "人可读时间必须走 linggan_human_moment()，不要另写 to_char 格式：
$human_moment_offenders"
fi

if [[ "$errors" -ne 0 ]]; then
  echo "project governance check failed with $errors error(s)" >&2
  exit 1
fi

echo "project governance check passed"
