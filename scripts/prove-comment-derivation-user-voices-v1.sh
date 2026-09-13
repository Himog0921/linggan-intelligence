#!/usr/bin/env bash
set -euo pipefail

# Runs the Comment Derivation V1 proof against only a disposable PostgreSQL
# container on a random loopback port. It reads no configured runtime database,
# makes no model/vector/network call, and always removes the container.
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="postgres:16-alpine"
CONTAINER="linggan-comment-derivation-v1-proof-$(date +%s)-$RANDOM"
DATABASE="linggan_comment_derivation_v1_proof_${RANDOM}_${RANDOM}"
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
export LINGGAN_COMMENT_DERIVATION_V1_TEST_DATABASE_URL="postgresql://$USER_NAME:$PASSWORD@127.0.0.1:$HOST_PORT/$DATABASE"

cd "$PROJECT_ROOT"
cargo test -p linggan-api --test comment_derivation_user_voices_v1 -- --ignored --nocapture
