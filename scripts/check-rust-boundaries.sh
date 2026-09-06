#!/usr/bin/env bash
set -euo pipefail

# SCOPE-001 file and dependency gate. It fails on the limits that can be checked reliably,
# warns on the ones meant as early signals, and prints the review-only gates it does not
# automate rather than pretending they passed.

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

errors=0
warnings=0

report_error() {
  echo "boundary error: $1" >&2
  errors=$((errors + 1))
}

report_warning() {
  echo "boundary warning: $1" >&2
  warnings=$((warnings + 1))
}

check_file_size() {
  local file="$1" warn_at="$2" fail_at="$3" kind="$4"
  local lines
  lines="$(wc -l <"$file" | tr -d ' ')"
  if ((lines > fail_at)); then
    report_error "$kind $file has $lines lines (hard limit $fail_at)"
  elif ((lines > warn_at)); then
    report_warning "$kind $file has $lines lines (warning limit $warn_at)"
  fi
}

while IFS= read -r file; do
  base="$(basename "$file")"
  if [[ "$file" == *"/tests/"* ]]; then
    check_file_size "$file" 600 900 "test file"
  elif [[ "$base" == "lib.rs" || "$base" == "main.rs" ]]; then
    check_file_size "$file" 120 200 "crate entry point"
  else
    check_file_size "$file" 350 500 "production file"
  fi
done < <(find crates apps -name '*.rs' -not -path '*/target/*' | sort)

# No unowned dumping grounds.
while IFS= read -r directory; do
  report_error "$directory is an unowned catch-all directory"
done < <(find crates apps -type d -name node_modules -prune -o -type d \( -name common -o -name utils -o -name helpers \) -not -path '*/target/*' -print)

# Dependency direction: contracts owns boundary contracts and depends on no storage or
# business crate.
for forbidden in linggan-evidence linggan-observation linggan-storage-postgres; do
  if grep -q "$forbidden" crates/contracts/Cargo.toml; then
    report_error "crates/contracts must not depend on $forbidden"
  fi
done

# Composition roots wire modules together; business SQL belongs to the crate that owns the
# invariant it protects.
if [[ -d apps ]]; then
  while IFS= read -r file; do
    if grep -qiE '(INSERT INTO|UPDATE [a-z_]+ SET|DELETE FROM|SELECT .* FROM )' "$file"; then
      report_error "$file contains business SQL; it belongs in the crate that owns the invariant"
    fi
  done < <(find apps -name '*.rs' -not -path '*/target/*')
fi

# Public surface is reported, not enforced: counting items with a regular expression is not
# reliable enough to fail a build on.
while IFS= read -r file; do
  public_items="$(grep -cE '^\s*pub (fn|struct|enum|trait|const|type|mod|use) ' "$file" || true)"
  if ((public_items > 15)); then
    echo "boundary report: $file exposes $public_items public items (review threshold 15, hard threshold 25)"
  fi
done < <(find crates apps -name '*.rs' -not -path '*/target/*' -not -path '*/tests/*' | sort)

cat <<'REVIEW'
boundary report: not automated by this script, and therefore NOT VERIFIED here:
  - function length (60-line warning); the 100-line hard limit runs as clippy::too_many_lines
  - SQLx/Axum/CLI types must not cross a deep module's public interface
  - exceptions require an ADR with an owner, reason, alternative and review date
REVIEW

if ((errors > 0)); then
  echo "rust boundary check failed with $errors error(s) and $warnings warning(s)" >&2
  exit 1
fi

echo "rust boundary check passed with $warnings warning(s)"
