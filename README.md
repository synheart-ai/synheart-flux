# Synheart Flux

[![CI](https://github.com/synheart-ai/synheart-flux/actions/workflows/ci.yml/badge.svg)](https://github.com/synheart-ai/synheart-flux/actions/workflows/ci.yml)
[![Release](https://github.com/synheart-ai/synheart-flux/actions/workflows/release.yml/badge.svg)](https://github.com/synheart-ai/synheart-flux/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/synheart-flux.svg)](https://crates.io/crates/synheart-flux)
[![docs.rs](https://img.shields.io/docsrs/synheart-flux)](https://docs.rs/synheart-flux)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)

**Synheart Flux** is an on-device compute engine that transforms raw wearable vendor payloads (e.g. WHOOP, Garmin) into **HSI-compliant human state signals**.

Flux centralizes a deterministic pipeline:
**vendor adaptation → normalization → feature derivation → baseline computation → HSI encoding**.

## What this crate does

- **Parse vendor JSON** into canonical, vendor-agnostic structures
- **Normalize** units and scales (and surface data quality flags)
- **Derive features** (sleep efficiency/fragmentation, normalized load, etc.)
- **Maintain rolling baselines** for relative interpretation (HRV, RHR, sleep)
- **Encode** daily windows into HSI JSON with provenance + quality/confidence

## Non-goals

- Emotion inference or labeling
- Medical/diagnostic claims
- UI, visualization, or cloud-side compute
- Replacing wearable ingestion/auth SDKs

## Install

### Rust (crate)

Add to your `Cargo.toml`:

```toml
[dependencies]
synheart-flux = "0.1"
```

### Platform installs (Android / iOS / Flutter / Desktop)

Flux is typically **bundled into a host SDK** (e.g. Synheart Wear) as native artifacts.

- **Recommended (prebuilt)**: download artifacts from **GitHub Releases** for a tag like `v0.1.0` and vendor them into your SDK repo.
- **Fallback (from source)**: build artifacts in CI (or locally) using the scripts in `scripts/`.


#### Android (JNI `.so` inside an AAR)

- **Get**: release asset `synheart-flux-android-jniLibs.tar.gz`
- **Place** in your Android library/module:

```text
src/main/jniLibs/
  arm64-v8a/libsynheart_flux.so
  armeabi-v7a/libsynheart_flux.so
  x86_64/libsynheart_flux.so
```

- **Build from source**:

```bash
ANDROID_NDK_HOME=/path/to/ndk bash scripts/build-android.sh dist/android/jniLibs
```

#### iOS (XCFramework)

- **Get**: release asset `synheart-flux-ios-xcframework.zip`
- **Place**:

```text
ios/Frameworks/SynheartFlux.xcframework
```

- **Build from source (macOS)**:

```bash
bash scripts/build-ios-xcframework.sh dist/ios
```

#### Flutter (plugin bundles Android + iOS)

Bundle the same artifacts in your Flutter plugin:

```text
android/src/main/jniLibs/*/libsynheart_flux.so
ios/Frameworks/SynheartFlux.xcframework
```

#### Desktop (macOS / Linux / Windows)

- **Get** (examples):
  - `synheart-flux-desktop-linux-x86_64.tar.gz`
  - `synheart-flux-desktop-macos-<arch>.tar.gz`
  - `synheart-flux-desktop-windows-x86_64.zip`
- Use them in your app/tooling distribution (or load dynamically via FFI).

## Usage

### One-shot conversion (stateless)

```rust
use synheart_flux::{garmin_to_hsi_daily, whoop_to_hsi_daily};

fn main() -> Result<(), synheart_flux::ComputeError> {
    let whoop_json = r#"{"sleep": [], "recovery": [], "cycle": []}"#.to_string();
    let garmin_json = r#"{"dailies": [], "sleep": []}"#.to_string();

    let whoop_hsi = whoop_to_hsi_daily(
        whoop_json,
        "America/New_York".to_string(),
        "device-123".to_string(),
    )?;

    let garmin_hsi = garmin_to_hsi_daily(
        garmin_json,
        "America/Los_Angeles".to_string(),
        "garmin-device-456".to_string(),
    )?;

    println!("WHOOP payloads: {}", whoop_hsi.len());
    println!("Garmin payloads: {}", garmin_hsi.len());
    Ok(())
}
```

### Persistent baselines across calls

If you want baselines to accumulate across multiple payloads (e.g., across app launches), use `FluxProcessor`.

```rust
use synheart_flux::FluxProcessor;

fn main() -> Result<(), synheart_flux::ComputeError> {
    let mut p = FluxProcessor::with_baseline_window(7);

    // Load baseline state from disk/keychain/etc.
    // p.load_baselines(&saved_json)?;

    let whoop_json = r#"{"sleep": [], "recovery": [], "cycle": []}"#;
    let hsi = p.process_whoop(whoop_json, "America/New_York", "device-123")?;

    // Save baseline state for next run
    let saved_json = p.save_baselines()?;
    println!("Saved baselines JSON size: {}", saved_json.len());

    println!("HSI payloads: {}", hsi.len());
    Ok(())
}
```

## Output

Flux emits **HSI JSON** payloads that include (at minimum):

- `hsi_version`
- `producer` (`name`, `version`, `instance_id`)
- `provenance` (`source_vendor`, `source_device_id`, timestamps)
- `quality` (`coverage`, `freshness_sec`, `confidence`, `flags`)
- `windows[]` (daily), with canonical namespaces such as `sleep.*`, `physiology.*`, `activity.*`, `baseline.*`

Vendor-specific metrics are preserved under `*.vendor.*` for transparency.

## Feature flags

- **`ffi`**: Enables the FFI feature set (intended for mobile bindings). *(Currently a placeholder feature flag in this crate.)*

## Development

```bash
cargo test
```

Recommended:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
```

## Contributing

See `CONTRIBUTING.md`. By contributing, you agree that your contributions will be licensed under the **Apache License 2.0**.

## Security

See `SECURITY.md` for how to report vulnerabilities.

## License

Licensed under the **Apache License, Version 2.0**. See `LICENSE`.


## Patent Pending Notice

This project is provided under an open-source license. Certain underlying systems, methods, and architectures described or implemented herein may be covered by one or more pending patent applications.

Nothing in this repository grants any license, express or implied, to any patents or patent applications, except as provided by the applicable open-source license.
