#!/usr/bin/env bash
set -euo pipefail

# Creates a disposable local PostgreSQL instance for the Comment Research
# User Voices V0 HTTP proof only. It never reads an existing database URL,
# joins a compose network, or leaves a container behind.
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="postgres:16-alpine"
CONTAINER="linggan-comment-voices-api-proof-$(date +%s)-$RANDOM"
DATABASE="linggan_comment_voices_api_proof_$RANDOM-$RANDOM"
USER_NAME="proof_user"
PASSWORD="proof_$RANDOM-$RANDOM"

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

HOST_PORT="$(docker port "$CONTAINER" 5432/tcp | awk -F: 'NR == 1 { print $NF }')"
export LINGGAN_COMMENT_VOICES_API_TEST_DATABASE_URL="postgresql://$USER_NAME:$PASSWORD@127.0.0.1:$HOST_PORT/$DATABASE"

cd "$PROJECT_ROOT"
cargo test -p linggan-api --test comment_voices_read_v0 -- --ignored --nocapture
