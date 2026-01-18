//! Behavioral feature derivation
//!
//! Computes derived behavioral metrics from normalized signals using the formulas
//! from synheart-behavior-dart.

use crate::behavior::types::{DerivedBehaviorSignals, NormalizedBehaviorSignals};

/// Minimum duration for deep focus block (120 seconds = 2 minutes)
const DEEP_FOCUS_MIN_DURATION_SEC: f64 = 120.0;

/// Feature deriver for behavioral signals
pub struct BehaviorFeatureDeriver;

impl BehaviorFeatureDeriver {
    /// Derive behavioral features from normalized signals
    pub fn derive(normalized: NormalizedBehaviorSignals) -> DerivedBehaviorSignals {
        let canonical = &normalized.canonical;

        // Core metrics
        let task_switch_rate = compute_task_switch_rate(normalized.app_switches_per_min);
        let notification_load = compute_notification_load(normalized.notifications_per_min);
        let idle_ratio = compute_idle_ratio(canonical.total_idle_time_sec, canonical.duration_sec);
        let fragmented_idle_ratio = compute_fragmented_idle_ratio(
            canonical.idle_segments.len() as u32,
            canonical.duration_sec,
        );
        let scroll_jitter_rate = compute_scroll_jitter_rate(
            canonical.scroll_direction_reversals,
            canonical.scroll_events,
        );
        let burstiness = compute_burstiness(&canonical.inter_event_gaps);
        let deep_focus_blocks = count_deep_focus_blocks(&canonical.engagement_segments);
        let interaction_intensity = compute_interaction_intensity(
            canonical.total_events,
            canonical.notification_events + canonical.call_events,
            canonical.total_typing_duration_sec,
            canonical.duration_sec,
        );

        // Composite scores
        let distraction_score = compute_distraction_score(
            task_switch_rate,
            notification_load,
            fragmented_idle_ratio,
            scroll_jitter_rate,
        );
        let focus_hint = 1.0 - distraction_score;

        DerivedBehaviorSignals {
            normalized,
            task_switch_rate,
            notification_load,
            idle_ratio,
            fragmented_idle_ratio,
            scroll_jitter_rate,
            burstiness,
            deep_focus_blocks,
            interaction_intensity,
            distraction_score,
            focus_hint,
        }
    }
}

/// Compute task switch rate using exponential saturation
///
/// Formula: `1.0 - exp(-app_switches_per_min / 0.5)`
/// This maps 0.5 switches/min to ~63% task switching, reaching near 1.0 asymptotically
fn compute_task_switch_rate(app_switches_per_min: f64) -> f64 {
    (1.0 - (-app_switches_per_min / 0.5).exp()).clamp(0.0, 1.0)
}

/// Compute notification load using exponential saturation
///
/// Formula: `1.0 - exp(-notifications_per_min / 1.0)`
/// This maps 1 notification/min to ~63% load
fn compute_notification_load(notifications_per_min: f64) -> f64 {
    (1.0 - (-notifications_per_min / 1.0).exp()).clamp(0.0, 1.0)
}

/// Compute idle ratio
///
/// Formula: `total_idle_time / session_duration`
/// Where idle gaps are > 30 seconds
fn compute_idle_ratio(total_idle_time_sec: f64, session_duration_sec: f64) -> f64 {
    if session_duration_sec <= 0.0 {
        return 0.0;
    }
    (total_idle_time_sec / session_duration_sec).clamp(0.0, 1.0)
}

/// Compute fragmented idle ratio
///
/// Formula: `idle_segment_count / session_duration_sec`
/// This measures how frequently idle periods occur, normalized by session length
fn compute_fragmented_idle_ratio(idle_segment_count: u32, session_duration_sec: f64) -> f64 {
    if session_duration_sec <= 0.0 {
        return 0.0;
    }
    // Scale to make typical values fall in 0-1 range
    // ~1 idle segment per 60 seconds would give 0.0167, we want this to be meaningful
    // So we multiply by 60 to get "idle segments per minute equivalent"
    let segments_per_minute = (idle_segment_count as f64 / session_duration_sec) * 60.0;
    // Cap at 1.0 (more than 1 idle segment per minute is very fragmented)
    segments_per_minute.clamp(0.0, 1.0)
}

/// Compute scroll jitter rate
///
/// Formula: `direction_reversals / (scroll_events - 1)`
/// This measures how often the user changes scroll direction (indicative of searching/scanning)
fn compute_scroll_jitter_rate(direction_reversals: u32, scroll_events: u32) -> f64 {
    if scroll_events <= 1 {
        return 0.0;
    }
    let max_reversals = scroll_events - 1;
    (direction_reversals as f64 / max_reversals as f64).clamp(0.0, 1.0)
}

/// Compute burstiness using the Barabási formula
///
/// Formula: `((σ - μ) / (σ + μ) + 1) / 2`
/// Where σ is standard deviation and μ is mean of inter-event gaps
///
/// Result: 0.0 = perfectly regular (Poisson), 0.5 = random, 1.0 = very bursty
fn compute_burstiness(inter_event_gaps: &[f64]) -> f64 {
    if inter_event_gaps.is_empty() {
        return 0.5; // Default to neutral when no data
    }

    let n = inter_event_gaps.len() as f64;
    let mean: f64 = inter_event_gaps.iter().sum::<f64>() / n;

    if mean <= 0.0 {
        return 0.5;
    }

    let variance: f64 = inter_event_gaps
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / n;
    let std_dev = variance.sqrt();

    // Barabási burstiness formula: B = (σ - μ) / (σ + μ)
    // This gives values from -1 (periodic) to 1 (bursty)
    // We normalize to 0-1: (B + 1) / 2
    let barabasi = (std_dev - mean) / (std_dev + mean);
    ((barabasi + 1.0) / 2.0).clamp(0.0, 1.0)
}

/// Count deep focus blocks (engagement segments >= 120 seconds without interruptions)
fn count_deep_focus_blocks(
    engagement_segments: &[crate::behavior::types::EngagementSegment],
) -> u32 {
    engagement_segments
        .iter()
        .filter(|s| s.duration_sec >= DEEP_FOCUS_MIN_DURATION_SEC)
        .count() as u32
}

/// Compute interaction intensity
///
/// Formula: `(non_interruption_events + typing_duration/10) / session_duration`
/// This measures sustained engagement level
fn compute_interaction_intensity(
    total_events: u32,
    interruption_events: u32,
    typing_duration_sec: f64,
    session_duration_sec: f64,
) -> f64 {
    if session_duration_sec <= 0.0 {
        return 0.0;
    }

    let non_interruption_events = total_events.saturating_sub(interruption_events);
    let typing_equivalent = typing_duration_sec / 10.0; // 10 seconds of typing = 1 "event"
    let total_interaction = non_interruption_events as f64 + typing_equivalent;

    // Normalize to events per minute, then scale to 0-1 range
    // ~10 events per minute is considered high intensity
    let events_per_minute = (total_interaction / session_duration_sec) * 60.0;
    (events_per_minute / 10.0).clamp(0.0, 1.0)
}

/// Compute distraction score (weighted combination)
///
/// Formula:
/// ```text
/// Distraction Score = 0.35 * task_switch_rate
///                   + 0.30 * notification_load
///                   + 0.20 * fragmented_idle_ratio
///                   + 0.15 * scroll_jitter_rate
/// ```
fn compute_distraction_score(
    task_switch_rate: f64,
    notification_load: f64,
    fragmented_idle_ratio: f64,
    scroll_jitter_rate: f64,
) -> f64 {
    let score = 0.35 * task_switch_rate
        + 0.30 * notification_load
        + 0.20 * fragmented_idle_ratio
        + 0.15 * scroll_jitter_rate;
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::types::{CanonicalBehaviorSignals, EngagementSegment, IdleSegment};
    use chrono::{TimeZone, Utc};

    fn make_test_normalized() -> NormalizedBehaviorSignals {
        let canonical = CanonicalBehaviorSignals {
            session_id: "test".to_string(),
            device_id: "device".to_string(),
            timezone: "UTC".to_string(),
            start_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
            end_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap(),
            duration_sec: 1800.0, // 30 minutes
            total_events: 120,
            scroll_events: 60,
            tap_events: 40,
            swipe_events: 5,
            notification_events: 8,
            call_events: 2,
            typing_events: 3,
            app_switch_events: 6,
            scroll_direction_reversals: 12,
            total_typing_duration_sec: 120.0,
            idle_segments: vec![IdleSegment {
                start: Utc.with_ymd_and_hms(2024, 1, 15, 14, 10, 0).unwrap(),
                end: Utc.with_ymd_and_hms(2024, 1, 15, 14, 11, 0).unwrap(),
                duration_sec: 60.0,
            }],
            total_idle_time_sec: 120.0,
            engagement_segments: vec![
                EngagementSegment {
                    start: Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
                    end: Utc.with_ymd_and_hms(2024, 1, 15, 14, 5, 0).unwrap(),
                    duration_sec: 300.0, // 5 minutes - deep focus
                    event_count: 30,
                },
                EngagementSegment {
                    start: Utc.with_ymd_and_hms(2024, 1, 15, 14, 15, 0).unwrap(),
                    end: Utc.with_ymd_and_hms(2024, 1, 15, 14, 16, 0).unwrap(),
                    duration_sec: 60.0, // 1 minute - not deep focus
                    event_count: 10,
                },
            ],
            inter_event_gaps: vec![10.0, 5.0, 15.0, 8.0, 12.0, 3.0, 20.0, 7.0],
            computed_at: Utc::now(),
        };

        NormalizedBehaviorSignals {
            canonical,
            events_per_min: 4.0,
            scrolls_per_min: 2.0,
            taps_per_min: 1.33,
            swipes_per_min: 0.17,
            notifications_per_min: 0.27, // 8 notifications in 30 min
            app_switches_per_min: 0.2,   // 6 switches in 30 min
            coverage: 0.85,
            quality_flags: vec![],
        }
    }

    #[test]
    fn test_task_switch_rate() {
        // 0 switches/min should give 0
        assert!((compute_task_switch_rate(0.0) - 0.0).abs() < 0.001);

        // 0.5 switches/min should give ~63% (1 - e^-1)
        let rate_at_half = compute_task_switch_rate(0.5);
        assert!((rate_at_half - 0.632).abs() < 0.01);

        // High rate should approach 1
        assert!(compute_task_switch_rate(5.0) > 0.99);
    }

    #[test]
    fn test_notification_load() {
        // 0 notifications/min should give 0
        assert!((compute_notification_load(0.0) - 0.0).abs() < 0.001);

        // 1 notification/min should give ~63%
        let load_at_one = compute_notification_load(1.0);
        assert!((load_at_one - 0.632).abs() < 0.01);

        // High load should approach 1
        assert!(compute_notification_load(5.0) > 0.99);
    }

    #[test]
    fn test_idle_ratio() {
        // 120 seconds idle in 1800 second session = 6.67%
        let ratio = compute_idle_ratio(120.0, 1800.0);
        assert!((ratio - 0.0667).abs() < 0.01);

        // Zero duration should return 0
        assert_eq!(compute_idle_ratio(100.0, 0.0), 0.0);

        // More idle than session should cap at 1
        assert_eq!(compute_idle_ratio(2000.0, 1000.0), 1.0);
    }

    #[test]
    fn test_fragmented_idle_ratio() {
        // 1 idle segment in 1800 seconds
        // = (1 / 1800) * 60 = 0.033 segments per minute
        let ratio = compute_fragmented_idle_ratio(1, 1800.0);
        assert!((ratio - 0.033).abs() < 0.01);

        // 30 segments in 1800 seconds = 1 per minute
        let high_fragmentation = compute_fragmented_idle_ratio(30, 1800.0);
        assert!((high_fragmentation - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_scroll_jitter_rate() {
        // 12 reversals in 60 scrolls, max possible = 59
        let rate = compute_scroll_jitter_rate(12, 60);
        assert!((rate - 12.0 / 59.0).abs() < 0.01);

        // No scrolls should return 0
        assert_eq!(compute_scroll_jitter_rate(0, 0), 0.0);
        assert_eq!(compute_scroll_jitter_rate(5, 1), 0.0);

        // All reversals
        assert_eq!(compute_scroll_jitter_rate(9, 10), 1.0);
    }

    #[test]
    fn test_burstiness() {
        // Empty gaps should return 0.5 (neutral)
        assert_eq!(compute_burstiness(&[]), 0.5);

        // Perfectly regular intervals should be low burstiness
        let regular = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let regular_burstiness = compute_burstiness(&regular);
        assert!(regular_burstiness < 0.3); // Low burstiness

        // Highly variable intervals should be high burstiness
        let bursty = vec![1.0, 1.0, 100.0, 1.0, 1.0, 100.0];
        let bursty_burstiness = compute_burstiness(&bursty);
        assert!(bursty_burstiness > 0.5); // Higher burstiness
    }

    #[test]
    fn test_deep_focus_blocks() {
        let normalized = make_test_normalized();
        let derived = BehaviorFeatureDeriver::derive(normalized);

        // Should have 1 deep focus block (300 seconds >= 120 seconds)
        assert_eq!(derived.deep_focus_blocks, 1);
    }

    #[test]
    fn test_interaction_intensity() {
        // 110 non-interruption events + 120/10 = 12 typing equivalent = 122 total
        // In 1800 seconds = 4.07 events per minute
        // Scaled to 0-1: 4.07 / 10 = 0.407
        let intensity = compute_interaction_intensity(120, 10, 120.0, 1800.0);
        assert!((intensity - 0.407).abs() < 0.02);

        // Zero duration should return 0
        assert_eq!(compute_interaction_intensity(100, 10, 60.0, 0.0), 0.0);
    }

    #[test]
    fn test_distraction_score_weights() {
        // Test that weights sum correctly
        // Max distraction: all components at 1.0
        let max_distraction = compute_distraction_score(1.0, 1.0, 1.0, 1.0);
        assert!((max_distraction - 1.0).abs() < 0.001);

        // Zero distraction: all components at 0.0
        let min_distraction = compute_distraction_score(0.0, 0.0, 0.0, 0.0);
        assert!((min_distraction - 0.0).abs() < 0.001);

        // Individual weight contributions
        let task_only = compute_distraction_score(1.0, 0.0, 0.0, 0.0);
        assert!((task_only - 0.35).abs() < 0.001);

        let notif_only = compute_distraction_score(0.0, 1.0, 0.0, 0.0);
        assert!((notif_only - 0.30).abs() < 0.001);
    }

    #[test]
    fn test_focus_hint_is_inverse_of_distraction() {
        let normalized = make_test_normalized();
        let derived = BehaviorFeatureDeriver::derive(normalized);

        assert!((derived.focus_hint + derived.distraction_score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_full_feature_derivation() {
        let normalized = make_test_normalized();
        let derived = BehaviorFeatureDeriver::derive(normalized);

        // All scores should be in valid ranges
        assert!(derived.task_switch_rate >= 0.0 && derived.task_switch_rate <= 1.0);
        assert!(derived.notification_load >= 0.0 && derived.notification_load <= 1.0);
        assert!(derived.idle_ratio >= 0.0 && derived.idle_ratio <= 1.0);
        assert!(derived.fragmented_idle_ratio >= 0.0 && derived.fragmented_idle_ratio <= 1.0);
        assert!(derived.scroll_jitter_rate >= 0.0 && derived.scroll_jitter_rate <= 1.0);
        assert!(derived.burstiness >= 0.0 && derived.burstiness <= 1.0);
        assert!(derived.distraction_score >= 0.0 && derived.distraction_score <= 1.0);
        assert!(derived.focus_hint >= 0.0 && derived.focus_hint <= 1.0);
        assert!(derived.interaction_intensity >= 0.0);
    }
}
