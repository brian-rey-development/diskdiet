#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="diskdiet"
BIN_DIR="${HOME}/.local/bin"
BIN_PATH="${BIN_DIR}/${BIN_NAME}"
CONFIG_DIR="${HOME}/.config/diskdiet"
CONFIG_PATH="${CONFIG_DIR}/config.toml"
EXAMPLE_CONFIG="${REPO_ROOT}/config/diskdiet.example.toml"
RUSTUP_URL="https://sh.rustup.rs"
RUSTUP_COMMAND="curl --proto '=https' --tlsv1.2 -sSf ${RUSTUP_URL} | sh"

if [[ -t 1 ]]; then
  BOLD=$'\033[1m'; DIM=$'\033[2m'; RED=$'\033[31m'
  GREEN=$'\033[32m'; YELLOW=$'\033[33m'; CYAN=$'\033[36m'; RESET=$'\033[0m'
else
  BOLD=""; DIM=""; RED=""; GREEN=""; YELLOW=""; CYAN=""; RESET=""
fi

step() { printf '%s==>%s %s%s%s\n' "${CYAN}" "${RESET}" "${BOLD}" "$*" "${RESET}"; }
ok() { printf '    %sok%s  %s\n' "${GREEN}" "${RESET}" "$*"; }
warn() { printf '    %swarn%s %s\n' "${YELLOW}" "${RESET}" "$*"; }
die() { printf '%serror%s %s\n' "${RED}" "${RESET}" "$*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: ./setup.sh [--uninstall] [--help]

Builds diskdiet from source and installs it for the current user.

  (no flags)   check the platform and Rust, build in release mode, install the binary
               to ${BIN_PATH}, and create ${CONFIG_PATH}
               if it does not exist
  --uninstall  remove the binary, and ask before removing ${CONFIG_DIR}
  --help       show this message

Running it again is safe. The binary is replaced and an existing config is kept.
EOF
}

confirm() {
  local prompt="$1" reply=""
  if [[ ! -t 0 ]]; then
    return 1
  fi
  read -r -p "${prompt} [y/N] " reply
  [[ "${reply}" =~ ^[Yy]$ ]]
}

require_supported_platform() {
  step "Checking platform"
  case "$(uname -s)" in
    Darwin) ok "macOS $(sw_vers -productVersion 2>/dev/null || echo unknown)" ;;
    Linux) ok "Linux $(uname -r)" ;;
    *) die "diskdiet supports macOS and Linux (found $(uname -s))." ;;
  esac
}

load_cargo_env() {
  if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "${HOME}/.cargo/env"
  fi
}

install_rustup() {
  local installer
  installer="$(mktemp -t rustup-init.XXXXXX)"
  curl --proto '=https' --tlsv1.2 -sSf "${RUSTUP_URL}" -o "${installer}"
  printf '    downloaded the rustup installer to %s\n' "${installer}"
  sh "${installer}"
  rm -f "${installer}"
  load_cargo_env
}

require_rust() {
  step "Checking Rust toolchain"
  load_cargo_env
  if command -v cargo >/dev/null 2>&1; then
    ok "$(cargo --version)"
    return
  fi
  warn "cargo was not found. The official rustup install command is"
  printf '\n      %s\n\n' "${RUSTUP_COMMAND}"
  if confirm "    Download the installer to a temp file and run it now?"; then
    install_rustup
    command -v cargo >/dev/null 2>&1 || die "rustup finished but cargo is still not on PATH. Open a new shell and rerun ./setup.sh."
    ok "$(cargo --version)"
    return
  fi
  die "Rust is required. Install it with the command above, then rerun ./setup.sh."
}

build_release() {
  step "Building diskdiet (release)"
  (cd "${REPO_ROOT}" && cargo build --release --locked --target-dir "${REPO_ROOT}/target")
  [[ -x "${REPO_ROOT}/target/release/${BIN_NAME}" ]] || die "Build finished but target/release/${BIN_NAME} is missing."
  ok "built target/release/${BIN_NAME}"
}

install_binary() {
  step "Installing binary"
  mkdir -p "${BIN_DIR}"
  command install -m 755 "${REPO_ROOT}/target/release/${BIN_NAME}" "${BIN_PATH}"
  ok "installed ${BIN_PATH}"
  case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *)
      warn "${BIN_DIR} is not on your PATH. Add this line to your shell profile (~/.zshrc or ~/.bashrc) and open a new terminal"
      # shellcheck disable=SC2016
      printf '\n      export PATH="$HOME/.local/bin:$PATH"\n\n'
      ;;
  esac
}

install_config() {
  step "Setting up config"
  if [[ -f "${CONFIG_PATH}" ]]; then
    ok "kept existing ${CONFIG_PATH}"
    return
  fi
  mkdir -p "${CONFIG_DIR}"
  cp "${EXAMPLE_CONFIG}" "${CONFIG_PATH}"
  ok "created ${CONFIG_PATH}"
}

print_next_steps() {
  step "Done"
  printf '    Next, see what diskdiet would remove without deleting anything\n\n'
  printf '      %sdiskdiet --dry-run%s\n\n' "${BOLD}" "${RESET}"
  printf '    %sWhen you are happy with the list, run diskdiet without flags.%s\n' "${DIM}" "${RESET}"
}

uninstall() {
  step "Removing binary"
  if [[ -e "${BIN_PATH}" ]]; then
    rm -f "${BIN_PATH}"
    ok "removed ${BIN_PATH}"
  else
    ok "no binary at ${BIN_PATH}"
  fi
  step "Config"
  if [[ ! -d "${CONFIG_DIR}" ]]; then
    ok "no config at ${CONFIG_DIR}"
  elif confirm "    Remove ${CONFIG_DIR}?"; then
    rm -rf "${CONFIG_DIR}"
    ok "removed ${CONFIG_DIR}"
  else
    ok "kept ${CONFIG_DIR}"
  fi
  if [[ -f "${HOME}/Library/Logs/diskdiet.log" ]]; then
    ok "kept the action log at ~/Library/Logs/diskdiet.log (delete it by hand if you want)"
  fi
  step "Done"
}

run_install() {
  require_supported_platform
  require_rust
  build_release
  install_binary
  install_config
  print_next_steps
}

main() {
  case "${1:-}" in
    "") run_install ;;
    --uninstall) require_macos; uninstall ;;
    --help | -h) usage ;;
    *) usage >&2; die "unknown option $1" ;;
  esac
}

main "$@"
