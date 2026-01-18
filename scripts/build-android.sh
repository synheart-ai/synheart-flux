#!/usr/bin/env bash
set -euo pipefail

# Builds Android JNI libraries into dist/android/jniLibs/ using cargo-ndk.
#
# Requirements (local):
# - Rust toolchain + targets
# - Android NDK (ANDROID_NDK_HOME set)
# - cargo-ndk installed (`cargo install cargo-ndk`)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME is required (path to Android NDK)}"

OUT_DIR="${1:-dist/android/jniLibs}"

mkdir -p "$OUT_DIR"

rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# cargo-ndk uses -o/--output-dir (not --output).
cargo ndk \
  --target arm64-v8a \
  --target armeabi-v7a \
  --target x86_64 \
  -o "$OUT_DIR" \
  build --release

echo "Built Android JNI libs into: $OUT_DIR"

