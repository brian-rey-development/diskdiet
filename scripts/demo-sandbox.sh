#!/usr/bin/env bash
# Builds a fake home folder for recording the demo, so the GIF never shows real project names.
# Prints the sandbox path on stdout. Use it as HOME: export HOME="$(./scripts/demo-sandbox.sh)"
set -euo pipefail

TEMP_ROOT="${TMPDIR:-/tmp}"
SANDBOX="${DISKDIET_DEMO_HOME:-${TEMP_ROOT%/}/diskdiet-demo}"
SANDBOX="${SANDBOX%/}"
MARKER=".diskdiet-demo-sandbox"
DEV="$SANDBOX/Develop"
REPO="$DEV/acme-app"
OLD_DATE="202401010000"

reset_sandbox() {
  if [[ -e "$SANDBOX" && ! -e "$SANDBOX/$MARKER" ]]; then
    echo "Refusing to reuse $SANDBOX, it was not created by this script." >&2
    exit 1
  fi
  rm -rf "$SANDBOX"
  mkdir -p "$SANDBOX/.config/diskdiet" "$SANDBOX/Downloads" "$DEV"
  touch "$SANDBOX/$MARKER"
  printf 'dev_roots = ["~/Develop"]\n' > "$SANDBOX/.config/diskdiet/config.toml"
}

blob() {
  mkdir -p "$(dirname "$1")"
  dd if=/dev/zero of="$1" bs=1048576 count="$2" 2>/dev/null
}

git_in() {
  HOME="$SANDBOX" git -C "$1" -c user.name=Demo -c user.email=demo@example.com "${@:2}"
}

make_repo() {
  mkdir -p "$REPO"
  git_in "$REPO" init -q -b main
  printf 'target/\n.env\n.claude/\n' > "$REPO/.gitignore"
  printf '[package]\nname = "acme-app"\n' > "$REPO/Cargo.toml"
  git_in "$REPO" add -A
  git_in "$REPO" commit -qm "Initial commit"
}

add_worktree() {
  local path="$REPO/.claude/worktrees/$1"
  git_in "$REPO" worktree add -q -b "$1" "$path"
  blob "$path/target/debug/acme-app" "$2"
  echo "$path"
}

make_worktrees() {
  add_worktree fix-login-redirect 160 > /dev/null
  local spike
  spike="$(add_worktree spike-edge-cache 90)"
  echo "fn cache() {}" > "$spike/cache.rs"
  git_in "$spike" add cache.rs
  git_in "$spike" commit -qm "Try an edge cache"
  local refactor
  refactor="$(add_worktree agent-refactor-auth 120)"
  echo "STRIPE_KEY=sk_test_demo" > "$refactor/.env"
}

make_projects() {
  printf '[package]\nname = "billing"\n' > "$(mkdir -p "$DEV/billing" && echo "$DEV/billing/Cargo.toml")"
  blob "$DEV/billing/target/release/billing" 240
  echo '{"name": "landing"}' > "$(mkdir -p "$DEV/landing" && echo "$DEV/landing/package.json")"
  blob "$DEV/landing/.next/cache/webpack.pack" 70
  blob "$DEV/landing/node_modules/next/dist/server.js" 110
  touch -t "$OLD_DATE" "$DEV/landing/package.json"
  blob "$SANDBOX/Downloads/SomeApp-2.4.dmg" 60
  touch -t "$OLD_DATE" "$SANDBOX/Downloads/SomeApp-2.4.dmg"
}

reset_sandbox
make_repo
make_worktrees
make_projects
echo "$SANDBOX"
