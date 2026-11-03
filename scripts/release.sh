#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cat >&2 <<'EOF'
STUB: scripts/release.sh is not implemented yet.

Planned release flow (cargo-dist):

  1. Run scripts/check.sh and bump the version in Cargo.toml and CHANGELOG.md.
  2. Run `cargo dist init` once to generate the release workflow and dist config.
  3. Tag the release (git tag vX.Y.Z) and push the tag.
  4. GitHub Actions builds macOS and Linux binaries (aarch64 and x86_64) and publishes them
     to GitHub Releases with checksums.
  5. cargo-dist updates the Homebrew tap (brian-rey-development/homebrew-tap)
     so `brew install diskdiet` works.
  6. cargo-dist publishes the npm wrapper so `npx diskdiet` downloads the
     matching release binary.

Until then, install from source with ./setup.sh.
EOF

exit 1
