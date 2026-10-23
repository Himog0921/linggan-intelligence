#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

base_ref="${1:-origin/main}"
base_commit="$(git rev-parse --verify --end-of-options "${base_ref}^{commit}")"
comparison="$(git merge-base "$base_commit" HEAD)"

# No database, providers, package installs, or builds. Fail before expensive CI.
git diff --check "$comparison" --
bash scripts/check-project-governance.sh "$comparison"
bash scripts/verify-ui-design-handbook.sh
# Source fingerprints include the agent entrypoints. This check needs no npm
# install and must fail here, not after the full frontend test suite.
node scripts/lids.mjs check
node --test scripts/check-document-governance.test.mjs scripts/ci-select.test.mjs

# Include staged, unstaged and untracked files for local use. CI uses the same
# entry point against the PR base. NUL delimiters preserve spaces/newlines.
while IFS= read -r -d '' path; do
  [[ -f "$path" ]] || continue
  case "$path" in
    *.sh) bash -n "$path" ;;
    *.mjs|*.cjs) node --check "$path" ;;
  esac
done < <(git diff --name-only --no-renames -z "$comparison" --; git ls-files --others --exclude-standard -z)

echo "Preflight passed; required module proofs and independent review remain separate."
