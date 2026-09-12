#!/usr/bin/env bash
set -euo pipefail

# The local PostgreSQL volume stays intact. This only creates the pinned image that supplies the
# server-side extension before Compose (or an isolated proof) starts a PostgreSQL 16 container.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

case "${1:-}" in
  --ensure)
    if docker image inspect "$image" >/dev/null 2>&1; then
      exit 0
    fi
    docker build --pull=false --file "$repo_root/docker/postgres-pgvector/Dockerfile" --tag "$image" "$repo_root"
    ;;
  --check)
    docker image inspect "$image" >/dev/null
    ;;
  *)
    echo "usage: $0 {--ensure|--check}" >&2
    exit 2
    ;;
esac
