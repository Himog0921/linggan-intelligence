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
repair_project="linggan-runtime-repair-proof-${proof_suffix}"
repair_compose="$proof_directory/repair-compose.yaml"
repair_old_password="$(openssl rand -hex 24)"
export RUNTIME_PROOF_OLD_PASSWORD="$repair_old_password"
server_pid=""
real_docker="$(command -v docker)"
docker_argv_guard_directory="$proof_directory/docker-argv-guard"

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
  if [[ -f "$repair_compose" ]]; then
    COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
      RUNTIME_PROOF_OLD_PASSWORD="$repair_old_password" \
      docker compose down --volumes --remove-orphans >/dev/null || exit_code=1
  fi
  rm -rf "$proof_directory"
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

compose_client_authentication() {
  local password="$1"
  printf '%s\n' "$password" | docker compose run --rm --no-deps -T postgres sh -ceu '
    IFS= read -r database_password
    case "$database_password" in
      ""|*[!0-9a-f]*)
        echo "proof password must be a hex local secret" >&2
        exit 1
        ;;
    esac
    PGPASSWORD="$database_password" exec psql -X -v ON_ERROR_STOP=1 -h postgres \
      -U linggan_dev_admin -d "$1" -c "SELECT 1;"
  ' sh "$POSTGRES_DB"
}

require_no_password_in_docker_argv_contract() {
  if rg -n -- '-e[[:space:]]+"?(PGPASSWORD|LINGGAN_REPAIR_PASSWORD)=' \
    "$project_root/scripts/local-runtime.sh" "$project_root/scripts/test-local-runtime.sh"; then
    echo "runtime proof found a password-bearing Docker CLI argument in source" >&2
    exit 1
  fi
}

install_docker_argv_guard() {
  mkdir -p "$docker_argv_guard_directory"
  cat > "$docker_argv_guard_directory/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

for argument in "$@"; do
  if [[ "$argument" == *"${LINGGAN_RUNTIME_FORBIDDEN_DOCKER_ARG_SECRET:?}"* ]]; then
    echo "runtime proof found a password in Docker CLI arguments" >&2
    exit 97
  fi
done

exec "${LINGGAN_RUNTIME_REAL_DOCKER:?}" "$@"
EOF
  chmod 700 "$docker_argv_guard_directory/docker"
}

require_password_repair_uses_current_env() {
  cat > "$repair_compose" <<EOF
services:
  postgres:
    image: postgres:16.14-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8
    environment:
      POSTGRES_DB: ${POSTGRES_DB}
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: \${RUNTIME_PROOF_OLD_PASSWORD}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U \$\${POSTGRES_USER} -d \$\${POSTGRES_DB}"]
      interval: 1s
      timeout: 3s
      retries: 30
    volumes:
      - repair-proof-data:/var/lib/postgresql/data
volumes:
  repair-proof-data:
EOF

  COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
    RUNTIME_PROOF_OLD_PASSWORD="$repair_old_password" \
    docker compose up -d --wait postgres >/dev/null

  if COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
    compose_client_authentication "$POSTGRES_PASSWORD" >/dev/null 2>&1; then
    echo "runtime proof expected the isolated container old password to reject the current .env password before repair" >&2
    exit 1
  fi

  install_docker_argv_guard
  COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
    RUNTIME_PROOF_OLD_PASSWORD="$repair_old_password" \
    PATH="$docker_argv_guard_directory:$PATH" \
    LINGGAN_RUNTIME_FORBIDDEN_DOCKER_ARG_SECRET="$POSTGRES_PASSWORD" \
    LINGGAN_RUNTIME_REAL_DOCKER="$real_docker" \
    "$project_root/scripts/local-runtime.sh" repair-password >/dev/null

  if COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
    compose_client_authentication "$repair_old_password" >/dev/null 2>&1; then
    echo "runtime proof expected the isolated container old password to stop working after repair" >&2
    exit 1
  fi

  COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
    compose_client_authentication "$POSTGRES_PASSWORD" >/dev/null

  COMPOSE_FILE="$repair_compose" COMPOSE_PROJECT_NAME="$repair_project" \
    RUNTIME_PROOF_OLD_PASSWORD="$repair_old_password" \
    docker compose down --volumes --remove-orphans >/dev/null
}

require_no_password_in_docker_argv_contract
require_password_repair_uses_current_env

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
  "coverage":{"unit":"visible_search_card","visibleCards":1,"discoveredCards":1,"emittedCards":1,"failedCards":0,"notAttemptedCards":0,"stoppedReason":"risk_control"},
  "cards":[{"content":{"platformContentId":"runtime-proof-card","title":"Synthetic local runtime discovery card","creatorDisplayName":"Runtime proof","publishedAtSourceText":"${observed_at}","coverCandidate":{"observedExternalUri":"https://example.invalid/not-for-display"}},"occurrence":{"query":"ADHD","sort":"comprehensive","observedAt":"${observed_at}","resultPosition":1}}]
}
EOF

start_server() {
  LINGGAN_LOCAL_PORT="$proof_port" \
    "$project_root/scripts/local-runtime.sh" --database "$proof_database" serve >"$proof_directory/server.log" 2>&1 &
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

require_runtime_target_mismatch_is_rejected() {
  local mismatched_url exit_code
  mismatched_url="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT:-55432}/${proof_database}_other"
  set +e
  LINGGAN_LOCAL_PORT="$proof_port" \
  LINGGAN_LOCAL_DATABASE_URL="$mismatched_url" \
    "$project_root/scripts/local-runtime.sh" --database "$proof_database" serve \
    >"$proof_directory/mismatched-target.log" 2>&1
  exit_code=$?
  set -e
  if [[ $exit_code -eq 0 ]]; then
    echo "runtime proof accepted a conflicting API database target" >&2
    exit 1
  fi
  require_contains "$proof_directory/mismatched-target.log" \
    'LINGGAN_LOCAL_DATABASE_URL conflicts with the exact database verified by migration' \
    "runtime target mismatch was not explicitly rejected"
}

require_runtime_target_mismatch_is_rejected

start_server
require_contains "$proof_directory/health.json" '"state":"READY"' "health did not report a ready database"
require_contains "$proof_directory/health.json" '"schema":"LOCAL_003_SCHEMA_READY"' "health did not report all local migrations"
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

# This removes only new connections to the exact disposable proof database and terminates
# only its sessions. The already-running API must stop reporting READY after its pool loses
# that database; no shared development database or process is stopped for this check.
docker compose exec -T postgres psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d postgres <<SQL >/dev/null
ALTER DATABASE ${proof_database} WITH ALLOW_CONNECTIONS false;
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE datname = '${proof_database}' AND pid <> pg_backend_pid();
SQL
curl --fail --silent "http://localhost:${proof_port}/health" >"$proof_directory/health-after-loss.json"
require_contains "$proof_directory/health-after-loss.json" \
  '"state":"CONFIGURED_UNAVAILABLE"' \
  "health continued to report READY after the proof database became unavailable"
require_contains "$proof_directory/health-after-loss.json" \
  '"schema":"LOCAL_001_DATABASE_UNAVAILABLE"' \
  "health did not identify post-start database loss"
read_after_loss_status="$(curl --silent --output "$proof_directory/read-after-loss.json" --write-out '%{http_code}' \
  "http://localhost:${proof_port}/api/local/evidence-library?q=Synthetic")"
if [[ "$read_after_loss_status" != "503" ]]; then
  echo "runtime proof expected a 503 local read after proof database loss, got ${read_after_loss_status}" >&2
  exit 1
fi
stop_server

echo "local runtime proof passed: target mismatch rejection, migration, accepted synthetic discovery, restart persistence, post-start database-loss readiness, and local-only readback"
