//! Behavioral pipeline orchestration
//!
//! This module provides the public API for behavioral metrics processing.
//! It orchestrates the full pipeline from behavioral session JSON to HSI output.

use crate::behavior::adapter::{parse_session, session_to_canonical};
use crate::behavior::baseline::BehaviorBaselineStore;
use crate::behavior::encoder::HsiBehaviorEncoder;
use crate::behavior::features::BehaviorFeatureDeriver;
use crate::behavior::normalizer::BehaviorNormalizer;
use crate::error::ComputeError;

/// Convert behavioral session JSON to HSI-compliant JSON (stateless, one-shot).
///
/// # Arguments
/// * `session_json` - Raw behavioral session JSON
///
/// # Returns
/// HSI JSON payload string
///
/// # Example
/// ```ignore
/// let hsi_json = behavior_to_hsi(session_json)?;
/// ```
pub fn behavior_to_hsi(session_json: String) -> Result<String, ComputeError> {
    // Stage 1: Parse session JSON
    let session = parse_session(&session_json)?;

    // Stage 2: Convert to canonical signals
    let canonical = session_to_canonical(&session)?;

    // Stage 3: Normalize signals
    let normalized = BehaviorNormalizer::normalize(canonical);

    // Stage 4: Derive features
    let derived = BehaviorFeatureDeriver::derive(normalized);

    // Stage 5: Apply baselines (fresh baseline store for stateless call)
    let mut baseline_store = BehaviorBaselineStore::default();
    let contextual = baseline_store.update_and_contextualize(derived);

    // Stage 6: Encode to HSI JSON
    let encoder = HsiBehaviorEncoder::new();
    encoder.encode_to_json(&contextual)
}

/// Stateful processor for incremental processing with persistent baselines.
///
/// Use this when you need to maintain baselines across multiple sessions.
pub struct BehaviorProcessor {
    baseline_store: BehaviorBaselineStore,
    encoder: HsiBehaviorEncoder,
}

impl Default for BehaviorProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl BehaviorProcessor {
    /// Create a new processor with default settings (20 session baseline window)
    pub fn new() -> Self {
        Self {
            baseline_store: BehaviorBaselineStore::default(),
            encoder: HsiBehaviorEncoder::new(),
        }
    }

    /// Create a processor with a specific baseline window size (number of sessions)
    pub fn with_baseline_window(sessions: usize) -> Self {
        Self {
            baseline_store: BehaviorBaselineStore::new(sessions),
            encoder: HsiBehaviorEncoder::new(),
        }
    }

    /// Process a behavioral session and return HSI JSON
    ///
    /// # Arguments
    /// * `session_json` - Raw behavioral session JSON
    ///
    /// # Returns
    /// HSI JSON payload string
    pub fn process(&mut self, session_json: &str) -> Result<String, ComputeError> {
        // Stage 1: Parse session JSON
        let session = parse_session(session_json)?;

        // Stage 2: Convert to canonical signals
        let canonical = session_to_canonical(&session)?;

        // Stage 3: Normalize signals
        let normalized = BehaviorNormalizer::normalize(canonical);

        // Stage 4: Derive features
        let derived = BehaviorFeatureDeriver::derive(normalized);

        // Stage 5: Apply baselines
        let contextual = self.baseline_store.update_and_contextualize(derived);

        // Stage 6: Encode to HSI JSON
        self.encoder.encode_to_json(&contextual)
    }

    /// Save baseline state to JSON for persistence
    pub fn save_baselines(&self) -> Result<String, ComputeError> {
        self.baseline_store
            .to_json()
            .map_err(|e| ComputeError::EncodingError(e.to_string()))
    }

    /// Load baseline state from JSON
    pub fn load_baselines(&mut self, json: &str) -> Result<(), ComputeError> {
        self.baseline_store = BehaviorBaselineStore::from_json(json)
            .map_err(|e| ComputeError::ParseError(e.to_string()))?;
        Ok(())
    }

    /// Get the number of sessions currently in the baseline
    pub fn baseline_session_count(&self) -> usize {
        self.baseline_store.session_count()
    }

    /// Clear all baseline data
    pub fn clear_baselines(&mut self) {
        self.baseline_store.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_behavior_session_json() -> &'static str {
        r#"{
            "session_id": "sess-123-abc",
            "device_id": "device-456",
            "timezone": "America/New_York",
            "start_time": "2024-01-15T14:00:00Z",
            "end_time": "2024-01-15T14:30:00Z",
            "events": [
                {
                    "timestamp": "2024-01-15T14:01:00Z",
                    "event_type": "scroll",
                    "scroll": {
                        "velocity": 150.5,
                        "direction": "down",
                        "direction_reversal": false
                    }
                },
                {
                    "timestamp": "2024-01-15T14:01:30Z",
                    "event_type": "scroll",
                    "scroll": {
                        "velocity": 120.0,
                        "direction": "up",
                        "direction_reversal": true
                    }
                },
                {
                    "timestamp": "2024-01-15T14:02:00Z",
                    "event_type": "tap",
                    "tap": {
                        "tap_duration_ms": 120,
                        "long_press": false
                    }
                },
                {
                    "timestamp": "2024-01-15T14:03:00Z",
                    "event_type": "notification",
                    "interruption": {
                        "action": "ignored"
                    }
                },
                {
                    "timestamp": "2024-01-15T14:05:00Z",
                    "event_type": "app_switch",
                    "app_switch": {
                        "from_app_id": "com.app.one",
                        "to_app_id": "com.app.two"
                    }
                },
                {
                    "timestamp": "2024-01-15T14:10:00Z",
                    "event_type": "typing",
                    "typing": {
                        "typing_speed_cpm": 180.5,
                        "cadence_stability": 0.85,
                        "duration_sec": 45.0
                    }
                },
                {
                    "timestamp": "2024-01-15T14:15:00Z",
                    "event_type": "scroll",
                    "scroll": {
                        "velocity": 200.0,
                        "direction": "down",
                        "direction_reversal": false
                    }
                },
                {
                    "timestamp": "2024-01-15T14:20:00Z",
                    "event_type": "tap",
                    "tap": {
                        "tap_duration_ms": 80,
                        "long_press": false
                    }
                },
                {
                    "timestamp": "2024-01-15T14:25:00Z",
                    "event_type": "notification",
                    "interruption": {
                        "action": "opened"
                    }
                },
                {
                    "timestamp": "2024-01-15T14:28:00Z",
                    "event_type": "swipe",
                    "swipe": {
                        "direction": "left",
                        "velocity": 300.0
                    }
                }
            ]
        }"#
    }

    #[test]
    fn test_behavior_to_hsi_stateless() {
        let result = behavior_to_hsi(sample_behavior_session_json().to_string());

        assert!(result.is_ok());
        let json = result.unwrap();

        // Verify JSON is valid and contains expected fields
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(payload["hsi_version"], "1.0.0");
        assert_eq!(payload["producer"]["name"], "synheart-flux");

        // Verify behavior window
        let window = &payload["behavior_windows"][0];
        assert_eq!(window["session_id"], "sess-123-abc");
        assert_eq!(window["duration_sec"], 1800.0);

        // Verify behavior metrics exist
        assert!(window["behavior"]["distraction_score"].is_number());
        assert!(window["behavior"]["focus_hint"].is_number());
        assert!(window["behavior"]["task_switch_rate"].is_number());
        assert!(window["behavior"]["burstiness"].is_number());

        // Verify event summary
        assert_eq!(window["event_summary"]["total_events"], 10);
        assert_eq!(window["event_summary"]["scroll_events"], 3);
        assert_eq!(window["event_summary"]["tap_events"], 2);
        assert_eq!(window["event_summary"]["notifications"], 2);
        assert_eq!(window["event_summary"]["app_switches"], 1);
    }

    #[test]
    fn test_behavior_processor_stateful() {
        let mut processor = BehaviorProcessor::new();

        // Process first session
        let result1 = processor.process(sample_behavior_session_json());
        assert!(result1.is_ok());
        assert_eq!(processor.baseline_session_count(), 1);

        // Process same session again - baseline should update
        let result2 = processor.process(sample_behavior_session_json());
        assert!(result2.is_ok());
        assert_eq!(processor.baseline_session_count(), 2);

        // Second result should have baseline data
        let payload: serde_json::Value = serde_json::from_str(&result2.unwrap()).unwrap();
        let baseline = &payload["behavior_windows"][0]["baseline"];

        // After 2 sessions, baseline should be established
        assert!(baseline["distraction"].is_number());
        assert_eq!(baseline["sessions_in_baseline"], 2);

        // Deviation should be present
        assert!(baseline["distraction_deviation_pct"].is_number());
    }

    #[test]
    fn test_behavior_processor_custom_window() {
        let mut processor = BehaviorProcessor::with_baseline_window(5);

        // Process 7 sessions - only last 5 should be in baseline
        for _ in 0..7 {
            processor.process(sample_behavior_session_json()).unwrap();
        }

        assert_eq!(processor.baseline_session_count(), 5);
    }

    #[test]
    fn test_baseline_serialization() {
        let mut processor = BehaviorProcessor::new();

        // Process a session
        processor.process(sample_behavior_session_json()).unwrap();

        // Save baselines
        let saved = processor.save_baselines().unwrap();

        // Create new processor and load baselines
        let mut new_processor = BehaviorProcessor::new();
        new_processor.load_baselines(&saved).unwrap();

        // Should have 1 session in baseline
        assert_eq!(new_processor.baseline_session_count(), 1);

        // Process another session - should have 2 in baseline
        new_processor.process(sample_behavior_session_json()).unwrap();
        assert_eq!(new_processor.baseline_session_count(), 2);
    }

    #[test]
    fn test_clear_baselines() {
        let mut processor = BehaviorProcessor::new();

        for _ in 0..5 {
            processor.process(sample_behavior_session_json()).unwrap();
        }

        assert_eq!(processor.baseline_session_count(), 5);

        processor.clear_baselines();
        assert_eq!(processor.baseline_session_count(), 0);
    }

    #[test]
    fn test_invalid_json() {
        let result = behavior_to_hsi("not valid json".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_events_session() {
        let json = r#"{
            "session_id": "empty-session",
            "device_id": "device",
            "timezone": "UTC",
            "start_time": "2024-01-15T14:00:00Z",
            "end_time": "2024-01-15T14:30:00Z",
            "events": []
        }"#;

        let result = behavior_to_hsi(json.to_string());
        assert!(result.is_ok());

        let payload: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();

        // Should still produce valid output with zero events
        let window = &payload["behavior_windows"][0];
        assert_eq!(window["event_summary"]["total_events"], 0);

        // Quality flags should indicate low event count
        let flags = &payload["quality"]["flags"];
        assert!(flags.as_array().unwrap().iter().any(|f| f == "loweventcount"));
    }

    #[test]
    fn test_distraction_and_focus_inverse() {
        let result = behavior_to_hsi(sample_behavior_session_json().to_string()).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&result).unwrap();

        let behavior = &payload["behavior_windows"][0]["behavior"];
        let distraction = behavior["distraction_score"].as_f64().unwrap();
        let focus = behavior["focus_hint"].as_f64().unwrap();

        // Focus should be 1 - distraction
        assert!((distraction + focus - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_scroll_jitter_calculation() {
        let result = behavior_to_hsi(sample_behavior_session_json().to_string()).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&result).unwrap();

        let behavior = &payload["behavior_windows"][0]["behavior"];
        let scroll_jitter = behavior["scroll_jitter_rate"].as_f64().unwrap();

        // We have 3 scroll events, 1 reversal
        // jitter = 1 / (3 - 1) = 0.5
        assert!((scroll_jitter - 0.5).abs() < 0.001);
    }
}
