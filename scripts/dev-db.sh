#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

usage() {
  echo "usage: $0 {setup|up|stop|down|status|logs|versions|psql|pg_restore} [arguments...]" >&2
}

ensure_env() {
  "$project_root/scripts/setup-local-env.sh"
  set -a
  # shellcheck disable=SC1091
  source "$project_root/.env"
  set +a

  if [[ "${POSTGRES_PASSWORD:-}" == replace-* || "${#POSTGRES_PASSWORD}" -lt 24 ]]; then
    echo "POSTGRES_PASSWORD must be a non-placeholder local secret with at least 24 characters" >&2
    exit 1
  fi

  if [[ "${POSTGRES_USER:-}" != "linggan_dev_admin" || -z "${DATABASE_ADMIN_URL:-}" ]]; then
    echo "local environment must use linggan_dev_admin and DATABASE_ADMIN_URL" >&2
    exit 1
  fi
}

command_name="${1:-}"
if [[ -z "$command_name" ]]; then
  usage
  exit 1
fi
shift

case "$command_name" in
  setup)
    "$project_root/scripts/setup-local-env.sh"
    ;;
  up)
    ensure_env
    docker compose up -d --wait postgres
    ;;
  stop)
    ensure_env
    docker compose stop postgres
    ;;
  down)
    ensure_env
    docker compose down
    ;;
  status)
    ensure_env
    docker compose ps postgres
    ;;
  logs)
    ensure_env
    docker compose logs --tail=100 postgres
    ;;
  versions)
    ensure_env
    docker compose exec -T postgres psql --version
    docker compose exec -T postgres pg_restore --version
    ;;
  psql)
    ensure_env
    docker compose exec postgres psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"
    ;;
  pg_restore)
    ensure_env
    docker compose exec -T postgres pg_restore "$@"
    ;;
  *)
    usage
    exit 1
    ;;
esac
