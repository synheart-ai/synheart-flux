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
    /// Typing speed in characters per minute
    pub typing_speed_cpm: Option<f64>,
    /// Cadence stability (0-1, higher = more consistent rhythm)
    pub cadence_stability: Option<f64>,
    /// Duration of typing session in seconds
    pub duration_sec: Option<f64>,
    /// Number of pauses during typing
    pub pause_count: Option<u32>,
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

/// HSI behavioral namespace signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehavior {
    /// Distraction score (0-1)
    pub distraction_score: f64,
    /// Focus hint (0-1)
    pub focus_hint: f64,
    /// Task switch rate (0-1)
    pub task_switch_rate: f64,
    /// Notification load (0-1)
    pub notification_load: f64,
    /// Burstiness (0-1)
    pub burstiness: f64,
    /// Scroll jitter rate (0-1)
    pub scroll_jitter_rate: f64,
    /// Interaction intensity
    pub interaction_intensity: f64,
    /// Number of deep focus blocks
    pub deep_focus_blocks: u32,
}

/// HSI behavioral baseline namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehaviorBaseline {
    /// Baseline distraction score
    pub distraction: Option<f64>,
    /// Baseline focus hint
    pub focus: Option<f64>,
    /// Distraction deviation from baseline (percentage)
    pub distraction_deviation_pct: Option<f64>,
    /// Number of sessions in baseline
    pub sessions_in_baseline: u32,
}

/// Event summary for HSI output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiEventSummary {
    /// Total number of events
    pub total_events: u32,
    /// Number of scroll events
    pub scroll_events: u32,
    /// Number of tap events
    pub tap_events: u32,
    /// Number of app switches
    pub app_switches: u32,
    /// Number of notifications
    pub notifications: u32,
}

/// HSI behavioral window (one session)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehaviorWindow {
    /// Session identifier
    pub session_id: String,
    /// Session start time (UTC)
    pub start_time_utc: String,
    /// Session end time (UTC)
    pub end_time_utc: String,
    /// Session duration in seconds
    pub duration_sec: f64,
    /// Behavioral metrics
    pub behavior: HsiBehavior,
    /// Baseline information
    pub baseline: HsiBehaviorBaseline,
    /// Event summary
    pub event_summary: HsiEventSummary,
}

/// HSI producer metadata (same as wearable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehaviorProducer {
    pub name: String,
    pub version: String,
    pub instance_id: String,
}

/// HSI provenance for behavioral data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehaviorProvenance {
    /// Source device identifier
    pub source_device_id: String,
    /// When the session was observed
    pub observed_at_utc: String,
    /// When the HSI was computed
    pub computed_at_utc: String,
}

/// HSI quality metrics for behavioral data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehaviorQuality {
    /// Data coverage (0-1)
    pub coverage: f64,
    /// Confidence in the signals (0-1)
    pub confidence: f64,
    /// Quality flags
    pub flags: Vec<String>,
}

/// Complete HSI behavioral payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBehaviorPayload {
    /// HSI schema version
    pub hsi_version: String,
    /// Producer metadata
    pub producer: HsiBehaviorProducer,
    /// Provenance information
    pub provenance: HsiBehaviorProvenance,
    /// Quality metrics
    pub quality: HsiBehaviorQuality,
    /// Behavioral windows (one per session)
    pub behavior_windows: Vec<HsiBehaviorWindow>,
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
