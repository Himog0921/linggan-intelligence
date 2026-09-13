#!/usr/bin/env bash
set -euo pipefail

# Runs the confirmed Comment Research input-freezing HTTP proof against one
# randomly named disposable PostgreSQL container. It does not read a runtime
# DSN, use port 3000, or start any model/provider/vector/queue/worker process.
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="postgres:16-alpine"
CONTAINER="linggan-comment-research-run-preparation-v1-$(date +%s)-$RANDOM"
DATABASE="linggan_comment_research_run_preparation_v1_${RANDOM}_${RANDOM}"
USER_NAME="proof_user"
PASSWORD="proof_${RANDOM}_${RANDOM}"

cleanup() {
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run --detach --rm \
  --name "$CONTAINER" \
  --env "POSTGRES_DB=$DATABASE" \
  --env "POSTGRES_USER=$USER_NAME" \
  --env "POSTGRES_PASSWORD=$PASSWORD" \
  --publish "127.0.0.1::5432" \
  "$IMAGE" >/dev/null

for _ in $(seq 1 60); do
  if docker exec "$CONTAINER" pg_isready --username "$USER_NAME" --dbname "$DATABASE" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if ! docker exec "$CONTAINER" pg_isready --username "$USER_NAME" --dbname "$DATABASE" >/dev/null 2>&1; then
  echo "isolated PostgreSQL proof database did not become ready" >&2
  exit 1
fi

HOST_PORT="$(docker port "$CONTAINER" 5432/tcp | sed -E 's/.*:([0-9]+)$/\1/' | head -n 1)"
export LINGGAN_COMMENT_RESEARCH_RUN_PREPARATION_TEST_DATABASE_URL="postgresql://$USER_NAME:$PASSWORD@127.0.0.1:$HOST_PORT/$DATABASE"

cd "$PROJECT_ROOT"
cargo test -p linggan-api --test comment_research_run_preparation_v1 -- --ignored --nocapture
