//! Behavioral data types
//!
//! This module defines types for behavioral events and signals that flow through
//! the behavioral metrics pipeline.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Behavioral event types captured from smartphone usage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorEventType {
    Scroll,
    Tap,
    Swipe,
    Notification,
    Call,
    Typing,
    AppSwitch,
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Interruption action taken by the user
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptionAction {
    Ignored,
    Opened,
    Answered,
    Dismissed,
}

/// Scroll event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollEvent {
    /// Scroll velocity in pixels per second
    pub velocity: Option<f64>,
    /// Scroll direction
    pub direction: Option<ScrollDirection>,
    /// Whether this scroll reversed direction from the previous scroll
    #[serde(default)]
    pub direction_reversal: bool,
}

/// Tap event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapEvent {
    /// Duration of the tap in milliseconds
    pub tap_duration_ms: Option<u32>,
    /// Whether this was a long press (typically > 500ms)
    #[serde(default)]
    pub long_press: bool,
}

/// Swipe event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwipeEvent {
    /// Swipe direction
    pub direction: Option<ScrollDirection>,
    /// Swipe velocity in pixels per second
    pub velocity: Option<f64>,
}

/// Interruption event data (notifications, calls)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptionEvent {
    /// Action taken by the user
    pub action: InterruptionAction,
    /// App that generated the interruption
    pub source_app_id: Option<String>,
}

/// Typing event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingEvent {
    /// Typing speed.
    ///
    /// Notes:
    /// - Some producers report characters/minute (CPM) under `typing_speed_cpm`.
    /// - The SDK typing session metrics report taps/second under `typing_speed`.
    #[serde(default)]
    #[serde(alias = "typing_speed")]
    pub typing_speed_cpm: Option<f64>,

    /// Cadence stability (0-1, higher = more consistent rhythm).
    ///
    /// Notes:
    /// - Some producers report this under `typing_cadence_stability` (captured separately in
    ///   `typing_cadence_stability` below).
    #[serde(default)]
    pub cadence_stability: Option<f64>,

    /// Duration of the typing session in seconds.
    ///
    /// Notes:
    /// - The SDK uses `duration` (seconds) in typing-session payloads.
    #[serde(default)]
    #[serde(alias = "duration")]
    pub duration_sec: Option<f64>,

    /// Number of pauses during typing.
    #[serde(default)]
    pub pause_count: Option<u32>,

    // ---------------------------------------------------------------------
    // Optional detailed typing-session metrics (from BehaviorTextField / SDK)
    // ---------------------------------------------------------------------
    /// ISO8601 typing session start timestamp.
    #[serde(default)]
    pub start_at: Option<String>,

    /// ISO8601 typing session end timestamp.
    #[serde(default)]
    pub end_at: Option<String>,

    /// Total keyboard taps in the session.
    #[serde(default)]
    pub typing_tap_count: Option<u32>,

    /// Mean time between taps (milliseconds).
    #[serde(default)]
    pub mean_inter_tap_interval_ms: Option<f64>,

    /// Variability in inter-tap timing (milliseconds).
    #[serde(default)]
    pub typing_cadence_variability: Option<f64>,

    /// Normalized rhythmic consistency (0.0-1.0).
    #[serde(default)]
    pub typing_cadence_stability: Option<f64>,

    /// Number of pauses exceeding threshold.
    #[serde(default)]
    pub typing_gap_count: Option<u32>,

    /// Proportion of intervals that are gaps.
    #[serde(default)]
    pub typing_gap_ratio: Option<f64>,

    /// Dispersion of inter-tap intervals.
    #[serde(default)]
    pub typing_burstiness: Option<f64>,

    /// Fraction of the window with active typing.
    #[serde(default)]
    pub typing_activity_ratio: Option<f64>,

    /// Composite engagement measure for typing.
    #[serde(default)]
    pub typing_interaction_intensity: Option<f64>,

    /// Whether this is a deep typing session.
    #[serde(default)]
    pub deep_typing: Option<bool>,
}

/// App switch event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSwitchEvent {
    /// App being switched from
    pub from_app_id: Option<String>,
    /// App being switched to
    pub to_app_id: Option<String>,
}

/// A behavioral event with timestamp and type-specific payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: BehaviorEventType,
    /// Scroll event data (present when event_type is Scroll)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll: Option<ScrollEvent>,
    /// Tap event data (present when event_type is Tap)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<TapEvent>,
    /// Swipe event data (present when event_type is Swipe)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swipe: Option<SwipeEvent>,
    /// Interruption event data (present when event_type is Notification or Call)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interruption: Option<InterruptionEvent>,
    /// Typing event data (present when event_type is Typing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typing: Option<TypingEvent>,
    /// App switch event data (present when event_type is AppSwitch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_switch: Option<AppSwitchEvent>,
}

/// A behavioral session containing a collection of events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSession {
    /// Unique session identifier
    pub session_id: String,
    /// Device identifier
    pub device_id: String,
    /// User's timezone
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Session start time
    pub start_time: DateTime<Utc>,
    /// Session end time
    pub end_time: DateTime<Utc>,
    /// Events in the session
    pub events: Vec<BehaviorEvent>,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

/// Idle segment detected during the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleSegment {
    /// Start of idle period
    pub start: DateTime<Utc>,
    /// End of idle period
    pub end: DateTime<Utc>,
    /// Duration in seconds
    pub duration_sec: f64,
}

/// Engagement segment (period of sustained activity without interruptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementSegment {
    /// Start of engagement period
    pub start: DateTime<Utc>,
    /// End of engagement period
    pub end: DateTime<Utc>,
    /// Duration in seconds
    pub duration_sec: f64,
    /// Number of events during this segment
    pub event_count: u32,
}

/// Canonical behavioral signals extracted from a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalBehaviorSignals {
    /// Session identifier
    pub session_id: String,
    /// Device identifier
    pub device_id: String,
    /// Session timezone
    pub timezone: String,
    /// Session start time
    pub start_time: DateTime<Utc>,
    /// Session end time
    pub end_time: DateTime<Utc>,
    /// Session duration in seconds
    pub duration_sec: f64,

    // Event counts by type
    /// Total number of events
    pub total_events: u32,
    /// Number of scroll events
    pub scroll_events: u32,
    /// Number of tap events
    pub tap_events: u32,
    /// Number of swipe events
    pub swipe_events: u32,
    /// Number of notification events
    pub notification_events: u32,
    /// Number of call events
    pub call_events: u32,
    /// Number of typing events
    pub typing_events: u32,
    /// Number of app switch events
    pub app_switch_events: u32,

    // Scroll-specific metrics
    /// Number of scroll direction reversals
    pub scroll_direction_reversals: u32,

    // Typing metrics
    /// Total typing duration in seconds
    pub total_typing_duration_sec: f64,
    /// Per-typing-session metrics if provided by the producer.
    ///
    /// Each typing event can represent one typing session (keyboard open → close).
    #[serde(default)]
    pub typing_sessions: Vec<TypingSessionMetrics>,

    // Idle and engagement analysis
    /// Detected idle segments (gaps > 30s)
    pub idle_segments: Vec<IdleSegment>,
    /// Total idle time in seconds
    pub total_idle_time_sec: f64,
    /// Detected engagement segments
    pub engagement_segments: Vec<EngagementSegment>,

    // Inter-event timing
    /// Inter-event gaps in seconds (for burstiness calculation)
    pub inter_event_gaps: Vec<f64>,

    /// When the canonical signals were computed
    pub computed_at: DateTime<Utc>,
}

/// Typing metrics for a single typing session (keyboard open to close).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TypingSessionMetrics {
    pub start_at: String,
    pub end_at: String,
    /// Duration in seconds.
    pub duration: u32,
    pub deep_typing: bool,
    pub typing_tap_count: u32,
    /// Tap events per second (SDK).
    pub typing_speed: f64,
    pub mean_inter_tap_interval_ms: f64,
    pub typing_cadence_variability: f64,
    pub typing_cadence_stability: f64,
    pub typing_gap_count: u32,
    pub typing_gap_ratio: f64,
    pub typing_burstiness: f64,
    pub typing_activity_ratio: f64,
    pub typing_interaction_intensity: f64,
}

/// Quality flags for behavioral data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorQualityFlag {
    /// Session is shorter than recommended (< 5 minutes)
    ShortSession,
    /// Very few events in session (< 10)
    LowEventCount,
    /// Mostly idle time (> 80%)
    HighIdleRatio,
    /// Only one type of event
    LowEventDiversity,
    /// Session has gaps suggesting device was off
    SessionGaps,
}

/// Normalized behavioral signals with rates per minute and quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedBehaviorSignals {
    /// Source canonical signals
    pub canonical: CanonicalBehaviorSignals,

    // Rates per minute
    /// Events per minute
    pub events_per_min: f64,
    /// Scrolls per minute
    pub scrolls_per_min: f64,
    /// Taps per minute
    pub taps_per_min: f64,
    /// Swipes per minute
    pub swipes_per_min: f64,
    /// Notifications per minute
    pub notifications_per_min: f64,
    /// App switches per minute
    pub app_switches_per_min: f64,

    // Quality metrics
    /// Data coverage (0-1, based on event diversity)
    pub coverage: f64,
    /// Quality flags
    pub quality_flags: Vec<BehaviorQualityFlag>,
}

/// Derived behavioral metrics computed from normalized signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedBehaviorSignals {
    /// Source normalized signals
    pub normalized: NormalizedBehaviorSignals,

    // Core metrics
    /// Task switch rate (0-1, exponential saturation)
    pub task_switch_rate: f64,
    /// Task switch cost normalized to 0-1 (raw ms / 10_000).
    pub task_switch_cost: f64,
    /// Active time ratio (0-1).
    pub active_time_ratio: f64,
    /// Notification load (0-1, exponential saturation)
    pub notification_load: f64,
    /// Idle ratio (total idle time / session duration)
    pub idle_ratio: f64,
    /// Fragmented idle ratio (idle segment count / session duration in seconds)
    pub fragmented_idle_ratio: f64,
    /// Scroll jitter rate (direction reversals / scroll events - 1)
    pub scroll_jitter_rate: f64,
    /// Burstiness of inter-event gaps (Barabási formula, 0-1)
    pub burstiness: f64,
    /// Number of deep focus blocks (engagement >= 120s without interruptions)
    pub deep_focus_blocks: u32,
    /// Interaction intensity ((events + typing_duration/10) / session_duration)
    pub interaction_intensity: f64,

    // Composite scores
    /// Distraction score (weighted combination, 0-1)
    pub distraction_score: f64,
    /// Focus hint (1 - distraction_score, 0-1)
    pub focus_hint: f64,
}

/// Behavioral baselines for relative interpretation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BehaviorBaselines {
    /// Baseline distraction score
    pub distraction_baseline: Option<f64>,
    /// Baseline focus hint
    pub focus_baseline: Option<f64>,
    /// Baseline burstiness
    pub burstiness_baseline: Option<f64>,
    /// Baseline interaction intensity
    pub intensity_baseline: Option<f64>,
    /// Number of sessions in the baseline
    pub sessions_in_baseline: u32,
}

/// Contextual behavioral signals with baseline comparisons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualBehaviorSignals {
    /// Source derived signals
    pub derived: DerivedBehaviorSignals,
    /// Current baselines
    pub baselines: BehaviorBaselines,
    /// Distraction deviation from baseline (percentage)
    pub distraction_deviation_pct: Option<f64>,
    /// Focus deviation from baseline (percentage)
    pub focus_deviation_pct: Option<f64>,
}

// ============================================================================
// HSI 1.0 Compliant Types
// ============================================================================

/// HSI 1.0 axis reading direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HsiDirection {
    HigherIsMore,
    HigherIsLess,
    Bidirectional,
}

/// HSI 1.0 source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HsiSourceType {
    Sensor,
    App,
    SelfReport,
    Observer,
    Derived,
    Other,
}

/// HSI 1.0 consent level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HsiConsent {
    None,
    Implicit,
    Explicit,
}

/// HSI 1.0 producer metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiProducer {
    /// Name of the producing software
    pub name: String,
    /// Version of the producing software
    pub version: String,
    /// Unique instance identifier (UUID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// HSI 1.0 window definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiWindow {
    /// Window start time (RFC3339)
    pub start: String,
    /// Window end time (RFC3339)
    pub end: String,
    /// Optional label for the window
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// HSI 1.0 axis reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiAxisReading {
    /// Axis name (lower_snake_case)
    pub axis: String,
    /// Score value (0-1) or null if unavailable
    pub score: Option<f64>,
    /// Confidence in the score (0-1)
    pub confidence: f64,
    /// Window ID this reading belongs to
    pub window_id: String,
    /// Direction semantics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<HsiDirection>,
    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Source IDs that contributed to this reading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_source_ids: Option<Vec<String>>,
    /// Notes about this reading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// HSI 1.0 axes domain (contains readings array)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiAxesDomain {
    /// Axis readings
    pub readings: Vec<HsiAxisReading>,
}

/// HSI 1.0 axes container
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HsiAxes {
    /// Affect domain readings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affect: Option<HsiAxesDomain>,
    /// Engagement domain readings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engagement: Option<HsiAxesDomain>,
    /// Behavior domain readings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behavior: Option<HsiAxesDomain>,
}

/// HSI 1.0 source definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiSource {
    /// Source type
    #[serde(rename = "type")]
    pub source_type: HsiSourceType,
    /// Quality of the source (0-1)
    pub quality: f64,
    /// Whether the source is degraded
    pub degraded: bool,
    /// Optional notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// HSI 1.0 privacy declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiPrivacy {
    /// Must be false - HSI payloads must not contain PII
    pub contains_pii: bool,
    /// Whether raw biosignals are allowed
    pub raw_biosignals_allowed: bool,
    /// Whether derived metrics are allowed
    pub derived_metrics_allowed: bool,
    /// Whether embeddings are allowed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_allowed: Option<bool>,
    /// Consent level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent: Option<HsiConsent>,
    /// Purposes for data use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purposes: Option<Vec<String>>,
    /// Notes about privacy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl Default for HsiPrivacy {
    fn default() -> Self {
        Self {
            contains_pii: false,
            raw_biosignals_allowed: false,
            derived_metrics_allowed: true,
            embedding_allowed: None,
            consent: None,
            purposes: None,
            notes: None,
        }
    }
}

/// HSI 1.0 compliant payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiPayload {
    /// HSI schema version (must be "1.0")
    pub hsi_version: String,
    /// When the human state was observed (RFC3339)
    pub observed_at_utc: String,
    /// When this payload was computed (RFC3339)
    pub computed_at_utc: String,
    /// Producer metadata
    pub producer: HsiProducer,
    /// Window identifiers
    pub window_ids: Vec<String>,
    /// Window definitions keyed by ID
    pub windows: std::collections::HashMap<String, HsiWindow>,
    /// Source identifiers (required if sources present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ids: Option<Vec<String>>,
    /// Source definitions keyed by ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<std::collections::HashMap<String, HsiSource>>,
    /// Axis readings by domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axes: Option<HsiAxes>,
    /// Privacy declaration
    pub privacy: HsiPrivacy,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_behavior_event_type_serialization() {
        let event_type = BehaviorEventType::AppSwitch;
        let json = serde_json::to_string(&event_type).unwrap();
        assert_eq!(json, "\"app_switch\"");

        let parsed: BehaviorEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, BehaviorEventType::AppSwitch);
    }

    #[test]
    fn test_interruption_action_serialization() {
        let action = InterruptionAction::Dismissed;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"dismissed\"");
    }

    #[test]
    fn test_behavior_session_deserialization() {
        let json = r#"{
            "session_id": "test-session",
            "device_id": "device-123",
            "timezone": "America/New_York",
            "start_time": "2024-01-15T14:00:00Z",
            "end_time": "2024-01-15T14:30:00Z",
            "events": []
        }"#;

        let session: BehaviorSession = serde_json::from_str(json).unwrap();
        assert_eq!(session.session_id, "test-session");
        assert_eq!(session.device_id, "device-123");
        assert_eq!(session.timezone, "America/New_York");
        assert!(session.events.is_empty());
    }

    #[test]
    fn test_behavior_event_with_payload() {
        let json = r#"{
            "timestamp": "2024-01-15T14:05:00Z",
            "event_type": "scroll",
            "scroll": {
                "velocity": 150.5,
                "direction": "down",
                "direction_reversal": false
            }
        }"#;

        let event: BehaviorEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, BehaviorEventType::Scroll);
        assert!(event.scroll.is_some());
        let scroll = event.scroll.unwrap();
        assert_eq!(scroll.velocity, Some(150.5));
        assert_eq!(scroll.direction, Some(ScrollDirection::Down));
        assert!(!scroll.direction_reversal);
    }
}
