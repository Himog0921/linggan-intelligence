#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

./scripts/new-machine-check.sh
./scripts/setup-local-env.sh

set -a
# shellcheck disable=SC1091
source .env
set +a

if [[ "${POSTGRES_PASSWORD:-}" == replace-* || "${#POSTGRES_PASSWORD}" -lt 24 ]]; then
  echo "POSTGRES_PASSWORD must be a non-placeholder local secret with at least 24 characters" >&2
  exit 1
fi

if [[ "${POSTGRES_USER:-}" != "linggan_dev_admin" || -z "${DATABASE_ADMIN_URL:-}" ]]; then
  echo "local environment must use linggan_dev_admin and DATABASE_ADMIN_URL" >&2
  exit 1
fi

docker compose config --quiet
docker compose up -d --wait postgres

server_version_num="$(
  docker compose exec -T postgres \
    psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc 'SHOW server_version_num;'
)"

if [[ ! "$server_version_num" =~ ^16[0-9]{4}$ ]]; then
  echo "expected PostgreSQL 16, got server_version_num=$server_version_num" >&2
  exit 1
fi

psql_version="$(docker compose exec -T postgres psql --version)"
pg_restore_version="$(docker compose exec -T postgres pg_restore --version)"

if [[ "$psql_version" != psql\ \(PostgreSQL\)\ 16.* ]]; then
  echo "expected PostgreSQL 16 psql, got: $psql_version" >&2
  exit 1
fi

if [[ "$pg_restore_version" != pg_restore\ \(PostgreSQL\)\ 16.* ]]; then
  echo "expected PostgreSQL 16 pg_restore, got: $pg_restore_version" >&2
  exit 1
fi

prisma_ledger="$(
  docker compose exec -T postgres \
    psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc \
      "SELECT COALESCE(to_regclass('public._prisma_migrations')::text, '');"
)"

if [[ -n "$prisma_ledger" ]]; then
  echo "legacy Prisma migration ledger found in the clean development database" >&2
  exit 1
fi

proof_database="linggan_intelligence_proof_$(date -u +%Y%m%d%H%M%S)_$$"
if [[ ! "$proof_database" =~ ^linggan_intelligence_proof_[0-9]{14}_[0-9]+$ ]]; then
  echo "unsafe proof database name" >&2
  exit 1
fi

cleanup_proof_database() {
  docker compose exec -T postgres \
    dropdb --if-exists -U "$POSTGRES_USER" "$proof_database"
}

cleanup_proof_database_on_exit() {
  cleanup_proof_database >/dev/null 2>&1 || true
}
trap cleanup_proof_database_on_exit EXIT

docker compose exec -T postgres createdb -U "$POSTGRES_USER" "$proof_database"
docker compose exec -T postgres \
  psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$proof_database" >/dev/null <<'SQL'
CREATE TABLE environment_proof (
  proof_id integer PRIMARY KEY,
  proof_value text NOT NULL
);
INSERT INTO environment_proof (proof_id, proof_value)
VALUES (1, 'postgresql-16-real-write');
SQL

proof_value="$(
  docker compose exec -T postgres \
    psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$proof_database" -Atc \
      'SELECT proof_value FROM environment_proof WHERE proof_id = 1;'
)"

if [[ "$proof_value" != "postgresql-16-real-write" ]]; then
  echo "PostgreSQL proof write/read did not round-trip" >&2
  exit 1
fi

if ! cleanup_proof_database; then
  echo "proof database cleanup failed" >&2
  exit 1
fi

remaining_proof_database="$(
  docker compose exec -T postgres \
    psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d postgres -Atc \
      "SELECT datname FROM pg_database WHERE datname = '$proof_database';"
)"

if [[ -n "$remaining_proof_database" ]]; then
  echo "proof database still exists after cleanup" >&2
  exit 1
fi

trap - EXIT

expected_node="v$(tr -d '[:space:]' < .nvmrc)"
if [[ "$(node --version)" != "$expected_node" ]]; then
  echo "Node version does not match .nvmrc" >&2
  exit 1
fi

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

echo "development environment verification passed"
echo "Rust: $(rustc --version)"
echo "PostgreSQL server_version_num: $server_version_num"
echo "$psql_version"
echo "$pg_restore_version"
echo "PostgreSQL proof database was created, written, read, and removed"
