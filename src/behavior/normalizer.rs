//! Behavioral signal normalization
//!
//! Converts canonical signals to rates per minute and calculates quality metrics.

use crate::behavior::types::{
    BehaviorQualityFlag, CanonicalBehaviorSignals, NormalizedBehaviorSignals,
};

/// Minimum session duration in seconds for full quality (5 minutes)
const MIN_SESSION_DURATION_SEC: f64 = 300.0;

/// Minimum event count for full quality
const MIN_EVENT_COUNT: u32 = 10;

/// Maximum idle ratio before flagging
const MAX_IDLE_RATIO: f64 = 0.8;

/// Normalizer for behavioral signals
pub struct BehaviorNormalizer;

impl BehaviorNormalizer {
    /// Normalize canonical signals to rates per minute with quality assessment
    pub fn normalize(canonical: CanonicalBehaviorSignals) -> NormalizedBehaviorSignals {
        let duration_min = canonical.duration_sec / 60.0;

        // Calculate rates per minute (avoid division by zero)
        let events_per_min = if duration_min > 0.0 {
            canonical.total_events as f64 / duration_min
        } else {
            0.0
        };

        let scrolls_per_min = if duration_min > 0.0 {
            canonical.scroll_events as f64 / duration_min
        } else {
            0.0
        };

        let taps_per_min = if duration_min > 0.0 {
            canonical.tap_events as f64 / duration_min
        } else {
            0.0
        };

        let swipes_per_min = if duration_min > 0.0 {
            canonical.swipe_events as f64 / duration_min
        } else {
            0.0
        };

        let notifications_per_min = if duration_min > 0.0 {
            canonical.notification_events as f64 / duration_min
        } else {
            0.0
        };

        let app_switches_per_min = if duration_min > 0.0 {
            canonical.app_switch_events as f64 / duration_min
        } else {
            0.0
        };

        // Calculate coverage based on event diversity
        let coverage = calculate_coverage(&canonical);

        // Determine quality flags
        let quality_flags = determine_quality_flags(&canonical);

        NormalizedBehaviorSignals {
            canonical,
            events_per_min,
            scrolls_per_min,
            taps_per_min,
            swipes_per_min,
            notifications_per_min,
            app_switches_per_min,
            coverage,
            quality_flags,
        }
    }
}

/// Calculate coverage based on event type diversity (0-1)
fn calculate_coverage(canonical: &CanonicalBehaviorSignals) -> f64 {
    // Count how many different event types are present
    let mut type_count = 0;
    if canonical.scroll_events > 0 {
        type_count += 1;
    }
    if canonical.tap_events > 0 {
        type_count += 1;
    }
    if canonical.swipe_events > 0 {
        type_count += 1;
    }
    if canonical.notification_events > 0 || canonical.call_events > 0 {
        type_count += 1;
    }
    if canonical.typing_events > 0 {
        type_count += 1;
    }
    if canonical.app_switch_events > 0 {
        type_count += 1;
    }

    // Max 6 categories, base coverage on diversity
    let diversity_score = type_count as f64 / 6.0;

    // Also factor in session duration quality
    let duration_score = (canonical.duration_sec / MIN_SESSION_DURATION_SEC).min(1.0);

    // Event count quality
    let event_score = (canonical.total_events as f64 / MIN_EVENT_COUNT as f64).min(1.0);

    // Weight: 40% diversity, 30% duration, 30% event count
    (0.4 * diversity_score + 0.3 * duration_score + 0.3 * event_score).clamp(0.0, 1.0)
}

/// Determine quality flags based on session characteristics
fn determine_quality_flags(canonical: &CanonicalBehaviorSignals) -> Vec<BehaviorQualityFlag> {
    let mut flags = Vec::new();

    // Check session duration
    if canonical.duration_sec < MIN_SESSION_DURATION_SEC {
        flags.push(BehaviorQualityFlag::ShortSession);
    }

    // Check event count
    if canonical.total_events < MIN_EVENT_COUNT {
        flags.push(BehaviorQualityFlag::LowEventCount);
    }

    // Check idle ratio
    let idle_ratio = if canonical.duration_sec > 0.0 {
        canonical.total_idle_time_sec / canonical.duration_sec
    } else {
        0.0
    };
    if idle_ratio > MAX_IDLE_RATIO {
        flags.push(BehaviorQualityFlag::HighIdleRatio);
    }

    // Check event diversity
    let event_types_present = count_event_types_present(canonical);
    if event_types_present <= 1 {
        flags.push(BehaviorQualityFlag::LowEventDiversity);
    }

    // Check for session gaps (multiple idle segments may indicate device was off)
    if canonical.idle_segments.len() >= 3 {
        flags.push(BehaviorQualityFlag::SessionGaps);
    }

    flags
}

/// Count how many different event types have at least one event
fn count_event_types_present(canonical: &CanonicalBehaviorSignals) -> u32 {
    let mut count = 0;
    if canonical.scroll_events > 0 {
        count += 1;
    }
    if canonical.tap_events > 0 {
        count += 1;
    }
    if canonical.swipe_events > 0 {
        count += 1;
    }
    if canonical.notification_events > 0 {
        count += 1;
    }
    if canonical.call_events > 0 {
        count += 1;
    }
    if canonical.typing_events > 0 {
        count += 1;
    }
    if canonical.app_switch_events > 0 {
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn make_test_canonical() -> CanonicalBehaviorSignals {
        CanonicalBehaviorSignals {
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
            call_events: 0,
            typing_events: 3,
            app_switch_events: 4,
            scroll_direction_reversals: 10,
            total_typing_duration_sec: 45.0,
            idle_segments: vec![],
            total_idle_time_sec: 120.0,
            engagement_segments: vec![],
            inter_event_gaps: vec![10.0, 15.0, 8.0, 12.0],
            computed_at: Utc::now(),
        }
    }

    #[test]
    fn test_normalize_rates() {
        let canonical = make_test_canonical();
        let normalized = BehaviorNormalizer::normalize(canonical);

        // 120 events in 30 minutes = 4 events/min
        assert!((normalized.events_per_min - 4.0).abs() < 0.001);

        // 60 scrolls in 30 minutes = 2 scrolls/min
        assert!((normalized.scrolls_per_min - 2.0).abs() < 0.001);

        // 40 taps in 30 minutes = 1.33 taps/min
        assert!((normalized.taps_per_min - 1.333).abs() < 0.01);
    }

    #[test]
    fn test_coverage_calculation() {
        let canonical = make_test_canonical();
        let normalized = BehaviorNormalizer::normalize(canonical);

        // With 6 different event types, good duration, and many events
        // Coverage should be high
        assert!(normalized.coverage > 0.7);
    }

    #[test]
    fn test_quality_flags_short_session() {
        let mut canonical = make_test_canonical();
        canonical.duration_sec = 60.0; // 1 minute - too short

        let normalized = BehaviorNormalizer::normalize(canonical);
        assert!(normalized
            .quality_flags
            .contains(&BehaviorQualityFlag::ShortSession));
    }

    #[test]
    fn test_quality_flags_low_event_count() {
        let mut canonical = make_test_canonical();
        canonical.total_events = 5; // Less than 10

        let normalized = BehaviorNormalizer::normalize(canonical);
        assert!(normalized
            .quality_flags
            .contains(&BehaviorQualityFlag::LowEventCount));
    }

    #[test]
    fn test_quality_flags_high_idle() {
        let mut canonical = make_test_canonical();
        canonical.total_idle_time_sec = 1500.0; // 1500/1800 = 83% idle

        let normalized = BehaviorNormalizer::normalize(canonical);
        assert!(normalized
            .quality_flags
            .contains(&BehaviorQualityFlag::HighIdleRatio));
    }

    #[test]
    fn test_quality_flags_low_diversity() {
        let mut canonical = make_test_canonical();
        // Only scrolls, nothing else
        canonical.tap_events = 0;
        canonical.swipe_events = 0;
        canonical.notification_events = 0;
        canonical.call_events = 0;
        canonical.typing_events = 0;
        canonical.app_switch_events = 0;

        let normalized = BehaviorNormalizer::normalize(canonical);
        assert!(normalized
            .quality_flags
            .contains(&BehaviorQualityFlag::LowEventDiversity));
    }

    #[test]
    fn test_zero_duration_handling() {
        let mut canonical = make_test_canonical();
        canonical.duration_sec = 0.0;

        let normalized = BehaviorNormalizer::normalize(canonical);
        assert_eq!(normalized.events_per_min, 0.0);
        assert_eq!(normalized.scrolls_per_min, 0.0);
    }
}
