//! HSI 1.0 behavioral encoder
//!
//! Encodes contextual behavioral signals into HSI 1.0 compliant JSON payloads.

use crate::behavior::types::{
    ContextualBehaviorSignals, HsiAxes, HsiAxesDomain, HsiAxisReading, HsiDirection, HsiPayload,
    HsiPrivacy, HsiProducer, HsiSource, HsiSourceType, HsiWindow,
};
use crate::error::ComputeError;
use crate::{FLUX_VERSION, PRODUCER_NAME};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

/// HSI schema version
pub const HSI_VERSION: &str = "1.0";

/// HSI 1.0 behavioral encoder
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

    /// Encode contextual behavioral signals into an HSI 1.0 compliant payload
    pub fn encode(&self, signals: &ContextualBehaviorSignals) -> Result<HsiPayload, ComputeError> {
        let canonical = &signals.derived.normalized.canonical;
        let derived = &signals.derived;
        let computed_at = Utc::now();

        // Generate window ID
        let window_id = format!("w_{}", canonical.session_id.replace('-', "_"));

        // Build producer
        let producer = HsiProducer {
            name: PRODUCER_NAME.to_string(),
            version: FLUX_VERSION.to_string(),
            instance_id: Some(self.instance_id.clone()),
        };

        // Build window
        let mut windows = HashMap::new();
        windows.insert(
            window_id.clone(),
            HsiWindow {
                start: canonical.start_time.to_rfc3339(),
                end: canonical.end_time.to_rfc3339(),
                label: Some(format!("session:{}", canonical.session_id)),
            },
        );

        // Build source
        let source_id = format!("s_{}", canonical.device_id.replace('-', "_"));
        let mut sources = HashMap::new();
        sources.insert(
            source_id.clone(),
            HsiSource {
                source_type: HsiSourceType::App,
                quality: signals.derived.normalized.coverage,
                degraded: !signals.derived.normalized.quality_flags.is_empty(),
                notes: if !signals.derived.normalized.quality_flags.is_empty() {
                    Some(format!(
                        "Quality flags: {:?}",
                        signals.derived.normalized.quality_flags
                    ))
                } else {
                    None
                },
            },
        );

        // Calculate confidence based on coverage and baseline
        let base_confidence = signals.derived.normalized.coverage;
        let baseline_bonus = if signals.baselines.sessions_in_baseline >= 5 {
            0.1
        } else {
            0.0
        };
        let confidence = (base_confidence + baseline_bonus).min(1.0);

        // Build behavioral axis readings
        let behavior_readings = vec![
            // Distraction score
            HsiAxisReading {
                axis: "distraction".to_string(),
                score: Some(derived.distraction_score),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: None,
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Focus hint (inverse of distraction)
            HsiAxisReading {
                axis: "focus".to_string(),
                score: Some(derived.focus_hint),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: None,
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Task switch rate
            HsiAxisReading {
                axis: "task_switch_rate".to_string(),
                score: Some(derived.task_switch_rate),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("normalized".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: Some("Exponential saturation of app switches per minute".to_string()),
            },
            // Notification load
            HsiAxisReading {
                axis: "notification_load".to_string(),
                score: Some(derived.notification_load),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("normalized".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Burstiness
            HsiAxisReading {
                axis: "burstiness".to_string(),
                score: Some(derived.burstiness),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::Bidirectional),
                unit: Some("barabasi_index".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: Some("Barabási formula on inter-event gaps".to_string()),
            },
            // Scroll jitter rate
            HsiAxisReading {
                axis: "scroll_jitter_rate".to_string(),
                score: Some(derived.scroll_jitter_rate),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("ratio".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Interaction intensity (clamped to 0-1)
            HsiAxisReading {
                axis: "interaction_intensity".to_string(),
                score: Some(derived.interaction_intensity.min(1.0)),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("normalized".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Idle ratio
            HsiAxisReading {
                axis: "idle_ratio".to_string(),
                score: Some(derived.idle_ratio),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("ratio".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Fragmented idle ratio (raw ratio; clamped for HSI score expectations)
            HsiAxisReading {
                axis: "fragmented_idle_ratio".to_string(),
                score: Some(derived.fragmented_idle_ratio.min(1.0)),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("segments_per_second".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: None,
            },
            // Task switch cost (normalized 0-1 where 1.0 = 10s average per switch)
            HsiAxisReading {
                axis: "task_switch_cost".to_string(),
                score: Some(derived.task_switch_cost.clamp(0.0, 1.0)),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("ratio_of_10s".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: Some("Average time per app switch, normalized by 10s cap".to_string()),
            },
            // Active time ratio
            HsiAxisReading {
                axis: "active_time_ratio".to_string(),
                score: Some(derived.active_time_ratio.clamp(0.0, 1.0)),
                confidence,
                window_id: window_id.clone(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("ratio".to_string()),
                evidence_source_ids: Some(vec![source_id.clone()]),
                notes: Some("1 - idle_time_ratio - task_switch_cost contribution".to_string()),
            },
        ];

        // Build axes
        let axes = HsiAxes {
            affect: None,
            engagement: None,
            behavior: Some(HsiAxesDomain {
                readings: behavior_readings,
            }),
            context: None,
        };

        // Build privacy
        let privacy = HsiPrivacy {
            contains_pii: false,
            raw_biosignals_allowed: false,
            derived_metrics_allowed: true,
            embedding_allowed: None,
            consent: None,
            purposes: Some(vec!["behavioral_research".to_string()]),
            notes: None,
        };

        // Build metadata with baseline and event summary info
        let mut meta = HashMap::new();
        meta.insert(
            "session_id".to_string(),
            serde_json::Value::String(canonical.session_id.clone()),
        );
        meta.insert(
            "duration_sec".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(canonical.duration_sec).unwrap(),
            ),
        );
        meta.insert(
            "total_events".to_string(),
            serde_json::Value::Number(serde_json::Number::from(canonical.total_events)),
        );
        meta.insert(
            "deep_focus_blocks".to_string(),
            serde_json::Value::Number(serde_json::Number::from(derived.deep_focus_blocks)),
        );
        // Deep focus blocks detail (SDK expects detailed blocks)
        let deep_focus_detail: Vec<serde_json::Value> = canonical
            .engagement_segments
            .iter()
            .filter(|s| s.duration_sec >= 120.0)
            .map(|s| {
                serde_json::json!({
                    "start_at": s.start.to_rfc3339(),
                    "end_at": s.end.to_rfc3339(),
                    "duration_ms": (s.duration_sec * 1000.0).round() as u64
                })
            })
            .collect();
        meta.insert(
            "deep_focus_blocks_detail".to_string(),
            serde_json::Value::Array(deep_focus_detail),
        );

        // Typing session summary (SDK-compatible keys)
        let typing_sessions = &canonical.typing_sessions;
        if typing_sessions.is_empty() {
            meta.insert(
                "typing_session_count".to_string(),
                serde_json::Value::from(0),
            );
            meta.insert(
                "average_keystrokes_per_session".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "average_typing_session_duration".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "average_typing_speed".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "average_typing_gap".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "average_inter_tap_interval".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "typing_cadence_stability".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "burstiness_of_typing".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "total_typing_duration".to_string(),
                serde_json::Value::from(0),
            );
            meta.insert(
                "active_typing_ratio".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "typing_contribution_to_interaction_intensity".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert("deep_typing_blocks".to_string(), serde_json::Value::from(0));
            meta.insert(
                "typing_fragmentation".to_string(),
                serde_json::Value::from(0.0),
            );
            meta.insert(
                "typing_metrics".to_string(),
                serde_json::Value::Array(vec![]),
            );
        } else {
            let n = typing_sessions.len() as f64;
            let sum_taps: f64 = typing_sessions
                .iter()
                .map(|s| s.typing_tap_count as f64)
                .sum();
            let sum_dur: f64 = typing_sessions.iter().map(|s| s.duration as f64).sum();
            let sum_speed: f64 = typing_sessions.iter().map(|s| s.typing_speed).sum();
            let sum_mean_iti: f64 = typing_sessions
                .iter()
                .map(|s| s.mean_inter_tap_interval_ms)
                .sum();
            let sum_stability: f64 = typing_sessions
                .iter()
                .map(|s| s.typing_cadence_stability)
                .sum();
            let sum_burst: f64 = typing_sessions.iter().map(|s| s.typing_burstiness).sum();
            let sum_gap_ratio: f64 = typing_sessions.iter().map(|s| s.typing_gap_ratio).sum();
            let deep_typing_blocks: u32 =
                typing_sessions.iter().filter(|s| s.deep_typing).count() as u32;

            meta.insert(
                "typing_session_count".to_string(),
                serde_json::Value::from(typing_sessions.len() as u32),
            );
            meta.insert(
                "average_keystrokes_per_session".to_string(),
                serde_json::Value::from(sum_taps / n),
            );
            meta.insert(
                "average_typing_session_duration".to_string(),
                serde_json::Value::from(sum_dur / n),
            );
            meta.insert(
                "average_typing_speed".to_string(),
                serde_json::Value::from(sum_speed / n),
            );
            // SDK uses mean_inter_tap_interval_ms as "typing gap" and "inter tap interval"
            meta.insert(
                "average_typing_gap".to_string(),
                serde_json::Value::from(sum_mean_iti / n),
            );
            meta.insert(
                "average_inter_tap_interval".to_string(),
                serde_json::Value::from(sum_mean_iti / n),
            );
            meta.insert(
                "typing_cadence_stability".to_string(),
                serde_json::Value::from(sum_stability / n),
            );
            meta.insert(
                "burstiness_of_typing".to_string(),
                serde_json::Value::from(sum_burst / n),
            );
            meta.insert(
                "total_typing_duration".to_string(),
                serde_json::Value::from(sum_dur.round() as u32),
            );
            // Ratio = (totalTypingDuration * 1000) / durationMs
            let duration_ms = (canonical.duration_sec * 1000.0).round().max(0.0);
            let active_typing_ratio = if duration_ms > 0.0 {
                ((sum_dur * 1000.0) / duration_ms).clamp(0.0, 1.0)
            } else {
                0.0
            };
            meta.insert(
                "active_typing_ratio".to_string(),
                serde_json::Value::from(active_typing_ratio),
            );
            let typing_contribution = if canonical.total_events > 0 {
                (canonical.typing_events as f64) / (canonical.total_events as f64)
            } else {
                0.0
            };
            meta.insert(
                "typing_contribution_to_interaction_intensity".to_string(),
                serde_json::Value::from(typing_contribution),
            );
            meta.insert(
                "deep_typing_blocks".to_string(),
                serde_json::Value::from(deep_typing_blocks),
            );
            meta.insert(
                "typing_fragmentation".to_string(),
                serde_json::Value::from(sum_gap_ratio / n),
            );
            let metrics: Vec<serde_json::Value> = typing_sessions
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "start_at": s.start_at,
                        "end_at": s.end_at,
                        "duration": s.duration,
                        "deep_typing": s.deep_typing,
                        "typing_tap_count": s.typing_tap_count,
                        "typing_speed": s.typing_speed,
                        "mean_inter_tap_interval_ms": s.mean_inter_tap_interval_ms,
                        "typing_cadence_variability": s.typing_cadence_variability,
                        "typing_cadence_stability": s.typing_cadence_stability,
                        "typing_gap_count": s.typing_gap_count,
                        "typing_gap_ratio": s.typing_gap_ratio,
                        "typing_burstiness": s.typing_burstiness,
                        "typing_activity_ratio": s.typing_activity_ratio,
                        "typing_interaction_intensity": s.typing_interaction_intensity,
                    })
                })
                .collect();
            meta.insert(
                "typing_metrics".to_string(),
                serde_json::Value::Array(metrics),
            );
        }

        // Add baseline info to meta
        if let Some(baseline) = signals.baselines.distraction_baseline {
            meta.insert(
                "baseline_distraction".to_string(),
                serde_json::Value::Number(serde_json::Number::from_f64(baseline).unwrap()),
            );
        }
        if let Some(deviation) = signals.distraction_deviation_pct {
            meta.insert(
                "distraction_deviation_pct".to_string(),
                serde_json::Value::Number(serde_json::Number::from_f64(deviation).unwrap()),
            );
        }
        meta.insert(
            "sessions_in_baseline".to_string(),
            serde_json::Value::Number(serde_json::Number::from(
                signals.baselines.sessions_in_baseline,
            )),
        );

        Ok(HsiPayload {
            hsi_version: HSI_VERSION.to_string(),
            observed_at_utc: canonical.end_time.to_rfc3339(),
            computed_at_utc: computed_at.to_rfc3339(),
            producer,
            window_ids: vec![window_id],
            windows,
            source_ids: Some(vec![source_id]),
            sources: Some(sources),
            axes: Some(axes),
            privacy,
            meta: Some(meta),
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
            typing_sessions: vec![],
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
            task_switch_cost: 0.0,
            active_time_ratio: 0.0,
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
    fn test_encode_hsi_compliant_payload() {
        let signals = make_test_contextual();
        let encoder = HsiBehaviorEncoder::with_instance_id("test-instance".to_string());
        let payload = encoder.encode(&signals).unwrap();

        // Check HSI version
        assert_eq!(payload.hsi_version, "1.0");

        // Check required fields are present
        assert!(!payload.observed_at_utc.is_empty());
        assert!(!payload.computed_at_utc.is_empty());

        // Check producer
        assert_eq!(payload.producer.name, PRODUCER_NAME);
        assert_eq!(payload.producer.version, FLUX_VERSION);
        assert_eq!(
            payload.producer.instance_id,
            Some("test-instance".to_string())
        );

        // Check windows
        assert_eq!(payload.window_ids.len(), 1);
        let window_id = &payload.window_ids[0];
        assert!(payload.windows.contains_key(window_id));
        let window = &payload.windows[window_id];
        assert!(!window.start.is_empty());
        assert!(!window.end.is_empty());

        // Check sources
        assert!(payload.source_ids.is_some());
        assert!(payload.sources.is_some());

        // Check axes
        assert!(payload.axes.is_some());
        let axes = payload.axes.as_ref().unwrap();
        assert!(axes.behavior.is_some());
        let behavior = axes.behavior.as_ref().unwrap();
        assert!(!behavior.readings.is_empty());

        // Check privacy
        assert!(!payload.privacy.contains_pii);
        assert!(payload.privacy.derived_metrics_allowed);

        // Verify distraction reading
        let distraction = behavior
            .readings
            .iter()
            .find(|r| r.axis == "distraction")
            .unwrap();
        assert!((distraction.score.unwrap() - 0.35).abs() < 0.001);
        assert_eq!(distraction.window_id, *window_id);
        assert_eq!(distraction.direction, Some(HsiDirection::HigherIsMore));
    }

    #[test]
    fn test_encode_to_json_valid() {
        let signals = make_test_contextual();
        let encoder = HsiBehaviorEncoder::new();
        let json = encoder.encode_to_json(&signals).unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Check required top-level fields
        assert_eq!(parsed["hsi_version"], "1.0");
        assert!(parsed.get("observed_at_utc").is_some());
        assert!(parsed.get("computed_at_utc").is_some());
        assert!(parsed.get("producer").is_some());
        assert!(parsed.get("window_ids").is_some());
        assert!(parsed.get("windows").is_some());
        assert!(parsed.get("privacy").is_some());

        // Check privacy constraints
        assert_eq!(parsed["privacy"]["contains_pii"], false);
    }

    #[test]
    fn test_axis_readings_have_required_fields() {
        let signals = make_test_contextual();
        let encoder = HsiBehaviorEncoder::new();
        let payload = encoder.encode(&signals).unwrap();

        let axes = payload.axes.unwrap();
        let behavior = axes.behavior.unwrap();

        for reading in &behavior.readings {
            // Check required fields
            assert!(!reading.axis.is_empty(), "axis must not be empty");
            assert!(reading.confidence >= 0.0 && reading.confidence <= 1.0);
            assert!(!reading.window_id.is_empty());

            // Score should be 0-1 or null
            if let Some(score) = reading.score {
                assert!(
                    (0.0..=1.0).contains(&score),
                    "score must be 0-1, got {score}"
                );
            }
        }
    }

    #[test]
    fn test_meta_contains_baseline_info() {
        let signals = make_test_contextual();
        let encoder = HsiBehaviorEncoder::new();
        let payload = encoder.encode(&signals).unwrap();

        let meta = payload.meta.unwrap();
        assert!(meta.contains_key("session_id"));
        assert!(meta.contains_key("duration_sec"));
        assert!(meta.contains_key("baseline_distraction"));
        assert!(meta.contains_key("distraction_deviation_pct"));
        assert!(meta.contains_key("sessions_in_baseline"));
    }

    #[test]
    fn test_quality_flags_in_source() {
        let mut signals = make_test_contextual();
        signals.derived.normalized.quality_flags = vec![
            BehaviorQualityFlag::ShortSession,
            BehaviorQualityFlag::LowEventCount,
        ];

        let encoder = HsiBehaviorEncoder::new();
        let payload = encoder.encode(&signals).unwrap();

        let sources = payload.sources.unwrap();
        let source = sources.values().next().unwrap();
        assert!(source.degraded);
        assert!(source.notes.is_some());
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
