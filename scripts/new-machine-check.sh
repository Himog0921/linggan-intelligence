#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

missing=0
for command_name in git rustup rustc cargo node docker openssl; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "missing: $command_name"
    missing=1
  else
    "$command_name" --version 2>/dev/null | head -n 1
  fi
done

if command -v docker >/dev/null 2>&1; then
  if ! docker compose version; then
    echo "missing: Docker Compose v2" >&2
    missing=1
  fi

  if ! docker info >/dev/null 2>&1; then
    echo "Docker Desktop is installed but not running" >&2
    missing=1
  fi
fi

if command -v rustup >/dev/null 2>&1; then
  for component_name in rustfmt clippy; do
    if ! rustup component list --installed | grep -Eq "^${component_name}-"; then
      echo "missing Rust component: $component_name" >&2
      missing=1
    fi
  done
fi

if command -v node >/dev/null 2>&1 && [[ -f .nvmrc ]]; then
  expected_node="v$(tr -d '[:space:]' < .nvmrc)"
  actual_node="$(node --version)"
  if [[ "$actual_node" != "$expected_node" ]]; then
    echo "Node version mismatch: expected $expected_node, got $actual_node" >&2
    missing=1
  fi
fi

if [[ "$missing" -ne 0 ]]; then
  echo "new machine is not ready" >&2
  exit 1
fi

echo "new machine prerequisites are available; PostgreSQL clients are provided by Docker"
