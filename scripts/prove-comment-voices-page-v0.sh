#!/usr/bin/env bash
set -euo pipefail

# Proves the User Voices V0 browser shell through a real Axum process. The
# page has no source range until a user enters a workspace ID, so this proof
# only needs an empty disposable PostgreSQL connection. Fixture-seeded storage
# and no-write HTTP proof remain in prove-comment-voices-read-api-v0.sh.
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="postgres:16-alpine"
CONTAINER="linggan-comment-voices-page-proof-$(date +%s)-$RANDOM"
DATABASE="linggan_comment_voices_page_proof_$RANDOM-$RANDOM"
USER_NAME="proof_user"
PASSWORD="proof_$RANDOM-$RANDOM"
API_PORT="$(python3 - <<'PY'
import socket

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
API_LOG="$(mktemp -t linggan-comment-voices-page-proof)"
PAGE_RESPONSE="$(mktemp -t linggan-comment-voices-page-html)"
CSS_RESPONSE="$(mktemp -t linggan-comment-voices-page-css)"
API_PID=""

cleanup() {
  if [[ -n "$API_PID" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" >/dev/null 2>&1 || true
  fi
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  rm -f "$API_LOG" "$PAGE_RESPONSE" "$CSS_RESPONSE"
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
export LINGGAN_DATABASE_URL="postgresql://$USER_NAME:$PASSWORD@127.0.0.1:$HOST_PORT/$DATABASE"
export LINGGAN_API_BIND="127.0.0.1:$API_PORT"

cd "$PROJECT_ROOT"
cargo run --quiet -p linggan-api >"$API_LOG" 2>&1 &
API_PID="$!"

for _ in $(seq 1 60); do
  if curl --fail --silent --show-error "http://127.0.0.1:$API_PORT/comment-research/voices" >"$PAGE_RESPONSE" 2>/dev/null; then
    break
  fi
  sleep 1
done

if [[ ! -s "$PAGE_RESPONSE" ]]; then
  echo "User Voices V0 page did not become reachable" >&2
  cat "$API_LOG" >&2
  exit 1
fi

curl --fail --silent --show-error \
  "http://127.0.0.1:$API_PORT/comment-research/voices/styles.css" >"$CSS_RESPONSE"

grep -Fq "初始状态不读取任何数据" "$PAGE_RESPONSE"
grep -Fq "尚未具备来源事实" "$PAGE_RESPONSE"
grep -Fq "作品上下文未取得" "$PAGE_RESPONSE"
grep -Fq -- "--canvas: oklch(0.965 0.008 85)" "$CSS_RESPONSE"

# A malformed API range is rejected before the storage read; this proves the
# browser route's neighboring API validation works with no migration or write.
STATUS="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  "http://127.0.0.1:$API_PORT/api/v0/comment-research/voices")"
if [[ "$STATUS" != "400" ]]; then
  echo "missing workspace_id must return 400, got $STATUS" >&2
  exit 1
fi

echo "User Voices V0 page and stylesheet are reachable through a real Axum process."
