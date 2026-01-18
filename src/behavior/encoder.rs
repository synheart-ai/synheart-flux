//! HSI behavioral encoder
//!
//! Encodes contextual behavioral signals into HSI-compliant JSON payloads.

use crate::behavior::types::{
    ContextualBehaviorSignals, HsiBehavior, HsiBehaviorBaseline, HsiBehaviorPayload,
    HsiBehaviorProducer, HsiBehaviorProvenance, HsiBehaviorQuality, HsiBehaviorWindow,
    HsiEventSummary,
};
use crate::error::ComputeError;
use crate::{FLUX_VERSION, PRODUCER_NAME};
use chrono::Utc;
use uuid::Uuid;

/// Current HSI schema version
pub const HSI_VERSION: &str = "1.0.0";

/// HSI behavioral encoder
pub struct HsiBehaviorEncoder {
    instance_id: String,
}

impl Default for HsiBehaviorEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl HsiBehaviorEncoder {
    /// Create a new encoder with a unique instance ID
    pub fn new() -> Self {
        Self {
            instance_id: Uuid::new_v4().to_string(),
        }
    }

    /// Create an encoder with a specific instance ID
    pub fn with_instance_id(instance_id: String) -> Self {
        Self { instance_id }
    }

    /// Encode contextual behavioral signals into an HSI payload
    pub fn encode(
        &self,
        signals: &ContextualBehaviorSignals,
    ) -> Result<HsiBehaviorPayload, ComputeError> {
        let canonical = &signals.derived.normalized.canonical;
        let computed_at = Utc::now();

        // Build producer metadata
        let producer = HsiBehaviorProducer {
            name: PRODUCER_NAME.to_string(),
            version: FLUX_VERSION.to_string(),
            instance_id: self.instance_id.clone(),
        };

        // Build provenance
        let provenance = HsiBehaviorProvenance {
            source_device_id: canonical.device_id.clone(),
            observed_at_utc: canonical.start_time.to_rfc3339(),
            computed_at_utc: computed_at.to_rfc3339(),
        };

        // Build quality metrics
        let quality = self.build_quality(signals);

        // Build behavioral window
        let window = self.build_behavior_window(signals);

        Ok(HsiBehaviorPayload {
            hsi_version: HSI_VERSION.to_string(),
            producer,
            provenance,
            quality,
            behavior_windows: vec![window],
        })
    }

    /// Encode to JSON string
    pub fn encode_to_json(
        &self,
        signals: &ContextualBehaviorSignals,
    ) -> Result<String, ComputeError> {
        let payload = self.encode(signals)?;
        serde_json::to_string_pretty(&payload).map_err(ComputeError::JsonError)
    }

    fn build_quality(&self, signals: &ContextualBehaviorSignals) -> HsiBehaviorQuality {
        let normalized = &signals.derived.normalized;

        // Calculate confidence based on coverage and baseline availability
        let base_confidence = normalized.coverage;
        let baseline_bonus = if signals.baselines.sessions_in_baseline >= 5 {
            0.1
        } else {
            0.0
        };
        let confidence = (base_confidence + baseline_bonus).min(1.0);

        let flags: Vec<String> = normalized
            .quality_flags
            .iter()
            .map(|f| format!("{f:?}").to_lowercase())
            .collect();

        HsiBehaviorQuality {
            coverage: normalized.coverage,
            confidence,
            flags,
        }
    }

    fn build_behavior_window(&self, signals: &ContextualBehaviorSignals) -> HsiBehaviorWindow {
        let canonical = &signals.derived.normalized.canonical;
        let derived = &signals.derived;

        // Build behavior namespace
        let behavior = HsiBehavior {
            distraction_score: derived.distraction_score,
            focus_hint: derived.focus_hint,
            task_switch_rate: derived.task_switch_rate,
            notification_load: derived.notification_load,
            burstiness: derived.burstiness,
            scroll_jitter_rate: derived.scroll_jitter_rate,
            interaction_intensity: derived.interaction_intensity,
            deep_focus_blocks: derived.deep_focus_blocks,
        };

        // Build baseline namespace
        let baseline = HsiBehaviorBaseline {
            distraction: signals.baselines.distraction_baseline,
            focus: signals.baselines.focus_baseline,
            distraction_deviation_pct: signals.distraction_deviation_pct,
            sessions_in_baseline: signals.baselines.sessions_in_baseline,
        };

        // Build event summary
        let event_summary = HsiEventSummary {
            total_events: canonical.total_events,
            scroll_events: canonical.scroll_events,
            tap_events: canonical.tap_events,
            app_switches: canonical.app_switch_events,
            notifications: canonical.notification_events,
        };

        HsiBehaviorWindow {
            session_id: canonical.session_id.clone(),
            start_time_utc: canonical.start_time.to_rfc3339(),
            end_time_utc: canonical.end_time.to_rfc3339(),
            duration_sec: canonical.duration_sec,
            behavior,
            baseline,
            event_summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::types::{
        BehaviorBaselines, BehaviorQualityFlag, CanonicalBehaviorSignals, DerivedBehaviorSignals,
        NormalizedBehaviorSignals,
    };
    use chrono::{TimeZone, Utc};

    fn make_test_contextual() -> ContextualBehaviorSignals {
        let canonical = CanonicalBehaviorSignals {
            session_id: "test-session-123".to_string(),
            device_id: "test-device".to_string(),
            timezone: "America/New_York".to_string(),
            start_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
            end_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap(),
            duration_sec: 1800.0,
            total_events: 245,
            scroll_events: 120,
            tap_events: 85,
            swipe_events: 15,
            notification_events: 12,
            call_events: 1,
            typing_events: 4,
            app_switch_events: 8,
            scroll_direction_reversals: 15,
            total_typing_duration_sec: 90.0,
            idle_segments: vec![],
            total_idle_time_sec: 60.0,
            engagement_segments: vec![],
            inter_event_gaps: vec![5.0, 8.0, 12.0, 6.0],
            computed_at: Utc::now(),
        };

        let normalized = NormalizedBehaviorSignals {
            canonical,
            events_per_min: 8.17,
            scrolls_per_min: 4.0,
            taps_per_min: 2.83,
            swipes_per_min: 0.5,
            notifications_per_min: 0.4,
            app_switches_per_min: 0.27,
            coverage: 0.95,
            quality_flags: vec![],
        };

        let derived = DerivedBehaviorSignals {
            normalized,
            task_switch_rate: 0.42,
            notification_load: 0.28,
            idle_ratio: 0.033,
            fragmented_idle_ratio: 0.0,
            scroll_jitter_rate: 0.12,
            burstiness: 0.55,
            deep_focus_blocks: 2,
            interaction_intensity: 0.78,
            distraction_score: 0.35,
            focus_hint: 0.65,
        };

        let baselines = BehaviorBaselines {
            distraction_baseline: Some(0.38),
            focus_baseline: Some(0.62),
            burstiness_baseline: Some(0.50),
            intensity_baseline: Some(0.70),
            sessions_in_baseline: 15,
        };

        ContextualBehaviorSignals {
            derived,
            baselines,
            distraction_deviation_pct: Some(-7.9),
            focus_deviation_pct: Some(4.8),
        }
    }

    #[test]
    fn test_encode_hsi_behavior_payload() {
        let signals = make_test_contextual();
        let encoder = HsiBehaviorEncoder::with_instance_id("test-instance".to_string());
        let payload = encoder.encode(&signals).unwrap();

        assert_eq!(payload.hsi_version, HSI_VERSION);
        assert_eq!(payload.producer.name, PRODUCER_NAME);
        assert_eq!(payload.producer.version, FLUX_VERSION);
        assert_eq!(payload.producer.instance_id, "test-instance");

        assert_eq!(payload.provenance.source_device_id, "test-device");

        assert!(payload.quality.coverage > 0.9);
        assert!(payload.quality.confidence > 0.9);
        assert!(payload.quality.flags.is_empty());

        assert_eq!(payload.behavior_windows.len(), 1);
        let window = &payload.behavior_windows[0];
        assert_eq!(window.session_id, "test-session-123");
        assert_eq!(window.duration_sec, 1800.0);

        // Check behavior metrics
        assert!((window.behavior.distraction_score - 0.35).abs() < 0.001);
        assert!((window.behavior.focus_hint - 0.65).abs() < 0.001);
        assert!((window.behavior.task_switch_rate - 0.42).abs() < 0.001);
        assert!((window.behavior.notification_load - 0.28).abs() < 0.001);
        assert_eq!(window.behavior.deep_focus_blocks, 2);

        // Check baseline
        assert_eq!(window.baseline.distraction, Some(0.38));
        assert_eq!(window.baseline.focus, Some(0.62));
        assert!((window.baseline.distraction_deviation_pct.unwrap() - (-7.9)).abs() < 0.1);
        assert_eq!(window.baseline.sessions_in_baseline, 15);

        // Check event summary
        assert_eq!(window.event_summary.total_events, 245);
        assert_eq!(window.event_summary.scroll_events, 120);
        assert_eq!(window.event_summary.tap_events, 85);
        assert_eq!(window.event_summary.app_switches, 8);
        assert_eq!(window.event_summary.notifications, 12);
    }

    #[test]
    fn test_encode_to_json() {
        let signals = make_test_contextual();
        let encoder = HsiBehaviorEncoder::new();
        let json = encoder.encode_to_json(&signals).unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("hsi_version").is_some());
        assert!(parsed.get("producer").is_some());
        assert!(parsed.get("provenance").is_some());
        assert!(parsed.get("quality").is_some());
        assert!(parsed.get("behavior_windows").is_some());

        // Check specific values
        assert_eq!(parsed["hsi_version"], "1.0.0");
        assert_eq!(parsed["producer"]["name"], "synheart-flux");
    }

    #[test]
    fn test_confidence_with_no_baseline() {
        let mut signals = make_test_contextual();
        signals.baselines.sessions_in_baseline = 2; // Less than 5

        let encoder = HsiBehaviorEncoder::new();
        let payload = encoder.encode(&signals).unwrap();

        // Without enough baseline sessions, no bonus
        // Confidence should equal coverage
        assert!((payload.quality.confidence - payload.quality.coverage).abs() < 0.001);
    }

    #[test]
    fn test_quality_flags_in_output() {
        let mut signals = make_test_contextual();
        signals.derived.normalized.quality_flags =
            vec![BehaviorQualityFlag::ShortSession, BehaviorQualityFlag::LowEventCount];

        let encoder = HsiBehaviorEncoder::new();
        let payload = encoder.encode(&signals).unwrap();

        assert_eq!(payload.quality.flags.len(), 2);
        assert!(payload.quality.flags.contains(&"shortsession".to_string()));
        assert!(payload.quality.flags.contains(&"loweventcount".to_string()));
    }

    #[test]
    fn test_unique_instance_ids() {
        let encoder1 = HsiBehaviorEncoder::new();
        let encoder2 = HsiBehaviorEncoder::new();

        let signals = make_test_contextual();
        let payload1 = encoder1.encode(&signals).unwrap();
        let payload2 = encoder2.encode(&signals).unwrap();

        // Different encoders should have different instance IDs
        assert_ne!(payload1.producer.instance_id, payload2.producer.instance_id);
    }
}
