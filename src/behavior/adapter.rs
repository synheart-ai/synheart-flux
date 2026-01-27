//! Behavioral session adapter
//!
//! Parses behavioral session JSON and converts to canonical signals.

use crate::behavior::types::{
    BehaviorEvent, BehaviorEventType, BehaviorSession, CanonicalBehaviorSignals, EngagementSegment,
    IdleSegment,
};
use crate::error::ComputeError;
use chrono::Utc;

/// Minimum gap duration (in seconds) to be considered idle
const IDLE_GAP_THRESHOLD_SEC: f64 = 30.0;

/// Minimum duration (in seconds) for an engagement segment
const MIN_ENGAGEMENT_DURATION_SEC: f64 = 10.0;

/// Parse a behavioral session JSON string into a BehaviorSession
pub fn parse_session(json: &str) -> Result<BehaviorSession, ComputeError> {
    serde_json::from_str(json)
        .map_err(|e| ComputeError::ParseError(format!("Failed to parse behavioral session: {e}")))
}

/// Convert a BehaviorSession to CanonicalBehaviorSignals
pub fn session_to_canonical(
    session: &BehaviorSession,
) -> Result<CanonicalBehaviorSignals, ComputeError> {
    // Validate session
    if session.start_time >= session.end_time {
        return Err(ComputeError::ParseError(
            "Session end time must be after start time".to_string(),
        ));
    }

    let duration_sec = (session.end_time - session.start_time).num_milliseconds() as f64 / 1000.0;

    // Sort events by timestamp
    let mut events = session.events.clone();
    events.sort_by_key(|e| e.timestamp);

    // Count events by type
    let (
        scroll_events,
        tap_events,
        swipe_events,
        notification_events,
        call_events,
        typing_events,
        app_switch_events,
    ) = count_events_by_type(&events);

    let total_events = events.len() as u32;

    // Count scroll direction reversals
    let scroll_direction_reversals = count_scroll_reversals(&events);

    // Extract per-typing-session metrics (if provided by producer)
    let typing_sessions = extract_typing_sessions(&events);

    // Calculate total typing duration
    let total_typing_duration_sec = typing_sessions
        .iter()
        .map(|s| s.duration as f64)
        .sum::<f64>();

    // Compute inter-event gaps (typing-aware capping for burstiness parity)
    let inter_event_gaps = compute_inter_event_gaps(&events);

    // Detect idle segments
    let idle_segments = detect_idle_segments(&events, &session.start_time, &session.end_time);
    let total_idle_time_sec: f64 = idle_segments.iter().map(|s| s.duration_sec).sum();

    // Detect engagement segments
    let engagement_segments =
        detect_engagement_segments(&events, &session.start_time, &session.end_time);

    Ok(CanonicalBehaviorSignals {
        session_id: session.session_id.clone(),
        device_id: session.device_id.clone(),
        timezone: session.timezone.clone(),
        start_time: session.start_time,
        end_time: session.end_time,
        duration_sec,
        total_events,
        scroll_events,
        tap_events,
        swipe_events,
        notification_events,
        call_events,
        typing_events,
        app_switch_events,
        scroll_direction_reversals,
        total_typing_duration_sec,
        typing_sessions,
        idle_segments,
        total_idle_time_sec,
        engagement_segments,
        inter_event_gaps,
        computed_at: Utc::now(),
    })
}

/// Count events by type
fn count_events_by_type(events: &[BehaviorEvent]) -> (u32, u32, u32, u32, u32, u32, u32) {
    let mut scroll = 0;
    let mut tap = 0;
    let mut swipe = 0;
    let mut notification = 0;
    let mut call = 0;
    let mut typing = 0;
    let mut app_switch = 0;

    for event in events {
        match event.event_type {
            BehaviorEventType::Scroll => scroll += 1,
            BehaviorEventType::Tap => tap += 1,
            BehaviorEventType::Swipe => swipe += 1,
            BehaviorEventType::Notification => notification += 1,
            BehaviorEventType::Call => call += 1,
            BehaviorEventType::Typing => typing += 1,
            BehaviorEventType::AppSwitch => app_switch += 1,
        }
    }

    (scroll, tap, swipe, notification, call, typing, app_switch)
}

/// Count scroll direction reversals
fn count_scroll_reversals(events: &[BehaviorEvent]) -> u32 {
    events
        .iter()
        .filter(|e| e.event_type == BehaviorEventType::Scroll)
        .filter_map(|e| e.scroll.as_ref())
        .filter(|s| s.direction_reversal)
        .count() as u32
}

fn extract_typing_sessions(
    events: &[BehaviorEvent],
) -> Vec<crate::behavior::types::TypingSessionMetrics> {
    events
        .iter()
        .filter(|e| e.event_type == BehaviorEventType::Typing)
        .filter_map(|e| e.typing.as_ref())
        .map(|t| crate::behavior::types::TypingSessionMetrics {
            start_at: t.start_at.clone().unwrap_or_default(),
            end_at: t.end_at.clone().unwrap_or_default(),
            duration: t.duration_sec.unwrap_or(0.0).round().max(0.0) as u32,
            deep_typing: t.deep_typing.unwrap_or(false),
            typing_tap_count: t.typing_tap_count.unwrap_or(0),
            // The SDK provides `typing_speed` as taps/sec; FluxBridge may map it into
            // `typing_speed_cpm`. We expose it as-is in the session object.
            typing_speed: t.typing_speed_cpm.unwrap_or(0.0),
            mean_inter_tap_interval_ms: t.mean_inter_tap_interval_ms.unwrap_or(0.0),
            typing_cadence_variability: t.typing_cadence_variability.unwrap_or(0.0),
            typing_cadence_stability: t
                .typing_cadence_stability
                .or(t.cadence_stability)
                .unwrap_or(0.0),
            typing_gap_count: t.typing_gap_count.unwrap_or(0),
            typing_gap_ratio: t.typing_gap_ratio.unwrap_or(0.0),
            typing_burstiness: t.typing_burstiness.unwrap_or(0.0),
            typing_activity_ratio: t.typing_activity_ratio.unwrap_or(0.0),
            typing_interaction_intensity: t.typing_interaction_intensity.unwrap_or(0.0),
        })
        .collect()
}

/// Compute inter-event gaps (time between consecutive events)
fn compute_inter_event_gaps(events: &[BehaviorEvent]) -> Vec<f64> {
    if events.len() < 2 {
        return Vec::new();
    }

    // Step 1: collect gaps with typing flag
    let mut gaps: Vec<(f64, bool)> = events
        .windows(2)
        .map(|pair| {
            let gap_ms = (pair[1].timestamp - pair[0].timestamp).num_milliseconds();
            let gap_sec = (gap_ms as f64 / 1000.0).max(0.0);
            let involves_typing = pair[0].event_type == BehaviorEventType::Typing
                || pair[1].event_type == BehaviorEventType::Typing;
            (gap_sec, involves_typing)
        })
        .collect();

    // Step 2: find max non-typing gap
    let max_non_typing_gap = gaps
        .iter()
        .filter(|(_, involves)| !*involves)
        .map(|(gap, _)| *gap)
        .fold(0.0_f64, f64::max);

    // Step 3: cap typing gaps at max non-typing gap (if present)
    if max_non_typing_gap > 0.0 {
        for (gap, involves_typing) in &mut gaps {
            if *involves_typing {
                *gap = gap.min(max_non_typing_gap);
            }
        }
    }

    gaps.into_iter().map(|(gap, _)| gap).collect()
}

/// Detect idle segments (gaps > 30 seconds)
fn detect_idle_segments(
    events: &[BehaviorEvent],
    session_start: &chrono::DateTime<Utc>,
    session_end: &chrono::DateTime<Utc>,
) -> Vec<IdleSegment> {
    let mut segments = Vec::new();

    if events.is_empty() {
        // Entire session is idle
        let duration_sec = (*session_end - *session_start).num_milliseconds() as f64 / 1000.0;
        if duration_sec > IDLE_GAP_THRESHOLD_SEC {
            // SDK subtracts the 30s threshold from idle time.
            let idle_duration_sec = (duration_sec - IDLE_GAP_THRESHOLD_SEC).max(0.0);
            segments.push(IdleSegment {
                start: *session_start
                    + chrono::Duration::milliseconds((IDLE_GAP_THRESHOLD_SEC * 1000.0) as i64),
                end: *session_end,
                duration_sec: idle_duration_sec,
            });
        }
        return segments;
    }

    // Check gap from session start to first event
    let first_gap_sec = (events[0].timestamp - *session_start).num_milliseconds() as f64 / 1000.0;
    if first_gap_sec > IDLE_GAP_THRESHOLD_SEC {
        let idle_duration_sec = (first_gap_sec - IDLE_GAP_THRESHOLD_SEC).max(0.0);
        segments.push(IdleSegment {
            start: *session_start
                + chrono::Duration::milliseconds((IDLE_GAP_THRESHOLD_SEC * 1000.0) as i64),
            end: events[0].timestamp,
            duration_sec: idle_duration_sec,
        });
    }

    // Check gaps between events
    for pair in events.windows(2) {
        let gap_sec = (pair[1].timestamp - pair[0].timestamp).num_milliseconds() as f64 / 1000.0;
        if gap_sec > IDLE_GAP_THRESHOLD_SEC {
            let idle_duration_sec = (gap_sec - IDLE_GAP_THRESHOLD_SEC).max(0.0);
            segments.push(IdleSegment {
                start: pair[0].timestamp
                    + chrono::Duration::milliseconds((IDLE_GAP_THRESHOLD_SEC * 1000.0) as i64),
                end: pair[1].timestamp,
                duration_sec: idle_duration_sec,
            });
        }
    }

    // Check gap from last event to session end
    let last_gap_sec =
        (*session_end - events.last().unwrap().timestamp).num_milliseconds() as f64 / 1000.0;
    if last_gap_sec > IDLE_GAP_THRESHOLD_SEC {
        let idle_duration_sec = (last_gap_sec - IDLE_GAP_THRESHOLD_SEC).max(0.0);
        segments.push(IdleSegment {
            start: events.last().unwrap().timestamp
                + chrono::Duration::milliseconds((IDLE_GAP_THRESHOLD_SEC * 1000.0) as i64),
            end: *session_end,
            duration_sec: idle_duration_sec,
        });
    }

    segments
}

/// Detect engagement segments (periods of sustained activity without interruptions)
fn detect_engagement_segments(
    events: &[BehaviorEvent],
    session_start: &chrono::DateTime<Utc>,
    session_end: &chrono::DateTime<Utc>,
) -> Vec<EngagementSegment> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let interruption_types = [
        BehaviorEventType::Notification,
        BehaviorEventType::Call,
        BehaviorEventType::AppSwitch,
    ];

    // Find first non-interruption event to seed a segment.
    let mut first_idx = None;
    for (i, e) in events.iter().enumerate() {
        if !interruption_types.contains(&e.event_type) {
            first_idx = Some(i);
            break;
        }
    }
    let Some(first_idx) = first_idx else {
        return Vec::new();
    };

    let mut segment_start = events[first_idx].timestamp;
    let mut segment_event_count: u32 = 1;

    // Check if initial gap is too large
    let initial_gap_sec =
        (events[first_idx].timestamp - *session_start).num_milliseconds() as f64 / 1000.0;
    if initial_gap_sec <= IDLE_GAP_THRESHOLD_SEC {
        segment_start = *session_start;
    }

    for pair in events.windows(2).skip(first_idx) {
        let current = &pair[1];
        let gap_sec = (pair[1].timestamp - pair[0].timestamp).num_milliseconds() as f64 / 1000.0;
        let is_interruption = interruption_types.contains(&current.event_type);

        if is_interruption || gap_sec > IDLE_GAP_THRESHOLD_SEC {
            // End current segment at the interruption time, or at last event before idle gap
            let segment_end = if is_interruption {
                current.timestamp
            } else {
                pair[0].timestamp
            };
            let duration_sec = (segment_end - segment_start).num_milliseconds() as f64 / 1000.0;
            if duration_sec >= MIN_ENGAGEMENT_DURATION_SEC && segment_event_count > 0 {
                segments.push(EngagementSegment {
                    start: segment_start,
                    end: segment_end,
                    duration_sec,
                    event_count: segment_event_count,
                });
            }
            // Start new segment
            segment_start = current.timestamp;
            segment_event_count = if is_interruption { 0 } else { 1 };
        } else {
            segment_event_count += 1;
        }
    }

    // Close final segment
    let last_event_time = events.last().unwrap().timestamp;
    let final_gap_sec = (*session_end - last_event_time).num_milliseconds() as f64 / 1000.0;
    let segment_end = if final_gap_sec <= IDLE_GAP_THRESHOLD_SEC {
        *session_end
    } else {
        last_event_time
    };

    let duration_sec = (segment_end - segment_start).num_milliseconds() as f64 / 1000.0;
    if duration_sec >= MIN_ENGAGEMENT_DURATION_SEC && segment_event_count > 0 {
        segments.push(EngagementSegment {
            start: segment_start,
            end: segment_end,
            duration_sec,
            event_count: segment_event_count,
        });
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::types::{ScrollDirection, ScrollEvent};
    use chrono::TimeZone;

    fn make_test_session() -> BehaviorSession {
        let start = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap();

        let events = vec![
            BehaviorEvent {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 14, 1, 0).unwrap(),
                event_type: BehaviorEventType::Scroll,
                scroll: Some(ScrollEvent {
                    velocity: Some(100.0),
                    direction: Some(ScrollDirection::Down),
                    direction_reversal: false,
                }),
                tap: None,
                swipe: None,
                interruption: None,
                typing: None,
                app_switch: None,
            },
            BehaviorEvent {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 14, 1, 30).unwrap(),
                event_type: BehaviorEventType::Scroll,
                scroll: Some(ScrollEvent {
                    velocity: Some(120.0),
                    direction: Some(ScrollDirection::Up),
                    direction_reversal: true,
                }),
                tap: None,
                swipe: None,
                interruption: None,
                typing: None,
                app_switch: None,
            },
            BehaviorEvent {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 14, 2, 0).unwrap(),
                event_type: BehaviorEventType::Tap,
                scroll: None,
                tap: None,
                swipe: None,
                interruption: None,
                typing: None,
                app_switch: None,
            },
        ];

        BehaviorSession {
            session_id: "test-session".to_string(),
            device_id: "test-device".to_string(),
            timezone: "UTC".to_string(),
            start_time: start,
            end_time: end,
            events,
        }
    }

    #[test]
    fn test_parse_session_json() {
        let json = r#"{
            "session_id": "sess-123",
            "device_id": "dev-456",
            "timezone": "America/New_York",
            "start_time": "2024-01-15T14:00:00Z",
            "end_time": "2024-01-15T14:30:00Z",
            "events": []
        }"#;

        let session = parse_session(json).unwrap();
        assert_eq!(session.session_id, "sess-123");
        assert_eq!(session.device_id, "dev-456");
    }

    #[test]
    fn test_session_to_canonical() {
        let session = make_test_session();
        let canonical = session_to_canonical(&session).unwrap();

        assert_eq!(canonical.session_id, "test-session");
        assert_eq!(canonical.total_events, 3);
        assert_eq!(canonical.scroll_events, 2);
        assert_eq!(canonical.tap_events, 1);
        assert_eq!(canonical.scroll_direction_reversals, 1);
        assert_eq!(canonical.duration_sec, 1800.0); // 30 minutes
    }

    #[test]
    fn test_inter_event_gaps() {
        let session = make_test_session();
        let canonical = session_to_canonical(&session).unwrap();

        // Events at 14:01:00, 14:01:30, 14:02:00
        // Gaps: 30 seconds, 30 seconds
        assert_eq!(canonical.inter_event_gaps.len(), 2);
        assert!((canonical.inter_event_gaps[0] - 30.0).abs() < 0.001);
        assert!((canonical.inter_event_gaps[1] - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_idle_segment_detection() {
        let start = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 15, 14, 5, 0).unwrap();

        // Events with gaps that exceed the 30-second idle threshold
        let events = vec![
            BehaviorEvent {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 14, 1, 0).unwrap(),
                event_type: BehaviorEventType::Tap,
                scroll: None,
                tap: None,
                swipe: None,
                interruption: None,
                typing: None,
                app_switch: None,
            },
            BehaviorEvent {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 14, 2, 0).unwrap(),
                event_type: BehaviorEventType::Tap,
                scroll: None,
                tap: None,
                swipe: None,
                interruption: None,
                typing: None,
                app_switch: None,
            },
            BehaviorEvent {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 14, 3, 30).unwrap(),
                event_type: BehaviorEventType::Tap,
                scroll: None,
                tap: None,
                swipe: None,
                interruption: None,
                typing: None,
                app_switch: None,
            },
        ];

        let session = BehaviorSession {
            session_id: "test".to_string(),
            device_id: "dev".to_string(),
            timezone: "UTC".to_string(),
            start_time: start,
            end_time: end,
            events,
        };

        let canonical = session_to_canonical(&session).unwrap();

        // Should detect idle segments (gaps > 30 seconds):
        // 1. From session start (14:00:00) to first event (14:01:00) - 60s > 30s
        // 2. From 14:01:00 to 14:02:00 - 60s > 30s
        // 3. From 14:02:00 to 14:03:30 - 90s > 30s
        // 4. From 14:03:30 to end (14:05:00) - 90s > 30s
        assert_eq!(canonical.idle_segments.len(), 4);
        // SDK subtracts the 30s threshold from idle time.
        assert!((canonical.idle_segments[0].duration_sec - 30.0).abs() < 0.001);
        assert!((canonical.idle_segments[1].duration_sec - 30.0).abs() < 0.001);
        assert!((canonical.idle_segments[2].duration_sec - 60.0).abs() < 0.001);
        assert!((canonical.idle_segments[3].duration_sec - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_invalid_session_times() {
        let session = BehaviorSession {
            session_id: "test".to_string(),
            device_id: "dev".to_string(),
            timezone: "UTC".to_string(),
            start_time: Utc.with_ymd_and_hms(2024, 1, 15, 15, 0, 0).unwrap(),
            end_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(), // Before start
            events: vec![],
        };

        let result = session_to_canonical(&session);
        assert!(result.is_err());
    }
}
