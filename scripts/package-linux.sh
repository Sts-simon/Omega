#!/usr/bin/env bash
# Build the same Linux x86_64 archive the release workflow publishes, locally.
# Usage: scripts/package-linux.sh [--skip-build]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$VERSION" ]]; then
    echo "cannot read the package version from Cargo.toml" >&2
    exit 1
fi

if [[ "${1:-}" != "--skip-build" ]]; then
    cargo build --locked --release
fi

PACKAGE="omega-v${VERSION}-linux-x86_64"
DIST="$ROOT/dist/$PACKAGE"
rm -rf "$DIST"
mkdir -p "$DIST"

install -m 0755 target/release/omega "$DIST/omega"
install -m 0644 README.md README.fr.md LICENSE "$DIST/"
install -m 0644 docs/VISION_MISSILES.md docs/INSTALLATION.md docs/BALANCE.md "$DIST/"

tar -C "$ROOT/dist" -czf "$ROOT/dist/$PACKAGE.tar.gz" "$PACKAGE"
(
    cd "$ROOT/dist"
    sha256sum "$PACKAGE.tar.gz" > "$PACKAGE.tar.gz.sha256"
)

echo "Built dist/$PACKAGE.tar.gz"
echo "Checksum: dist/$PACKAGE.tar.gz.sha256"
