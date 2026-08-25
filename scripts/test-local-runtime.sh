#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

"$project_root/scripts/setup-local-env.sh" >/dev/null
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

"$project_root/scripts/dev-db.sh" up >/dev/null
proof_suffix="$(openssl rand -hex 8)"
proof_database="linggan_intelligence_runtime_proof_${proof_suffix}"
proof_directory="$(mktemp -d "${TMPDIR:-/tmp}/linggan-local-runtime-proof.XXXXXX")"
proof_port="$((31000 + $(printf '%d' "0x${proof_suffix:0:4}") % 1000))"
server_pid=""

if lsof -nP -iTCP:"$proof_port" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "runtime proof port ${proof_port} is already in use; retry the proof without interrupting that process" >&2
  exit 1
fi

cleanup() {
  local exit_code=$?
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  docker compose exec -T postgres psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d postgres \
    -c "DROP DATABASE IF EXISTS ${proof_database};" >/dev/null || exit_code=1
  if docker compose exec -T postgres psql -X -v ON_ERROR_STOP=1 -At -U "$POSTGRES_USER" -d postgres \
    -c "SELECT 1 FROM pg_database WHERE datname = '${proof_database}';" | grep -qx '1'; then
    echo "runtime proof database cleanup failed" >&2
    exit_code=1
  fi
  rm -rf "$proof_directory"
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

docker compose exec -T postgres psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d postgres \
  -c "CREATE DATABASE ${proof_database};" >/dev/null
"$project_root/scripts/local-runtime.sh" --database "$proof_database" migrate

observed_at="$(docker compose exec -T postgres psql -X -v ON_ERROR_STOP=1 -At -U "$POSTGRES_USER" -d "$proof_database" \
  -c "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"');")"
payload_path="$proof_directory/discovery.json"
cat > "$payload_path" <<EOF
{
  "contractVersion":"xhs.discovery.visible-card.v1",
  "acquisitionSpec":{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}},
  "observedAt":"${observed_at}",
  "coverage":{"unit":"visible_search_card","visibleCards":1,"stoppedReason":"risk_control"},
  "cards":[{"content":{"platformContentId":"runtime-proof-card","title":"Synthetic local runtime discovery card","creatorDisplayName":"Runtime proof","publishedAtSourceText":"${observed_at}","coverCandidate":{"observedExternalUri":"https://example.invalid/not-for-display"}},"occurrence":{"query":"ADHD","sort":"comprehensive","observedAt":"${observed_at}","resultPosition":1}}]
}
EOF

start_server() {
  LINGGAN_LOCAL_PORT="$proof_port" \
  LINGGAN_LOCAL_DATABASE_URL="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT:-55432}/${proof_database}" \
    cargo run -p linggan-api >"$proof_directory/server.log" 2>&1 &
  server_pid=$!
  for _ in $(seq 1 240); do
    if curl --fail --silent "http://localhost:${proof_port}/health" >"$proof_directory/health.json"; then
      return
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then
      cat "$proof_directory/server.log" >&2
      echo "local Linggan service exited before health became available" >&2
      exit 1
    fi
    sleep 0.25
  done
  cat "$proof_directory/server.log" >&2
  echo "local Linggan service did not become ready" >&2
  exit 1
}

stop_server() {
  kill "$server_pid"
  wait "$server_pid" || true
  server_pid=""
}

require_contains() {
  local path="$1"
  local expected="$2"
  local label="$3"
  if ! grep -q "$expected" "$path"; then
    echo "runtime proof failed: ${label}" >&2
    if [[ "$path" == *health.json ]]; then
      cat "$path" >&2
    fi
    exit 1
  fi
}

start_server
require_contains "$proof_directory/health.json" '"state":"READY"' "health did not report a ready database"
require_contains "$proof_directory/health.json" '"schema":"LOCAL_001_SCHEMA_READY"' "health did not report both migrations"
curl --fail --silent --show-error -H 'content-type: application/json' --data-binary @"$payload_path" \
  "http://localhost:${proof_port}/api/local/discovery-packages" >"$proof_directory/ingress.json"
require_contains "$proof_directory/ingress.json" '"admission":"accepted"' "synthetic discovery was not accepted"
curl --fail --silent "http://localhost:${proof_port}/api/local/evidence-library?q=Synthetic" >"$proof_directory/read-before-restart.json"
require_contains "$proof_directory/read-before-restart.json" 'runtime-proof-card' "readback did not contain the accepted card"
stop_server

start_server
require_contains "$proof_directory/health.json" '"state":"READY"' "restarted health did not report ready"
curl --fail --silent "http://localhost:${proof_port}/api/local/evidence-library?q=Synthetic" >"$proof_directory/read-after-restart.json"
require_contains "$proof_directory/read-after-restart.json" 'runtime-proof-card' "restart lost the accepted card"
if grep -q 'example.invalid' "$proof_directory/read-after-restart.json"; then
  echo "runtime proof exposed a remote cover candidate" >&2
  exit 1
fi
stop_server

echo "local runtime proof passed: migration, accepted synthetic discovery, restart persistence, health readiness, and local-only readback"
