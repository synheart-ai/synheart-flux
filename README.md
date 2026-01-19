# Synheart Flux

[![CI](https://github.com/synheart-ai/synheart-flux/actions/workflows/ci.yml/badge.svg)](https://github.com/synheart-ai/synheart-flux/actions/workflows/ci.yml)
[![Release](https://github.com/synheart-ai/synheart-flux/actions/workflows/release.yml/badge.svg)](https://github.com/synheart-ai/synheart-flux/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/synheart-flux.svg)](https://crates.io/crates/synheart-flux)
[![docs.rs](https://img.shields.io/docsrs/synheart-flux)](https://docs.rs/synheart-flux)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)

**Synheart Flux** is an on-device compute engine that transforms raw wearable vendor payloads (e.g. WHOOP, Garmin) and smartphone behavioral data into **HSI-compliant human state signals**.

Flux centralizes two parallel pipelines:
- **Wearable Pipeline**: vendor adaptation → normalization → feature derivation → baseline computation → HSI encoding
- **Behavioral Pipeline**: session parsing → normalization → metric computation → baseline tracking → HSI encoding

## What this crate does

### Wearable Processing
- **Parse vendor JSON** into canonical, vendor-agnostic structures
- **Normalize** units and scales (and surface data quality flags)
- **Derive features** (sleep efficiency/fragmentation, normalized load, etc.)
- **Maintain rolling baselines** for relative interpretation (HRV, RHR, sleep)
- **Encode** daily windows into HSI JSON with provenance + quality/confidence

### Behavioral Metrics
- **Parse behavioral session JSON** (taps, scrolls, notifications, app switches, typing)
- **Compute engagement metrics** (distraction score, focus hint, burstiness, interaction intensity)
- **Detect patterns** (idle segments, engagement blocks, scroll jitter, deep focus periods)
- **Maintain rolling baselines** across sessions (20-session default window)
- **Encode** behavioral windows into HSI JSON with deviation tracking

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

### Behavioral metrics (one-shot)

```rust
use synheart_flux::behavior_to_hsi;

fn main() -> Result<(), synheart_flux::ComputeError> {
    let session_json = r#"{
        "session_id": "sess-123",
        "device_id": "device-456",
        "timezone": "America/New_York",
        "start_time": "2024-01-15T14:00:00Z",
        "end_time": "2024-01-15T14:30:00Z",
        "events": [
            {"timestamp": "2024-01-15T14:01:00Z", "event_type": "scroll", "scroll": {"velocity": 150.5, "direction": "down"}},
            {"timestamp": "2024-01-15T14:02:00Z", "event_type": "tap", "tap": {"tap_duration_ms": 120}},
            {"timestamp": "2024-01-15T14:03:00Z", "event_type": "notification", "interruption": {"action": "ignored"}},
            {"timestamp": "2024-01-15T14:05:00Z", "event_type": "app_switch", "app_switch": {"from_app_id": "app1", "to_app_id": "app2"}}
        ]
    }"#.to_string();

    let hsi_json = behavior_to_hsi(session_json)?;
    println!("Behavioral HSI: {}", hsi_json);
    Ok(())
}
```

### Behavioral metrics with persistent baselines

```rust
use synheart_flux::BehaviorProcessor;

fn main() -> Result<(), synheart_flux::ComputeError> {
    let mut processor = BehaviorProcessor::with_baseline_window(20); // 20 sessions

    // Load baseline state from disk/keychain/etc.
    // processor.load_baselines(&saved_json)?;

    let session_json = r#"{"session_id": "...", "device_id": "...", ...}"#;
    let hsi = processor.process(session_json)?;

    // Save baseline state for next run
    let saved_json = processor.save_baselines()?;
    println!("Saved baselines JSON size: {}", saved_json.len());

    println!("Behavioral HSI: {}", hsi);
    Ok(())
}
```

## Output

Flux emits **HSI 1.0 JSON** payloads that conform to the Human State Interface specification:

### Required Fields

- `hsi_version` — Schema version (e.g., `"1.0"`)
- `observed_at_utc` — When the data was observed
- `computed_at_utc` — When HSI was computed
- `producer` — Name, version, and instance_id of the producing software
- `window_ids` / `windows` — Time windows with start/end timestamps
- `source_ids` / `sources` — Data sources with type and quality
- `axes` — Behavioral readings organized by domain
- `privacy` — Data handling declarations

### Behavioral Output Example

```json
{
  "hsi_version": "1.0",
  "observed_at_utc": "2024-01-15T14:30:00+00:00",
  "computed_at_utc": "2024-01-15T14:30:01+00:00",
  "producer": {
    "name": "synheart-flux",
    "version": "0.1.0",
    "instance_id": "550e8400-e29b-41d4-a716-446655440000"
  },
  "window_ids": ["w_session_123"],
  "windows": {
    "w_session_123": {
      "start": "2024-01-15T14:00:00+00:00",
      "end": "2024-01-15T14:30:00+00:00",
      "label": "session:session-123"
    }
  },
  "source_ids": ["s_device_456"],
  "sources": {
    "s_device_456": {
      "type": "app",
      "quality": 0.95,
      "degraded": false
    }
  },
  "axes": {
    "behavior": {
      "readings": [
        { "axis": "distraction", "score": 0.35, "confidence": 0.95, "window_id": "w_session_123", "direction": "higher_is_more", "evidence_source_ids": ["s_device_456"] },
        { "axis": "focus", "score": 0.65, "confidence": 0.95, "window_id": "w_session_123", "direction": "higher_is_more", "evidence_source_ids": ["s_device_456"] },
        { "axis": "task_switch_rate", "score": 0.42, "confidence": 0.95, "window_id": "w_session_123", "direction": "higher_is_more", "unit": "normalized", "evidence_source_ids": ["s_device_456"] },
        { "axis": "burstiness", "score": 0.55, "confidence": 0.95, "window_id": "w_session_123", "direction": "bidirectional", "unit": "barabasi_index", "evidence_source_ids": ["s_device_456"] }
      ]
    }
  },
  "privacy": {
    "contains_pii": false,
    "raw_biosignals_allowed": false,
    "derived_metrics_allowed": true,
    "purposes": ["behavioral_research"]
  },
  "meta": {
    "session_id": "session-123",
    "baseline_distraction": 0.38,
    "sessions_in_baseline": 15,
    "duration_sec": 1800.0,
    "total_events": 245
  }
}
```

### Behavioral Axes

| Axis | Direction | Description |
|------|-----------|-------------|
| `distraction` | higher_is_more | Composite distraction score (0-1) |
| `focus` | higher_is_more | Inverse of distraction (0-1) |
| `task_switch_rate` | higher_is_more | App switch frequency (normalized) |
| `notification_load` | higher_is_more | Notification frequency (normalized) |
| `burstiness` | bidirectional | Temporal clustering (Barabási index) |
| `scroll_jitter_rate` | higher_is_more | Direction reversals ratio |
| `interaction_intensity` | higher_is_more | Events per second (normalized) |
| `idle_ratio` | higher_is_more | Idle time ratio |

## Feature flags

- **`ffi`**: Enables the C FFI bindings for mobile and cross-language integration. Provides:
  - Wearable functions: `flux_whoop_to_hsi_daily`, `flux_garmin_to_hsi_daily`, and stateful `FluxProcessor` API
  - Behavioral functions: `flux_behavior_to_hsi`, and stateful `BehaviorProcessor` API

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
