#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CASES_FILE="${ROOT_DIR}/evals/cases.json"
RUST_DIR="${ROOT_DIR}/rust"
DATA_DIR=""
SERVER_PID=""
CONNECTION_ID="conn_gzu_cluster"

HOST="${SLURM_ASSISTANT_GZU_HOST:-210.40.56.85}"
PORT="${SLURM_ASSISTANT_GZU_PORT:-21563}"
USER_NAME="${SLURM_ASSISTANT_GZU_USER:-qiandingh}"

usage() {
  cat <<'EOF'
Usage:
  bash slurm-assistant/evals/run_eval.sh live
  bash slurm-assistant/evals/run_eval.sh trace --trace <trace.json>
EOF
}

cleanup() {
  set +e
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${DATA_DIR}" && -d "${DATA_DIR}" ]]; then
    rm -rf "${DATA_DIR}"
  fi
}

run_client() {
  SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-client -- "$@"
}

json_escape() {
  jq -Rn --arg value "$1" '$value'
}

load_trace_commands() {
  local trace_file="$1"
  jq -r '
    [
      .tool_calls[]
      | select(.tool == "exec_command" or .tool == "functions.exec_command")
      | (.cmd // .parameters.cmd // empty)
    ]' "${trace_file}"
}

evaluate_trace() {
  local trace_file=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --trace)
        trace_file="$2"
        shift 2
        ;;
      *)
        echo "unknown argument: $1" >&2
        usage
        exit 2
        ;;
    esac
  done

  if [[ -z "${trace_file}" ]]; then
    echo "--trace is required" >&2
    exit 2
  fi

  local commands_json
  commands_json="$(load_trace_commands "${trace_file}")"

  local total passed
  total="$(jq 'length' "${CASES_FILE}")"
  passed=0
  local results="[]"

  while IFS= read -r case_json; do
    local name ok messages must_count one_of_count not_count
    name="$(jq -r '.name' <<<"${case_json}")"
    ok=true
    messages="[]"

    must_count="$(jq '.trace_rules.must_contain_commands // [] | length' <<<"${case_json}")"
    for ((i = 0; i < must_count; i++)); do
      local pattern
      pattern="$(jq -r ".trace_rules.must_contain_commands[${i}]" <<<"${case_json}")"
      if ! jq -r '.[]' <<<"${commands_json}" | grep -Eq "${pattern}"; then
        ok=false
        messages="$(jq --arg msg "missing required command pattern: ${pattern}" '. + [$msg]' <<<"${messages}")"
      fi
    done

    one_of_count="$(jq '.trace_rules.must_contain_one_of_commands // [] | length' <<<"${case_json}")"
    for ((i = 0; i < one_of_count; i++)); do
      local group found=false
      group="$(jq -c ".trace_rules.must_contain_one_of_commands[${i}]" <<<"${case_json}")"
      while IFS= read -r pattern; do
        if jq -r '.[]' <<<"${commands_json}" | grep -Eq "${pattern}"; then
          found=true
          break
        fi
      done < <(jq -r '.[]' <<<"${group}")
      if [[ "${found}" != true ]]; then
        ok=false
        messages="$(jq --arg msg "missing one-of command group: $(jq -r 'join(" | ")' <<<"${group}")" '. + [$msg]' <<<"${messages}")"
      fi
    done

    not_count="$(jq '.trace_rules.must_not_contain_commands // [] | length' <<<"${case_json}")"
    for ((i = 0; i < not_count; i++)); do
      local pattern
      pattern="$(jq -r ".trace_rules.must_not_contain_commands[${i}]" <<<"${case_json}")"
      if jq -r '.[]' <<<"${commands_json}" | grep -Eq "${pattern}"; then
        ok=false
        messages="$(jq --arg msg "forbidden command pattern matched: ${pattern}" '. + [$msg]' <<<"${messages}")"
      fi
    done

    if [[ "${ok}" == true ]]; then
      passed=$((passed + 1))
    fi

    results="$(
      jq \
        --arg name "${name}" \
        --argjson ok "${ok}" \
        --argjson messages "${messages}" \
        '. + [{"name": $name, "ok": $ok, "messages": $messages}]' <<<"${results}"
    )"
  done < <(jq -c '.[]' "${CASES_FILE}")

  jq -n \
    --arg trace "${trace_file}" \
    --argjson passed "${passed}" \
    --argjson total "${total}" \
    --argjson results "${results}" \
    '{trace: $trace, passed: $passed, total: $total, results: $results}'

  [[ "${passed}" -eq "${total}" ]]
}

apply_live_assertion() {
  local output="$1"
  local assertion_json="$2"
  local kind expr pattern
  kind="$(jq -r '.kind' <<<"${assertion_json}")"
  expr="$(jq -r '.expr // empty' <<<"${assertion_json}")"
  pattern="$(jq -r '.pattern // empty' <<<"${assertion_json}")"

  case "${kind}" in
    jq_true)
      jq -e "${expr}" <<<"${output}" >/dev/null
      ;;
    jq_nonempty)
      jq -e "${expr} | tostring | length > 0" <<<"${output}" >/dev/null
      ;;
    jq_match)
      jq -er "${expr}" <<<"${output}" | grep -Eq "${pattern}"
      ;;
    *)
      echo "unknown assertion kind: ${kind}" >&2
      return 1
      ;;
  esac
}

prepare_live_runtime() {
  DATA_DIR="$(mktemp -d /tmp/slurm-assistant-eval.XXXXXX)"
  trap cleanup EXIT

  (
    cd "${RUST_DIR}"
    SLURM_ASSISTANT_DATA_DIR="${DATA_DIR}" cargo run --quiet --bin slurm-server -- serve
  ) &
  SERVER_PID=$!
  sleep 2

  (
    cd "${RUST_DIR}"
    run_client connection add \
      --label gzu-cluster \
      --host "${HOST}" \
      --port "${PORT}" \
      --user "${USER_NAME}" \
      --kind cluster \
      --json >/dev/null
  )
}

evaluate_live() {
  prepare_live_runtime

  local total passed
  total="$(jq 'length' "${CASES_FILE}")"
  passed=0
  local results="[]"

  while IFS= read -r case_json; do
    local name argv_json argv output ok messages assertion_count
    name="$(jq -r '.name' <<<"${case_json}")"
    argv_json="$(jq '.live_probe.argv' <<<"${case_json}")"
    argv_json="$(jq --arg conn "${CONNECTION_ID}" 'map(if . == "__CONNECTION_ID__" then $conn else . end)' <<<"${argv_json}")"
    ok=true
    messages="[]"

    argv=()
    while IFS= read -r arg; do
      argv+=("${arg}")
    done < <(jq -r '.[]' <<<"${argv_json}")
    output="$(
      cd "${RUST_DIR}"
      run_client "${argv[@]}"
    )"

    assertion_count="$(jq '.live_probe.assertions | length' <<<"${case_json}")"
    for ((i = 0; i < assertion_count; i++)); do
      local assertion
      assertion="$(jq -c ".live_probe.assertions[${i}]" <<<"${case_json}")"
      if ! apply_live_assertion "${output}" "${assertion}"; then
        ok=false
        messages="$(jq --arg msg "assertion failed: $(jq -c '.' <<<"${assertion}")" '. + [$msg]' <<<"${messages}")"
      fi
    done

    if [[ "${ok}" == true ]]; then
      passed=$((passed + 1))
    fi

    results="$(
      jq \
        --arg name "${name}" \
        --argjson ok "${ok}" \
        --argjson messages "${messages}" \
        '. + [{"name": $name, "ok": $ok, "messages": $messages}]' <<<"${results}"
    )"
  done < <(jq -c '.[]' "${CASES_FILE}")

  jq -n \
    --arg host "${HOST}" \
    --arg port "${PORT}" \
    --arg user "${USER_NAME}" \
    --argjson passed "${passed}" \
    --argjson total "${total}" \
    --argjson results "${results}" \
    '{host: $host, port: $port, user: $user, passed: $passed, total: $total, results: $results}'

  [[ "${passed}" -eq "${total}" ]]
}

main() {
  if [[ $# -lt 1 ]]; then
    usage
    exit 2
  fi

  case "$1" in
    live)
      shift
      evaluate_live "$@"
      ;;
    trace)
      shift
      evaluate_trace "$@"
      ;;
    *)
      usage
      exit 2
      ;;
  esac
}

main "$@"
