//! Synheart Flux - On-device compute engine for HSI-compliant human state signals
//!
//! Flux transforms raw wearable vendor data into HSI-compliant signals through a
//! deterministic pipeline: vendor adaptation → normalization → feature derivation
//! → baseline computation → HSI encoding.
//!
//! ## Modules
//!
//! - **Wearable Pipeline**: Process wearable device data (WHOOP, Garmin) into HSI signals
//! - **Behavior Module**: Process smartphone behavioral data into HSI signals

pub mod adapters;
pub mod baseline;
pub mod behavior;
pub mod context;
pub mod encoder;
pub mod error;
pub mod features;
pub mod normalizer;
pub mod pipeline;
pub mod schema;
pub mod types;

// FFI bindings for C interop (always available for cdylib/staticlib builds)
pub mod ffi;

pub use context::{BioDailyContext, DecayedBioContext};
pub use error::ComputeError;
pub use pipeline::{garmin_to_hsi_daily, whoop_to_hsi_daily, FluxProcessor};

// Schema exports
pub use schema::{RawEvent, RawEventAdapter, SCHEMA_VERSION};

// Behavioral exports
pub use behavior::{behavior_to_hsi, BehaviorProcessor};

/// Flux version embedded in all HSI payloads
pub const FLUX_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Producer name for HSI payloads
pub const PRODUCER_NAME: &str = "synheart-flux";
