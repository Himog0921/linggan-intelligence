#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

if [[ -f .env ]]; then
  echo "local .env already exists; it was not overwritten"
  exit 0
fi

if ! command -v openssl >/dev/null 2>&1; then
  echo "openssl is required to generate a local database password" >&2
  exit 1
fi

postgres_password="$(openssl rand -hex 24)"
umask 077
{
  printf '%s\n' 'APP_ENV=development'
  printf '%s\n' 'POSTGRES_DB=linggan_intelligence_dev'
  printf '%s\n' 'POSTGRES_USER=linggan_dev_admin'
  printf 'POSTGRES_PASSWORD=%s\n' "$postgres_password"
  printf '%s\n' 'POSTGRES_PORT=55432'
  printf 'DATABASE_ADMIN_URL=postgresql://linggan_dev_admin:%s@127.0.0.1:55432/linggan_intelligence_dev\n' "$postgres_password"
  printf '%s\n' 'RUST_LOG=info'
} > .env

echo "created local .env with a random password; the file is ignored by Git"
