#!/usr/bin/env bash
set -euo pipefail

# Builds an iOS XCFramework (static library) into dist/ios/SynheartFlux.xcframework.
#
# Requirements (local):
# - Xcode CLI tools (`xcodebuild` available)
# - Rust toolchain + iOS targets
#
# Output:
# - dist/ios/SynheartFlux.xcframework

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_DIR="${1:-dist/ios}"
XCFRAMEWORK_NAME="SynheartFlux.xcframework"

mkdir -p "$OUT_DIR"

rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

# Build static libs for device + simulators
cargo build --release --target aarch64-apple-ios
cargo build --release --target aarch64-apple-ios-sim
cargo build --release --target x86_64-apple-ios

DEVICE_LIB="target/aarch64-apple-ios/release/libsynheart_flux.a"
SIM_ARM64_LIB="target/aarch64-apple-ios-sim/release/libsynheart_flux.a"
SIM_X64_LIB="target/x86_64-apple-ios/release/libsynheart_flux.a"

HEADERS_DIR="include"
HEADER_FILE="$HEADERS_DIR/synheart_flux.h"

if [[ ! -f "$HEADER_FILE" ]]; then
  echo "Missing header: $HEADER_FILE" >&2
  exit 1
fi

rm -rf "$OUT_DIR/$XCFRAMEWORK_NAME"

xcodebuild -create-xcframework \
  -library "$DEVICE_LIB" -headers "$HEADERS_DIR" \
  -library "$SIM_ARM64_LIB" -headers "$HEADERS_DIR" \
  -library "$SIM_X64_LIB" -headers "$HEADERS_DIR" \
  -output "$OUT_DIR/$XCFRAMEWORK_NAME"

echo "Built iOS XCFramework: $OUT_DIR/$XCFRAMEWORK_NAME"

