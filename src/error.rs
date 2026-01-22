//! Error types for Synheart Flux

use thiserror::Error;

/// Errors that can occur during computation
#[derive(Debug, Error)]
pub enum ComputeError {
    #[error("Failed to parse vendor payload: {0}")]
    ParseError(String),

    #[error("Invalid JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),

    #[error("Date parse error: {0}")]
    DateParseError(String),

    #[error("Normalization error: {0}")]
    NormalizationError(String),

    #[error("Feature derivation error: {0}")]
    FeatureError(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Unsupported vendor: {0}")]
    UnsupportedVendor(String),

    #[error("Invalid behavioral session: {0}")]
    InvalidBehaviorSession(String),

    #[error("Insufficient events for computation: {0}")]
    InsufficientEvents(String),
}
