#!/usr/bin/env bash
# Creates the only supported local runtime for CI-AUTO-004 semantic compute.
# It never reads application configuration, database credentials, or a network source.
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
readonly APP_DIR="${REPO_ROOT}/apps/comment-semantics"
readonly LOCK_FILE="${APP_DIR}/requirements.lock"
readonly VENV_DIR="${APP_DIR}/.venv"
readonly EXPECTED_PYTHON="$(tr -d '[:space:]' < "${APP_DIR}/.python-version")"
readonly EXPECTED_ARCH="arm64"

usage() {
  cat <<'EOF'
Usage: scripts/runtime/prepare-comment-semantics.sh --install|--check

--install creates the ignored apps/comment-semantics/.venv from the hash-locked
wheel-only requirements. --check verifies that exact controlled environment.
EOF
}

fail() {
  printf 'comment-semantics runtime: %s\n' "$*" >&2
  exit 1
}

require_host() {
  [[ "$(uname -s)" == "Darwin" ]] || fail "only macOS is supported by this lock"
  [[ "$(uname -m)" == "${EXPECTED_ARCH}" ]] || fail "expected ${EXPECTED_ARCH}, got $(uname -m)"

  local macos_major
  macos_major="$(sw_vers -productVersion | cut -d. -f1)"
  [[ "${macos_major}" =~ ^[0-9]+$ ]] || fail "cannot determine macOS version"
  (( macos_major >= 14 )) || fail "faiss-cpu arm64 wheel requires macOS 14 or newer"
}

python_path() {
  if [[ -n "${LINGGAN_SEMANTICS_PYTHON:-}" ]]; then
    printf '%s\n' "${LINGGAN_SEMANTICS_PYTHON}"
  else
    command -v python3
  fi
}

require_python() {
  local python_bin="$1"
  [[ -x "${python_bin}" ]] || fail "Python executable is not usable: ${python_bin}"
  local actual_version
  actual_version="$("${python_bin}" -c 'import platform; print(platform.python_version())')"
  [[ "${actual_version}" == "${EXPECTED_PYTHON}" ]] || {
    fail "expected CPython ${EXPECTED_PYTHON}, got ${actual_version}; set LINGGAN_SEMANTICS_PYTHON to the controlled interpreter"
  }
}

lock_sha256() {
  shasum -a 256 "${LOCK_FILE}" | awk '{print $1}'
}

verify_venv() {
  [[ -x "${VENV_DIR}/bin/python" ]] || fail "missing controlled venv; run --install"
  local actual_version expected_lock expected_platform
  actual_version="$("${VENV_DIR}/bin/python" -c 'import platform; print(platform.python_version())')"
  expected_lock="$(lock_sha256)"
  expected_platform="$(uname -s)-$(uname -m)-$(sw_vers -productVersion | cut -d. -f1)"
  [[ "${actual_version}" == "${EXPECTED_PYTHON}" ]] || fail "venv Python drifted to ${actual_version}"
  [[ -f "${VENV_DIR}/.linggan-lock-sha256" ]] || fail "venv lock marker is absent"
  [[ "$(<"${VENV_DIR}/.linggan-lock-sha256")" == "${expected_lock}" ]] || fail "venv lock marker differs from requirements.lock"
  [[ -f "${VENV_DIR}/.linggan-platform" ]] || fail "venv platform marker is absent"
  [[ "$(<"${VENV_DIR}/.linggan-platform")" == "${expected_platform}" ]] || fail "venv platform marker differs from this host"
  "${VENV_DIR}/bin/python" "${APP_DIR}/comment_semantics.py" --self-check >/dev/null
}

[[ $# -eq 1 ]] || { usage; exit 2; }
[[ -f "${LOCK_FILE}" ]] || fail "requirements.lock is absent"
require_host

case "$1" in
  --check)
    verify_venv
    printf 'comment-semantics runtime is controlled and ready\n'
    ;;
  --install)
    command -v uv >/dev/null || fail "uv is required to install the locked wheel environment"
    python_bin="$(python_path)"
    require_python "${python_bin}"
    # .venv is ignored and exclusively owned by this script.
    uv venv --clear --python "${python_bin}" "${VENV_DIR}"
    uv pip sync --python "${VENV_DIR}/bin/python" --require-hashes --only-binary=:all: "${LOCK_FILE}"
    printf '%s\n' "$(lock_sha256)" > "${VENV_DIR}/.linggan-lock-sha256"
    printf '%s\n' "$(uname -s)-$(uname -m)-$(sw_vers -productVersion | cut -d. -f1)" > "${VENV_DIR}/.linggan-platform"
    verify_venv
    printf 'comment-semantics runtime installed\n'
    ;;
  *)
    usage
    exit 2
    ;;
esac
