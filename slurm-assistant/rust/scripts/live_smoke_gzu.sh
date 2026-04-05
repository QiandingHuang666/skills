#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="$(mktemp -d /tmp/slurm-assistant-live-gzu.XXXXXX)"
LOCAL_UPLOAD="/tmp/codex-live-upload.txt"
LOCAL_DOWNLOAD="/tmp/codex-live-download.txt"
REMOTE_UPLOAD="~/codex-live-upload.txt"
REMOTE_SCRIPT="~/codex_live_submit_smoke.sh"
SERVER_PID=""
SMOKE_JOB_ID=""

HOST="${SLURM_ASSISTANT_GZU_HOST:-210.40.56.85}"
PORT="${SLURM_ASSISTANT_GZU_PORT:-21563}"
USER_NAME="${SLURM_ASSISTANT_GZU_USER:-qiandingh}"
CONNECTION_ID="conn_gzu_cluster"

cleanup() {
  set +e
  if [[ -n "${SMOKE_JOB_ID}" ]]; then
    SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-client -- \
      cancel "${SMOKE_JOB_ID}" --connection "${CONNECTION_ID}" --json >/dev/null 2>&1
  fi
  SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-client -- \
    exec --connection "${CONNECTION_ID}" --cmd "rm -f ${REMOTE_UPLOAD} ${REMOTE_SCRIPT}" --json >/dev/null 2>&1
  rm -f "${LOCAL_UPLOAD}" "${LOCAL_DOWNLOAD}"
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
  python3 -c 'import json,sys; data=json.load(sys.stdin); value=data'"${path_expr}"'; print(value if not isinstance(value, bool) else str(value).lower())'
}

cd "${ROOT_DIR}"

echo "[1/8] starting local server"
SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-server -- serve &
SERVER_PID=$!
sleep 2

echo "[2/8] adding GZU cluster connection"
run_client connection add \
  --label gzu-cluster \
  --host "${HOST}" \
  --port "${PORT}" \
  --user "${USER_NAME}" \
  --kind cluster \
  --json >/dev/null

echo "[3/8] checking jobs and gpu status"
run_client jobs --connection "${CONNECTION_ID}" --json >/dev/null
run_client status --connection "${CONNECTION_ID}" --gpu --partition gpu-a10 --json >/dev/null
run_client find-gpu a10 --connection "${CONNECTION_ID}" --json >/dev/null

echo "[4/8] validating missing log contract"
missing_log_json="$(run_client log 999999999 --connection "${CONNECTION_ID}" --json)"
missing_found="$(printf '%s' "${missing_log_json}" | extract_json_field "['data']['found']")"
if [[ "${missing_found}" != "false" ]]; then
  echo "expected missing log to report found=false" >&2
  exit 1
fi

echo "[5/8] upload and download roundtrip"
printf 'codex live smoke\n' > "${LOCAL_UPLOAD}"
run_client upload "${LOCAL_UPLOAD}" "${REMOTE_UPLOAD}" --connection "${CONNECTION_ID}" --json >/dev/null
rm -f "${LOCAL_DOWNLOAD}"
run_client download "${REMOTE_UPLOAD}" "${LOCAL_DOWNLOAD}" --connection "${CONNECTION_ID}" --json >/dev/null
cmp "${LOCAL_UPLOAD}" "${LOCAL_DOWNLOAD}"

echo "[6/8] submit smoke job"
run_client exec --connection "${CONNECTION_ID}" --cmd "cat > ${REMOTE_SCRIPT} <<'EOF'
#!/bin/bash
#SBATCH --job-name=codex-live-smoke
#SBATCH --partition=cpu48c
#SBATCH --time=00:02:00
#SBATCH --cpus-per-task=1
echo codex live submit smoke
sleep 30
EOF" --json >/dev/null

submit_json="$(run_client submit "${REMOTE_SCRIPT}" --connection "${CONNECTION_ID}" --json)"
SMOKE_JOB_ID="$(printf '%s' "${submit_json}" | extract_json_field "['data']['job_id']")"
if [[ -z "${SMOKE_JOB_ID}" ]]; then
  echo "failed to capture smoke job id" >&2
  exit 1
fi

echo "[7/8] release smoke job"
run_client release "${SMOKE_JOB_ID}" --connection "${CONNECTION_ID}" --json >/dev/null
sleep 3
jobs_json="$(run_client jobs --connection "${CONNECTION_ID}" --job-id "${SMOKE_JOB_ID}" --json)"
jobs_count="$(printf '%s' "${jobs_json}" | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["data"]["jobs"]))')"
if [[ "${jobs_count}" != "0" ]]; then
  echo "smoke job still visible after release" >&2
  exit 1
fi

echo "[8/8] done"
echo "live smoke passed for ${USER_NAME}@${HOST}:${PORT}"
