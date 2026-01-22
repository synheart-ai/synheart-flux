//! Unified wear.raw_event.v1 schema
//!
//! This module defines the vendor-agnostic input schema for wearable data.
//! It supports both individual signal events (streaming) and session/summary
//! records (batch processing).

mod raw_event;
mod adapter;

pub use raw_event::*;
pub use adapter::*;
