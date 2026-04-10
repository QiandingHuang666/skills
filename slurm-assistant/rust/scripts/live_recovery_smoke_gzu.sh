#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="$(mktemp -d /tmp/slurm-assistant-live-recovery.XXXXXX)"
SERVER_PID=""

HOST="${SLURM_ASSISTANT_GZU_HOST:-210.40.56.85}"
PORT="${SLURM_ASSISTANT_GZU_PORT:-21563}"
USER_NAME="${SLURM_ASSISTANT_GZU_USER:-qiandingh}"
CONNECTION_ID="conn_gzu_cluster"

cleanup() {
  set +e
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "${DATA_DIR}"
}

trap cleanup EXIT

run_client() {
  SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-client -- "$@"
}

extract_json_field() {
  local path_expr="$1"
  jq -r "${path_expr}"
}

cd "${ROOT_DIR}"

echo "[1/6] start local server"
SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-server -- serve &
SERVER_PID=$!
sleep 2

echo "[2/6] ensure server and verify capabilities"
ensure_json="$(run_client server ensure --json)"
api_version="$(printf '%s' "${ensure_json}" | extract_json_field '.data.api_version')"
caps="$(printf '%s' "${ensure_json}" | extract_json_field '.data.capabilities | join(\",\")')"
if [[ "${api_version}" -lt 1 ]]; then
  echo "unexpected api_version: ${api_version}" >&2
  exit 1
fi
if [[ "${caps}" != *"sessions"* ]]; then
  echo "server capabilities missing sessions: ${caps}" >&2
  exit 1
fi

echo "[3/6] add GZU connection"
run_client connection add \
  --label gzu-cluster \
  --host "${HOST}" \
  --port "${PORT}" \
  --user "${USER_NAME}" \
  --kind cluster \
  --json >/dev/null

echo "[4/6] verify session summary path"
run_client session summary --json >/dev/null

echo "[5/6] verify connection execution path"
host_json="$(run_client exec --connection "${CONNECTION_ID}" --cmd 'hostname' --json)"
remote_host="$(printf '%s' "${host_json}" | extract_json_field '.data.stdout' | tr -d '\r\n')"
if [[ -z "${remote_host}" ]]; then
  echo "hostname probe returned empty output" >&2
  exit 1
fi

echo "[6/6] done"
echo "recovery smoke passed for ${USER_NAME}@${HOST}:${PORT} (remote host: ${remote_host})"
