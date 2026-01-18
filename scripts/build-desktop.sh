#!/usr/bin/env bash
set -euo pipefail

# Builds desktop (host) artifacts into dist/desktop/<platform>/.
#
# Output (varies by OS):
# - libsynheart_flux.(dylib|so|dll)
# - libsynheart_flux.a

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_BASE="${1:-dist/desktop}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
PLATFORM="${OS}-${ARCH}"

OUT_DIR="$OUT_BASE/$PLATFORM"
mkdir -p "$OUT_DIR"

cargo build --release

if [[ "$OS" == "darwin" ]]; then
  cp -f target/release/libsynheart_flux.dylib "$OUT_DIR/"
  cp -f target/release/libsynheart_flux.a "$OUT_DIR/"
elif [[ "$OS" == "linux" ]]; then
  cp -f target/release/libsynheart_flux.so "$OUT_DIR/"
  cp -f target/release/libsynheart_flux.a "$OUT_DIR/"
else
  echo "Unsupported host OS for this script: $OS" >&2
  echo "For Windows builds, use the GitHub Actions workflow (or build via PowerShell/cargo)." >&2
  exit 1
fi

cp -f include/synheart_flux.h "$OUT_DIR/"

echo "Built desktop artifacts into: $OUT_DIR"

