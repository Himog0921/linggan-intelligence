#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_comment_proof_${proof_suffix}"
proof_container="linggan-comment-proof-${proof_suffix}"
proof_volume="linggan-comment-proof-${proof_suffix}-data"
proof_user="comment_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="postgres:16.14-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8"

[[ "$proof_database" =~ ^linggan_comment_proof_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-comment-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-comment-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

if ! docker info >/dev/null 2>&1; then
  echo "Comment research PostgreSQL proof was not started: the Docker daemon is unavailable." >&2
  exit 1
fi

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  [[ -z "${preview_api_pid:-}" ]] || kill "$preview_api_pid" 2>/dev/null || true
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "Comment research proof cleanup failed" >&2; exit 1; }
  echo "Comment research PostgreSQL proof cleanup verified; isolated container and volume were removed"
  exit "$task_exit"
}
trap cleanup EXIT

docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" \
  --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" \
  --env POSTGRES_DB="$proof_database" \
  --env POSTGRES_USER="$proof_user" \
  --env POSTGRES_PASSWORD="$proof_password" \
  --publish 127.0.0.1::5432 "$postgres_image" >/dev/null
proof_container_created=1
for _ in {1..30}; do
  docker exec "$proof_container" pg_isready -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break
  sleep 1
done
docker exec "$proof_container" pg_isready -U "$proof_user" -d "$proof_database" >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || { echo "isolated proof PostgreSQL did not expose a safe port" >&2; exit 1; }
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"

cargo test -p linggan-intelligence --test comment_research_preview --locked prepare_comment_research_browser_preview -- --ignored
cargo build -p linggan-api -p linggan-worker --bin linggan-api --bin linggan-comment-worker --locked
preview_port="$(python3 - <<'PYPORT'
import socket
with socket.socket() as s:
 s.bind(('127.0.0.1',0))
 print(s.getsockname()[1])
PYPORT
)"
preview_log="$(mktemp /tmp/comment-research-preview.XXXXXX)"
preview_url="${LOCAL_001_PROOF_DATABASE_URL}?options=-csearch_path%3Dcomment_research_preview"
LINGGAN_LOCAL_DATABASE_URL="$preview_url" "$project_root/target/debug/linggan-comment-worker" --queue-only --once
LINGGAN_LOCAL_DATABASE_URL="$preview_url" LINGGAN_LOCAL_PORT="$preview_port" "$project_root/target/debug/linggan-api" >"$preview_log" 2>&1 &
preview_api_pid=$!
printf 'Synthetic preview: http://127.0.0.1:%s/corpus/comments\n' "$preview_port"
printf 'API PID: %s; API log: %s\n' "$preview_api_pid" "$preview_log"
printf 'Isolated container: %s; volume: %s\n' "$proof_container" "$proof_volume"
printf '%s\n' 'SYNTHETIC / NOT EVIDENCE. This process owns the isolated preview. Stop this script to remove its API, container and volume.'
wait "$preview_api_pid"
