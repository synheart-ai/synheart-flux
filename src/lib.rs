//! Synheart Flux - On-device compute engine for HSI-compliant human state signals
//!
//! Flux transforms raw wearable vendor data into HSI-compliant signals through a
//! deterministic pipeline: vendor adaptation → normalization → feature derivation
//! → baseline computation → HSI encoding.

pub mod adapters;
pub mod baseline;
pub mod encoder;
pub mod error;
pub mod features;
pub mod normalizer;
pub mod pipeline;
pub mod types;

pub use error::ComputeError;
pub use pipeline::{garmin_to_hsi_daily, whoop_to_hsi_daily, FluxProcessor};

/// Flux version embedded in all HSI payloads
pub const FLUX_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Producer name for HSI payloads
pub const PRODUCER_NAME: &str = "synheart-flux";
