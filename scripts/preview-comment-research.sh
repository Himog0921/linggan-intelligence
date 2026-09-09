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
  [[ -z "${preview_worker_pid:-}" ]] || kill "$preview_worker_pid" 2>/dev/null || true
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

proof_sql() {
  docker exec --env "PGPASSWORD=${proof_password}" "$proof_container" \
    psql --no-psqlrc --tuples-only --no-align --quiet --set ON_ERROR_STOP=1 \
      --username "$proof_user" --dbname "$proof_database" --command "$1"
}

assert_preview_state() {
  local phase="$1"
  local row
  row="$(proof_sql "SET search_path TO comment_research_preview; WITH state AS (SELECT
      (SELECT count(*) FROM linggan_comment_daily_batch) AS batches,
      (SELECT count(*) FROM linggan_comment_replay_run) AS replays,
      (SELECT count(*) FROM linggan_comment_daily_schedule WHERE singleton AND enabled) AS active_schedule,
      (SELECT count(*) FROM linggan_model_invocation WHERE operation IN ('analyze','embed')) AS externalizable_calls,
      (SELECT count(*) FROM linggan_comment_field_repair) AS field_repairs,
      (SELECT count(*) FROM linggan_comment_model_work) AS granted_legacy_work,
      (SELECT count(*) FROM collection_observation_target) AS observation_targets,
      (SELECT count(*) FROM linggan_model_invocation WHERE state='running') AS running_calls,
      (SELECT count(*) FROM linggan_model_config) AS configs,
      (SELECT count(*) FROM linggan_model_config cfg JOIN linggan_model_entry model USING(model_ref) JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref WHERE version.local_endpoint AND version.base_url LIKE 'http://127.0.0.1:%') AS local_configs,
      (SELECT count(*) FROM linggan_model_invocation WHERE operation='probe' AND state='succeeded') AS synthetic_probes,
      (SELECT count(*) FROM linggan_comment_research_eligibility_current) AS eligibility_rows
    ) SELECT concat_ws('|',batches,replays,active_schedule,externalizable_calls,field_repairs,granted_legacy_work,observation_targets,running_calls,configs,local_configs,synthetic_probes,eligibility_rows) FROM state;")"
  IFS='|' read -r batches replays active_schedule externalizable_calls field_repairs granted_legacy_work observation_targets running_calls configs local_configs synthetic_probes eligibility_rows <<<"$row"
  [[ "$batches" == 0 && "$replays" == 0 && "$active_schedule" == 0 && "$externalizable_calls" == 0 && "$field_repairs" == 0 && "$granted_legacy_work" == 0 && "$observation_targets" == 0 && "$running_calls" == 0 && "$configs" == 1 && "$local_configs" == 1 && "$synthetic_probes" == 1 ]] || {
    echo "Synthetic preview safety assertion failed before/after ${phase}: batch=${batches:-?} replay=${replays:-?} activeSchedule=${active_schedule:-?} analyzeOrEmbed=${externalizable_calls:-?} fieldRepairs=${field_repairs:-?} legacyWork=${granted_legacy_work:-?} observationTargets=${observation_targets:-?} runningCalls=${running_calls:-?} configs=${configs:-?} localConfigs=${local_configs:-?} probes=${synthetic_probes:-?}" >&2
    exit 1
  }
  if [[ "$phase" == after_local_reconcile && "${eligibility_rows:-0}" -lt 1 ]]; then
    echo "Synthetic preview local reconciliation did not create an eligibility projection" >&2
    exit 1
  fi
  printf 'Synthetic preview safety assertion (%s): no target, grant, batch/replay/field repair, or active schedule; analyze/embed invocations=0\n' "$phase"
}

prove_sigterm_drain() {
  local drain_dir drain_ack drain_log worker_status
  drain_dir="$(mktemp -d /tmp/comment-research-worker-drain.XXXXXX)"
  drain_ack="$drain_dir/worker-drain-ack"
  drain_log="$drain_dir/worker.log"

  # This is the actual long-running worker binary. The preview fixture has no observation
  # targets, active schedule, granted legacy work, batch, replay, or field repair, so it can
  # only exercise the process shutdown boundary and must not reserve an externalizable call.
  LINGGAN_LOCAL_DATABASE_URL="$preview_url" \
    LINGGAN_WORKER_DRAIN_ACK_PATH="$drain_ack" \
    "$project_root/target/debug/linggan-worker" >"$drain_log" 2>&1 &
  preview_worker_pid=$!

  for _ in {1..50}; do
    grep -Fq 'linggan worker: patrol tick every' "$drain_log" && break
    if ! kill -0 "$preview_worker_pid" 2>/dev/null; then
      echo "SIGTERM drain proof worker exited before becoming ready; log: $drain_log" >&2
      cat "$drain_log" >&2
      exit 1
    fi
    sleep 0.1
  done
  grep -Fq 'linggan worker: patrol tick every' "$drain_log" || {
    echo "SIGTERM drain proof worker did not become ready; log: $drain_log" >&2
    cat "$drain_log" >&2
    exit 1
  }

  kill -TERM "$preview_worker_pid"
  if wait "$preview_worker_pid"; then
    worker_status=0
  else
    worker_status=$?
  fi
  [[ "$worker_status" -eq 0 ]] || {
    echo "SIGTERM drain proof worker exited with status ${worker_status}; log: $drain_log" >&2
    cat "$drain_log" >&2
    exit 1
  }
  if kill -0 "$preview_worker_pid" 2>/dev/null; then
    echo "SIGTERM drain proof worker process is still live: ${preview_worker_pid}" >&2
    exit 1
  fi
  [[ -f "$drain_ack" ]] || {
    echo "SIGTERM drain proof did not write an acknowledgement; log: $drain_log" >&2
    cat "$drain_log" >&2
    exit 1
  }
  grep -Fxq "pid=${preview_worker_pid}" "$drain_ack"
  grep -Fxq 'state=drained' "$drain_ack"
  grep -Fq 'linggan worker: shutdown requested; stopping new model reservations' "$drain_log"
  grep -Fq 'linggan worker: model drain confirmed' "$drain_log"
  preview_worker_pid=""
  assert_preview_state after_sigterm_drain
  printf 'SIGTERM drain proof: pid=%s, ACK state=drained, worker exited; ACK: %s; log: %s\n' \
    "$(sed -n 's/^pid=//p' "$drain_ack")" "$drain_ack" "$drain_log"
}

cargo test -p linggan-intelligence --test comment_research_preview --locked prepare_comment_research_browser_preview -- --ignored
cargo build -p linggan-api -p linggan-worker --bin linggan-api --bin linggan-comment-worker --bin linggan-worker --locked
preview_port="$(python3 - <<'PYPORT'
import socket
with socket.socket() as s:
 s.bind(('127.0.0.1',0))
 print(s.getsockname()[1])
PYPORT
)"
preview_log="$(mktemp /tmp/comment-research-preview.XXXXXX)"
preview_url="${LOCAL_001_PROOF_DATABASE_URL}?options=-csearch_path%3Dcomment_research_preview"
assert_preview_state before_local_reconcile
LINGGAN_LOCAL_DATABASE_URL="$preview_url" "$project_root/target/debug/linggan-comment-worker" --execute --once
assert_preview_state after_local_reconcile
prove_sigterm_drain
LINGGAN_LOCAL_DATABASE_URL="$preview_url" LINGGAN_LOCAL_PORT="$preview_port" LINGGAN_MODEL_SYNTHETIC_PREVIEW=SYNTHETIC-NOT-EVIDENCE "$project_root/target/debug/linggan-api" >"$preview_log" 2>&1 &
preview_api_pid=$!
printf 'Synthetic preview: http://127.0.0.1:%s/corpus/comments\n' "$preview_port"
printf 'API PID: %s; API log: %s\n' "$preview_api_pid" "$preview_log"
printf 'Isolated container: %s; volume: %s\n' "$proof_container" "$proof_volume"
printf '%s\n' 'SYNTHETIC / NOT EVIDENCE. This process owns the isolated preview. Stop this script to remove its API, container and volume.'
wait "$preview_api_pid"
