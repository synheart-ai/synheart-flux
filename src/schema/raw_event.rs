//! wear.raw_event.v1 schema definition
//!
//! A scalable, vendor-agnostic schema for wearable data that supports:
//! - Individual signal events (for real-time streaming)
//! - Session records (sleep, workouts)
//! - Daily/hourly summaries (batch aggregates)
//! - Vendor scores (recovery, strain, readiness)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current schema version
pub const SCHEMA_VERSION: &str = "wear.raw_event.v1";

/// Supported wearable providers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Whoop,
    Garmin,
    Apple,
    Oura,
    Fitbit,
    Polar,
    Coros,
    Suunto,
    Samsung,
    Withings,
    /// For custom/unknown providers, use Other with a name
    #[serde(untagged)]
    Other(String),
}

impl Provider {
    pub fn as_str(&self) -> &str {
        match self {
            Provider::Whoop => "whoop",
            Provider::Garmin => "garmin",
            Provider::Apple => "apple",
            Provider::Oura => "oura",
            Provider::Fitbit => "fitbit",
            Provider::Polar => "polar",
            Provider::Coros => "coros",
            Provider::Suunto => "suunto",
            Provider::Samsung => "samsung",
            Provider::Withings => "withings",
            Provider::Other(name) => name.as_str(),
        }
    }
}

/// Data source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Wearable provider (whoop, garmin, apple, etc.)
    pub provider: Provider,
    /// Device model (e.g., "WHOOP 4.0", "Garmin Fenix 7")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    /// Unique device identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Firmware/software version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
}

/// Type of record contained in the event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    /// Individual point-in-time signal reading
    Signal,
    /// Session record (sleep, workout, meditation)
    Session,
    /// Aggregated summary (daily, hourly)
    Summary,
    /// Vendor-computed score (recovery, strain, readiness)
    Score,
}

/// Signal types for individual readings
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    // Cardiovascular
    HeartRate,
    HeartRateVariability,
    RestingHeartRate,

    // Respiratory
    RespiratoryRate,
    Spo2,

    // Activity
    Steps,
    Calories,
    ActiveCalories,
    Distance,
    Floors,
    ActiveMinutes,

    // Body
    SkinTemperature,
    BodyTemperature,
    Weight,
    BodyFat,

    // Sleep stages (for sleep tracking)
    SleepStage,

    // Other
    Stress,
    Energy,
    BodyBattery,

    /// For extensibility
    #[serde(untagged)]
    Custom(String),
}

/// Measurement unit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    // Time
    Milliseconds,
    Seconds,
    Minutes,
    Hours,

    // Cardiovascular
    Bpm,           // beats per minute
    Ms,            // milliseconds (for HRV)

    // Respiratory
    BreathsPerMin,
    Percent,       // for SpO2

    // Activity
    Count,         // steps, floors
    Kcal,
    Kj,
    Meters,
    Kilometers,
    Miles,

    // Body
    Celsius,
    Fahrenheit,
    Kg,
    Lbs,

    // Generic
    Score,         // normalized 0-100 or 0-1
    Level,         // categorical (awake, light, deep, rem)

    /// For extensibility
    #[serde(untagged)]
    Custom(String),
}

/// Individual signal reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalPayload {
    /// Type of signal
    #[serde(rename = "type")]
    pub signal_type: SignalType,
    /// Numeric value
    pub value: f64,
    /// Measurement unit
    pub unit: Unit,
    /// Data quality/confidence (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<f64>,
}

/// Session types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Sleep,
    Nap,
    Workout,
    Meditation,
    Recovery,
    /// For extensibility
    #[serde(untagged)]
    Custom(String),
}

/// Session record (sleep, workout, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPayload {
    /// Type of session
    #[serde(rename = "type")]
    pub session_type: SessionType,
    /// Session start time (UTC)
    pub start_time: DateTime<Utc>,
    /// Session end time (UTC)
    pub end_time: DateTime<Utc>,
    /// Session metrics (flexible key-value pairs)
    /// Keys use snake_case naming convention
    #[serde(default)]
    pub metrics: HashMap<String, MetricValue>,
}

/// Summary period type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryPeriod {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

/// Aggregated summary record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryPayload {
    /// Aggregation period
    pub period: SummaryPeriod,
    /// Date for the summary (YYYY-MM-DD for daily, YYYY-MM-DDTHH for hourly)
    pub date: String,
    /// Summary metrics (flexible key-value pairs)
    #[serde(default)]
    pub metrics: HashMap<String, MetricValue>,
}

/// Score types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreType {
    Recovery,
    Strain,
    Sleep,
    Readiness,
    Stress,
    BodyBattery,
    TrainingLoad,
    /// For extensibility
    #[serde(untagged)]
    Custom(String),
}

/// Score scale definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreScale {
    pub min: f64,
    pub max: f64,
}

/// Vendor-computed score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorePayload {
    /// Type of score
    #[serde(rename = "type")]
    pub score_type: ScoreType,
    /// Score value
    pub value: f64,
    /// Score scale (for normalization)
    pub scale: ScoreScale,
    /// Component scores that make up the total
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub components: HashMap<String, f64>,
}

/// Flexible metric value (supports various types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Number(f64),
    Integer(i64),
    String(String),
    Boolean(bool),
    Array(Vec<MetricValue>),
    Object(HashMap<String, MetricValue>),
}

impl From<f64> for MetricValue {
    fn from(v: f64) -> Self {
        MetricValue::Number(v)
    }
}

impl From<i64> for MetricValue {
    fn from(v: i64) -> Self {
        MetricValue::Integer(v)
    }
}

impl From<String> for MetricValue {
    fn from(v: String) -> Self {
        MetricValue::String(v)
    }
}

impl From<bool> for MetricValue {
    fn from(v: bool) -> Self {
        MetricValue::Boolean(v)
    }
}

impl MetricValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MetricValue::Number(n) => Some(*n),
            MetricValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            MetricValue::Integer(i) => Some(*i),
            MetricValue::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            MetricValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            MetricValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

/// Event payload - one of the four record types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Payload {
    Signal { signal: SignalPayload },
    Session { session: SessionPayload },
    Summary { summary: SummaryPayload },
    Score { score: ScorePayload },
}

/// Optional context for the event
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    /// Activity type if relevant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_type: Option<String>,
    /// Related session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// User timezone (IANA format, e.g., "America/New_York")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Arbitrary tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// The main wear.raw_event.v1 schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEvent {
    /// Schema version identifier
    pub schema_version: String,
    /// Unique event identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// Event timestamp (UTC)
    pub timestamp: DateTime<Utc>,
    /// Data source information
    pub source: Source,
    /// Optional user identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Type of record
    pub record_type: RecordType,
    /// Event payload (depends on record_type)
    pub payload: Payload,
    /// Optional context
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Context>,
    /// Raw vendor data (preserved for debugging/transparency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_raw: Option<serde_json::Value>,
}

impl RawEvent {
    /// Create a new signal event
    pub fn signal(
        timestamp: DateTime<Utc>,
        source: Source,
        signal: SignalPayload,
    ) -> Self {
        RawEvent {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: Some(uuid::Uuid::new_v4().to_string()),
            timestamp,
            source,
            user_id: None,
            record_type: RecordType::Signal,
            payload: Payload::Signal { signal },
            context: None,
            vendor_raw: None,
        }
    }

    /// Create a new session event
    pub fn session(
        timestamp: DateTime<Utc>,
        source: Source,
        session: SessionPayload,
    ) -> Self {
        RawEvent {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: Some(uuid::Uuid::new_v4().to_string()),
            timestamp,
            source,
            user_id: None,
            record_type: RecordType::Session,
            payload: Payload::Session { session },
            context: None,
            vendor_raw: None,
        }
    }

    /// Create a new summary event
    pub fn summary(
        timestamp: DateTime<Utc>,
        source: Source,
        summary: SummaryPayload,
    ) -> Self {
        RawEvent {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: Some(uuid::Uuid::new_v4().to_string()),
            timestamp,
            source,
            user_id: None,
            record_type: RecordType::Summary,
            payload: Payload::Summary { summary },
            context: None,
            vendor_raw: None,
        }
    }

    /// Create a new score event
    pub fn score(
        timestamp: DateTime<Utc>,
        source: Source,
        score: ScorePayload,
    ) -> Self {
        RawEvent {
            schema_version: SCHEMA_VERSION.to_string(),
            event_id: Some(uuid::Uuid::new_v4().to_string()),
            timestamp,
            source,
            user_id: None,
            record_type: RecordType::Score,
            payload: Payload::Score { score },
            context: None,
            vendor_raw: None,
        }
    }

    /// Add context to the event
    pub fn with_context(mut self, context: Context) -> Self {
        self.context = Some(context);
        self
    }

    /// Add user ID to the event
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Preserve vendor raw data
    pub fn with_vendor_raw(mut self, raw: serde_json::Value) -> Self {
        self.vendor_raw = Some(raw);
        self
    }

    /// Validate the event schema
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Check schema version
        if self.schema_version != SCHEMA_VERSION {
            return Err(ValidationError::InvalidSchemaVersion {
                expected: SCHEMA_VERSION.to_string(),
                actual: self.schema_version.clone(),
            });
        }

        // Validate payload matches record type
        match (&self.record_type, &self.payload) {
            (RecordType::Signal, Payload::Signal { .. }) => Ok(()),
            (RecordType::Session, Payload::Session { .. }) => Ok(()),
            (RecordType::Summary, Payload::Summary { .. }) => Ok(()),
            (RecordType::Score, Payload::Score { .. }) => Ok(()),
            _ => Err(ValidationError::PayloadTypeMismatch {
                record_type: format!("{:?}", self.record_type),
                payload_type: self.payload_type_name(),
            }),
        }
    }

    fn payload_type_name(&self) -> String {
        match &self.payload {
            Payload::Signal { .. } => "signal".to_string(),
            Payload::Session { .. } => "session".to_string(),
            Payload::Summary { .. } => "summary".to_string(),
            Payload::Score { .. } => "score".to_string(),
        }
    }
}

/// Validation errors for raw events
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid schema version: expected {expected}, got {actual}")]
    InvalidSchemaVersion { expected: String, actual: String },

    #[error("Payload type mismatch: record_type is {record_type} but payload is {payload_type}")]
    PayloadTypeMismatch { record_type: String, payload_type: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_signal_event() {
        let source = Source {
            provider: Provider::Whoop,
            device_model: Some("WHOOP 4.0".to_string()),
            device_id: Some("device-123".to_string()),
            firmware_version: None,
        };

        let signal = SignalPayload {
            signal_type: SignalType::HeartRate,
            value: 72.0,
            unit: Unit::Bpm,
            quality: Some(0.95),
        };

        let event = RawEvent::signal(Utc::now(), source, signal);
        let json = serde_json::to_string_pretty(&event).unwrap();

        assert!(json.contains("wear.raw_event.v1"));
        assert!(json.contains("heart_rate"));
        assert!(json.contains("bpm"));
    }

    #[test]
    fn test_serialize_session_event() {
        let source = Source {
            provider: Provider::Garmin,
            device_model: Some("Fenix 7".to_string()),
            device_id: None,
            firmware_version: None,
        };

        let mut metrics = HashMap::new();
        metrics.insert("total_sleep_minutes".to_string(), MetricValue::Number(420.0));
        metrics.insert("deep_sleep_minutes".to_string(), MetricValue::Number(90.0));
        metrics.insert("rem_sleep_minutes".to_string(), MetricValue::Number(100.0));
        metrics.insert("awakenings".to_string(), MetricValue::Integer(3));

        let session = SessionPayload {
            session_type: SessionType::Sleep,
            start_time: Utc::now() - chrono::Duration::hours(8),
            end_time: Utc::now(),
            metrics,
        };

        let event = RawEvent::session(Utc::now(), source, session);
        let json = serde_json::to_string_pretty(&event).unwrap();

        assert!(json.contains("session"));
        assert!(json.contains("sleep"));
        assert!(json.contains("total_sleep_minutes"));
    }

    #[test]
    fn test_serialize_score_event() {
        let source = Source {
            provider: Provider::Whoop,
            device_model: None,
            device_id: None,
            firmware_version: None,
        };

        let mut components = HashMap::new();
        components.insert("hrv_contribution".to_string(), 0.3);
        components.insert("rhr_contribution".to_string(), 0.25);
        components.insert("sleep_contribution".to_string(), 0.45);

        let score = ScorePayload {
            score_type: ScoreType::Recovery,
            value: 78.0,
            scale: ScoreScale { min: 0.0, max: 100.0 },
            components,
        };

        let event = RawEvent::score(Utc::now(), source, score);
        let json = serde_json::to_string_pretty(&event).unwrap();

        assert!(json.contains("recovery"));
        assert!(json.contains("78"));
    }

    #[test]
    fn test_deserialize_signal_event() {
        let json = r#"{
            "schema_version": "wear.raw_event.v1",
            "timestamp": "2024-01-15T08:30:00Z",
            "source": {
                "provider": "whoop",
                "device_model": "WHOOP 4.0"
            },
            "record_type": "signal",
            "payload": {
                "signal": {
                    "type": "heart_rate",
                    "value": 65.0,
                    "unit": "bpm"
                }
            }
        }"#;

        let event: RawEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.schema_version, SCHEMA_VERSION);
        assert!(matches!(event.source.provider, Provider::Whoop));
        assert!(matches!(event.record_type, RecordType::Signal));
    }

    #[test]
    fn test_validation() {
        let source = Source {
            provider: Provider::Whoop,
            device_model: None,
            device_id: None,
            firmware_version: None,
        };

        let signal = SignalPayload {
            signal_type: SignalType::HeartRate,
            value: 72.0,
            unit: Unit::Bpm,
            quality: None,
        };

        let event = RawEvent::signal(Utc::now(), source, signal);
        assert!(event.validate().is_ok());
    }
}
