# Synheart Flux

**Synheart Flux** is a Rust-based, on-device compute engine that transforms raw wearable vendor payloads (e.g. WHOOP, Garmin) into **HSI-compliant human state signals**.

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

Add to your `Cargo.toml`:

```toml
[dependencies]
synheart-flux = "0.1"
```

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

