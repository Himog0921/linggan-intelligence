#!/usr/bin/env bash
set -euo pipefail

missing=0
for command_name in git rustc cargo psql pg_restore; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "missing: $command_name"
    missing=1
  else
    "$command_name" --version | head -n 1
  fi
done

if [[ "$missing" -ne 0 ]]; then
  echo "new machine is not ready" >&2
  exit 1
fi

echo "new machine prerequisites are available"
