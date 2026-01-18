//! Vendor payload adapters
//!
//! This module provides adapters that parse raw vendor JSON payloads and map them
//! to canonical, vendor-agnostic structures.

mod garmin;
mod whoop;

pub use garmin::GarminAdapter;
pub use whoop::WhoopAdapter;

use crate::error::ComputeError;
use crate::types::CanonicalWearSignals;

/// Trait for vendor payload adapters
pub trait VendorPayloadAdapter {
    /// Parse raw JSON and convert to canonical signals
    fn parse(
        &self,
        raw_json: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<CanonicalWearSignals>, ComputeError>;
}
