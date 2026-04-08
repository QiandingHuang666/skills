#!/usr/bin/env bash

set -euo pipefail

REPO_OWNER="QiandingHuang666"
REPO_NAME="skills"
INSTALL_DIR="${SLURM_ASSISTANT_INSTALL_DIR:-${HOME}/.local/bin}"
BASE_URL="${SLURM_ASSISTANT_BASE_URL:-https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

download() {
  local url="$1"
  local output="$2"

  if need_cmd curl; then
    curl -fsSL "$url" -o "$output"
    return
  fi

  if need_cmd wget; then
    wget -qO "$output" "$url"
    return
  fi

  echo "error: curl or wget is required" >&2
  exit 1
}

detect_platform_suffix() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64)
          printf '%s\n' "linux-amd64"
          ;;
        *)
          echo "error: unsupported Linux architecture: ${arch}" >&2
          echo "supported Linux architectures: x86_64, amd64" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64)
          printf '%s\n' "macos-amd64"
          ;;
        arm64|aarch64)
          printf '%s\n' "macos-arm64"
          ;;
        *)
          echo "error: unsupported macOS architecture: ${arch}" >&2
          echo "supported macOS architectures: x86_64, amd64, arm64, aarch64" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "error: unsupported operating system: ${os}" >&2
      echo "supported operating systems: Linux, macOS" >&2
      exit 1
      ;;
  esac
}

install_binary() {
  local asset_name="$1"
  local target_name="$2"
  local tmp_file
  tmp_file="$(mktemp)"

  echo "downloading ${asset_name}"
  download "${BASE_URL}/${asset_name}" "$tmp_file"

  install -d "$INSTALL_DIR"
  install -m 0755 "$tmp_file" "${INSTALL_DIR}/${target_name}"
  rm -f "$tmp_file"
}

main() {
  local suffix
  suffix="$(detect_platform_suffix)"

  install_binary "slurm-client-${suffix}" "slurm-client"
  install_binary "slurm-server-${suffix}" "slurm-server"

  echo
  echo "installed:"
  echo "  ${INSTALL_DIR}/slurm-client"
  echo "  ${INSTALL_DIR}/slurm-server"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
      ;;
    *)
      echo
      echo "warning: ${INSTALL_DIR} is not in PATH"
      echo "add this to your shell profile:"
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac
}

main "$@"
