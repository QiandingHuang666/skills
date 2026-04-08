#!/usr/bin/env bash

set -euo pipefail

REPO_OWNER="QiandingHuang666"
REPO_NAME="skills"
INSTALL_DIR="${SLURM_ASSISTANT_INSTALL_DIR:-${HOME}/.local/bin}"
BASE_URL="${SLURM_ASSISTANT_BASE_URL:-https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download}"
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PACKAGE_DIR="${SCRIPT_DIR}/package"

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
        x86_64|amd64) printf '%s\n' "linux-amd64" ;;
        *)
          echo "error: unsupported Linux architecture: ${arch}" >&2
          echo "supported Linux architectures: x86_64, amd64" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64) printf '%s\n' "macos-amd64" ;;
        arm64|aarch64) printf '%s\n' "macos-arm64" ;;
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

get_skill_roots() {
  if [[ -n "${SLURM_ASSISTANT_SKILL_ROOTS:-}" ]]; then
    printf '%s\n' "${SLURM_ASSISTANT_SKILL_ROOTS}" | tr ':' '\n'
    return
  fi

  printf '%s\n' \
    "${HOME}/.codex/skills" \
    "${HOME}/.claude/skills" \
    "${HOME}/.openclaw/skills"
}

install_bundled_binary() {
  local source_name="$1"
  local target_name="$2"

  install -d "$INSTALL_DIR"
  install -m 0755 "${PACKAGE_DIR}/bin/${source_name}" "${INSTALL_DIR}/${target_name}"
}

install_bundled_skill() {
  local skill_source="${PACKAGE_DIR}/skill/slurm-assistant"
  local skill_root target

  while IFS= read -r skill_root; do
    [[ -n "$skill_root" ]] || continue
    target="${skill_root}/slurm-assistant"
    mkdir -p "$skill_root"
    rm -rf "$target"
    cp -R "$skill_source" "$target"
    echo "installed skill: ${target}"
  done < <(get_skill_roots)
}

install_from_bundle() {
  install_bundled_binary "slurm-client" "slurm-client"
  install_bundled_binary "slurm-server" "slurm-server"
  install_bundled_skill

  echo
  echo "installed binaries:"
  echo "  ${INSTALL_DIR}/slurm-client"
  echo "  ${INSTALL_DIR}/slurm-server"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      echo
      echo "warning: ${INSTALL_DIR} is not in PATH"
      echo "add this to your shell profile:"
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac
}

uninstall_all() {
  local skill_root target

  rm -f "${INSTALL_DIR}/slurm-client" "${INSTALL_DIR}/slurm-server"

  while IFS= read -r skill_root; do
    [[ -n "$skill_root" ]] || continue
    target="${skill_root}/slurm-assistant"
    rm -rf "$target"
    echo "removed skill: ${target}"
  done < <(get_skill_roots)

  echo
  echo "removed binaries:"
  echo "  ${INSTALL_DIR}/slurm-client"
  echo "  ${INSTALL_DIR}/slurm-server"
}

run_from_downloaded_bundle() {
  local action="$1"
  local suffix archive_name tmp_dir archive_path bundle_dir

  suffix="$(detect_platform_suffix)"
  archive_name="slurm-assistant-${suffix}.tar.gz"
  tmp_dir="$(mktemp -d)"
  archive_path="${tmp_dir}/${archive_name}"

  echo "downloading ${archive_name}"
  download "${BASE_URL}/${archive_name}" "$archive_path"

  mkdir -p "${tmp_dir}/extract"
  tar -xzf "$archive_path" -C "${tmp_dir}/extract"
  bundle_dir="$(find "${tmp_dir}/extract" -mindepth 1 -maxdepth 1 -type d | head -n 1)"

  if [[ -z "$bundle_dir" || ! -f "${bundle_dir}/install-slurm-assistant.sh" ]]; then
    rm -rf "$tmp_dir"
    echo "error: invalid package layout in ${archive_name}" >&2
    exit 1
  fi

  SLURM_ASSISTANT_INSTALL_DIR="${INSTALL_DIR}" \
  SLURM_ASSISTANT_SKILL_ROOTS="${SLURM_ASSISTANT_SKILL_ROOTS:-}" \
    bash "${bundle_dir}/install-slurm-assistant.sh" "$action"

  rm -rf "$tmp_dir"
}

main() {
  local action="${1:-install}"

  case "$action" in
    install|uninstall) ;;
    -h|--help|help)
      echo "usage: install-slurm-assistant.sh [install|uninstall]"
      return
      ;;
    *)
      echo "error: unsupported action: ${action}" >&2
      echo "usage: install-slurm-assistant.sh [install|uninstall]" >&2
      exit 1
      ;;
  esac

  if [[ -d "${PACKAGE_DIR}/bin" && -d "${PACKAGE_DIR}/skill/slurm-assistant" ]]; then
    case "$action" in
      install) install_from_bundle ;;
      uninstall) uninstall_all ;;
    esac
    return
  fi

  run_from_downloaded_bundle "$action"
}

main "$@"
