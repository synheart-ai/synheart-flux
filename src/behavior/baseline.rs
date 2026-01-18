//! Behavioral baseline management
//!
//! This module manages rolling baselines for behavioral metrics across sessions.
//! Baselines enable relative interpretation of distraction, focus, and other signals.

use crate::behavior::types::{
    BehaviorBaselines, ContextualBehaviorSignals, DerivedBehaviorSignals,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Default baseline window in sessions
pub const DEFAULT_BEHAVIOR_BASELINE_WINDOW: usize = 20;

/// Behavioral baseline store for managing rolling averages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorBaselineStore {
    /// Rolling distraction scores
    distraction_values: VecDeque<f64>,
    /// Rolling focus hints
    focus_values: VecDeque<f64>,
    /// Rolling burstiness values
    burstiness_values: VecDeque<f64>,
    /// Rolling interaction intensity values
    intensity_values: VecDeque<f64>,
    /// Maximum window size (number of sessions)
    window_size: usize,
}

impl Default for BehaviorBaselineStore {
    fn default() -> Self {
        Self::new(DEFAULT_BEHAVIOR_BASELINE_WINDOW)
    }
}

impl BehaviorBaselineStore {
    /// Create a new baseline store with specified window size (number of sessions)
    pub fn new(window_size: usize) -> Self {
        Self {
            distraction_values: VecDeque::with_capacity(window_size),
            focus_values: VecDeque::with_capacity(window_size),
            burstiness_values: VecDeque::with_capacity(window_size),
            intensity_values: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    /// Update baselines with new derived signals and return contextual signals
    pub fn update_and_contextualize(
        &mut self,
        derived: DerivedBehaviorSignals,
    ) -> ContextualBehaviorSignals {
        // Get current baselines before update (for deviation calculation)
        let old_baselines = self.get_baselines();

        // Calculate deviations from baseline
        let distraction_deviation_pct = self.calculate_deviation(
            Some(derived.distraction_score),
            old_baselines.distraction_baseline,
        );

        let focus_deviation_pct = self.calculate_deviation(
            Some(derived.focus_hint),
            old_baselines.focus_baseline,
        );

        // Update rolling values with current data
        self.distraction_values.push_back(derived.distraction_score);
        while self.distraction_values.len() > self.window_size {
            self.distraction_values.pop_front();
        }

        self.focus_values.push_back(derived.focus_hint);
        while self.focus_values.len() > self.window_size {
            self.focus_values.pop_front();
        }

        self.burstiness_values.push_back(derived.burstiness);
        while self.burstiness_values.len() > self.window_size {
            self.burstiness_values.pop_front();
        }

        self.intensity_values.push_back(derived.interaction_intensity);
        while self.intensity_values.len() > self.window_size {
            self.intensity_values.pop_front();
        }

        // Get updated baselines (including current data) for the output
        let baselines = self.get_baselines();

        ContextualBehaviorSignals {
            derived,
            baselines,
            distraction_deviation_pct,
            focus_deviation_pct,
        }
    }

    /// Get current baseline values
    pub fn get_baselines(&self) -> BehaviorBaselines {
        BehaviorBaselines {
            distraction_baseline: Self::rolling_average(&self.distraction_values),
            focus_baseline: Self::rolling_average(&self.focus_values),
            burstiness_baseline: Self::rolling_average(&self.burstiness_values),
            intensity_baseline: Self::rolling_average(&self.intensity_values),
            sessions_in_baseline: self.distraction_values.len() as u32,
        }
    }

    /// Calculate deviation from baseline as percentage
    fn calculate_deviation(&self, current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
        match (current, baseline) {
            (Some(curr), Some(base)) if base > 0.0 => Some(((curr - base) / base) * 100.0),
            (Some(curr), Some(base)) if base == 0.0 && curr > 0.0 => Some(100.0), // From 0 to something
            (Some(_curr), Some(_base)) => Some(0.0), // Both are 0
            _ => None,
        }
    }

    /// Calculate rolling average of a queue
    fn rolling_average(queue: &VecDeque<f64>) -> Option<f64> {
        if queue.is_empty() {
            return None;
        }
        let sum: f64 = queue.iter().sum();
        Some(sum / queue.len() as f64)
    }

    /// Load baseline store from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize baseline store to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Get the number of sessions currently in the baseline
    pub fn session_count(&self) -> usize {
        self.distraction_values.len()
    }

    /// Clear all baseline data
    pub fn clear(&mut self) {
        self.distraction_values.clear();
        self.focus_values.clear();
        self.burstiness_values.clear();
        self.intensity_values.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::types::{CanonicalBehaviorSignals, NormalizedBehaviorSignals};
    use chrono::{TimeZone, Utc};

    fn make_derived(distraction: f64, burstiness: f64, intensity: f64) -> DerivedBehaviorSignals {
        let canonical = CanonicalBehaviorSignals {
            session_id: "test".to_string(),
            device_id: "device".to_string(),
            timezone: "UTC".to_string(),
            start_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap(),
            end_time: Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap(),
            duration_sec: 1800.0,
            total_events: 100,
            scroll_events: 50,
            tap_events: 30,
            swipe_events: 5,
            notification_events: 5,
            call_events: 0,
            typing_events: 5,
            app_switch_events: 5,
            scroll_direction_reversals: 10,
            total_typing_duration_sec: 60.0,
            idle_segments: vec![],
            total_idle_time_sec: 60.0,
            engagement_segments: vec![],
            inter_event_gaps: vec![10.0, 12.0, 8.0],
            computed_at: Utc::now(),
        };

        let normalized = NormalizedBehaviorSignals {
            canonical,
            events_per_min: 3.33,
            scrolls_per_min: 1.67,
            taps_per_min: 1.0,
            swipes_per_min: 0.17,
            notifications_per_min: 0.17,
            app_switches_per_min: 0.17,
            coverage: 0.8,
            quality_flags: vec![],
        };

        DerivedBehaviorSignals {
            normalized,
            task_switch_rate: 0.3,
            notification_load: 0.15,
            idle_ratio: 0.033,
            fragmented_idle_ratio: 0.0,
            scroll_jitter_rate: 0.2,
            burstiness,
            deep_focus_blocks: 1,
            interaction_intensity: intensity,
            distraction_score: distraction,
            focus_hint: 1.0 - distraction,
        }
    }

    #[test]
    fn test_baseline_accumulation() {
        let mut store = BehaviorBaselineStore::new(10);

        // Add 5 sessions with distraction = 0.3
        for _ in 0..5 {
            let derived = make_derived(0.3, 0.5, 0.4);
            store.update_and_contextualize(derived);
        }

        let baselines = store.get_baselines();
        assert!(baselines.distraction_baseline.is_some());
        assert!((baselines.distraction_baseline.unwrap() - 0.3).abs() < 0.001);
        assert_eq!(baselines.sessions_in_baseline, 5);
    }

    #[test]
    fn test_baseline_window_rolling() {
        let mut store = BehaviorBaselineStore::new(3);

        // Add 5 sessions - only last 3 should be kept
        let distractions = [0.2, 0.3, 0.4, 0.5, 0.6];
        for &d in &distractions {
            let derived = make_derived(d, 0.5, 0.4);
            store.update_and_contextualize(derived);
        }

        let baselines = store.get_baselines();
        // Only 0.4, 0.5, 0.6 should be in window, average = 0.5
        assert!((baselines.distraction_baseline.unwrap() - 0.5).abs() < 0.001);
        assert_eq!(baselines.sessions_in_baseline, 3);
    }

    #[test]
    fn test_deviation_calculation() {
        let mut store = BehaviorBaselineStore::new(10);

        // Build baseline with distraction = 0.3
        for _ in 0..5 {
            let derived = make_derived(0.3, 0.5, 0.4);
            store.update_and_contextualize(derived);
        }

        // New session with distraction = 0.36 (20% above baseline)
        let derived = make_derived(0.36, 0.5, 0.4);
        let contextual = store.update_and_contextualize(derived);

        assert!(contextual.distraction_deviation_pct.is_some());
        // 0.36 vs baseline 0.3 = 20% increase
        let expected = ((0.36 - 0.3) / 0.3) * 100.0;
        assert!((contextual.distraction_deviation_pct.unwrap() - expected).abs() < 0.1);
    }

    #[test]
    fn test_serialization() {
        let mut store = BehaviorBaselineStore::new(10);
        let derived = make_derived(0.35, 0.5, 0.45);
        store.update_and_contextualize(derived);

        let json = store.to_json().unwrap();
        let loaded = BehaviorBaselineStore::from_json(&json).unwrap();

        let orig_baselines = store.get_baselines();
        let loaded_baselines = loaded.get_baselines();

        assert_eq!(
            orig_baselines.distraction_baseline,
            loaded_baselines.distraction_baseline
        );
        assert_eq!(
            orig_baselines.sessions_in_baseline,
            loaded_baselines.sessions_in_baseline
        );
    }

    #[test]
    fn test_clear_baselines() {
        let mut store = BehaviorBaselineStore::new(10);

        for _ in 0..5 {
            let derived = make_derived(0.3, 0.5, 0.4);
            store.update_and_contextualize(derived);
        }

        assert_eq!(store.session_count(), 5);

        store.clear();
        assert_eq!(store.session_count(), 0);

        let baselines = store.get_baselines();
        assert!(baselines.distraction_baseline.is_none());
    }

    #[test]
    fn test_empty_baseline_deviation() {
        let mut store = BehaviorBaselineStore::new(10);

        // First session, no baseline to compare against
        let derived = make_derived(0.3, 0.5, 0.4);
        let contextual = store.update_and_contextualize(derived);

        // No deviation should be calculated when there's no prior baseline
        // Actually the first session WILL have a deviation because we calculate it before updating
        // But with no prior data, baseline is None, so deviation is None
        // Wait, we need to check the logic... the first session:
        // - get_baselines() returns None for all (empty queues)
        // - calculate_deviation with None baseline returns None
        // That's correct!
        assert!(contextual.distraction_deviation_pct.is_none());
    }

    #[test]
    fn test_focus_is_inverse_tracking() {
        let mut store = BehaviorBaselineStore::new(10);

        for i in 0..5 {
            let distraction = 0.2 + (i as f64) * 0.1; // 0.2, 0.3, 0.4, 0.5, 0.6
            let derived = make_derived(distraction, 0.5, 0.4);
            store.update_and_contextualize(derived);
        }

        let baselines = store.get_baselines();
        // Average distraction = 0.4, so average focus = 0.6
        assert!((baselines.focus_baseline.unwrap() - 0.6).abs() < 0.001);
    }
}
