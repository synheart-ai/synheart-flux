//! Baseline management
//!
//! This module manages rolling baselines for HRV, RHR, and sleep metrics.
//! Baselines enable relative interpretation of daily signals.

use crate::context::BioDailyContext;
use crate::types::{Baselines, ContextualSignals, DerivedSignals};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Default baseline window in days
pub const DEFAULT_BASELINE_WINDOW: usize = 14;

/// Current baseline store format version
const BASELINE_STORE_VERSION: u32 = 2;

/// Baseline store for managing rolling averages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineStore {
    /// Format version for backward compatibility
    #[serde(default = "default_version")]
    version: u32,
    /// Rolling HRV values (ms)
    hrv_values: VecDeque<f64>,
    /// Rolling RHR values (bpm)
    rhr_values: VecDeque<f64>,
    /// Rolling sleep duration values (minutes)
    sleep_duration_values: VecDeque<f64>,
    /// Rolling sleep efficiency values (0-1)
    sleep_efficiency_values: VecDeque<f64>,
    /// Maximum window size
    window_size: usize,
    /// Last captured bio daily context (V2+)
    #[serde(skip_serializing_if = "Option::is_none")]
    bio_daily_context_last: Option<BioDailyContext>,
}

fn default_version() -> u32 {
    1 // V1 for backward compatibility with existing blobs
}

impl Default for BaselineStore {
    fn default() -> Self {
        Self::new(DEFAULT_BASELINE_WINDOW)
    }
}

impl BaselineStore {
    /// Create a new baseline store with specified window size
    pub fn new(window_size: usize) -> Self {
        Self {
            version: BASELINE_STORE_VERSION,
            hrv_values: VecDeque::with_capacity(window_size),
            rhr_values: VecDeque::with_capacity(window_size),
            sleep_duration_values: VecDeque::with_capacity(window_size),
            sleep_efficiency_values: VecDeque::with_capacity(window_size),
            window_size,
            bio_daily_context_last: None,
        }
    }

    /// Update baselines with new derived signals and return contextual signals
    pub fn update_and_contextualize(&mut self, derived: DerivedSignals) -> ContextualSignals {
        // Get current baselines before update (for deviation calculation)
        let old_baselines = self.get_baselines();

        // Calculate deviations from baseline (compare current day to historical baseline)
        let hrv_deviation_pct = self.calculate_deviation(
            derived.normalized.canonical.recovery.hrv_rmssd_ms,
            old_baselines.hrv_baseline_ms,
        );

        let rhr_deviation_pct = self.calculate_deviation(
            derived.normalized.canonical.recovery.resting_hr_bpm,
            old_baselines.rhr_baseline_bpm,
        );

        let sleep_duration_deviation_pct = self.calculate_deviation(
            derived.normalized.canonical.sleep.total_sleep_minutes,
            old_baselines.sleep_baseline_minutes,
        );

        // Update rolling values with current data
        if let Some(hrv) = derived.normalized.canonical.recovery.hrv_rmssd_ms {
            self.hrv_values.push_back(hrv);
            while self.hrv_values.len() > self.window_size {
                self.hrv_values.pop_front();
            }
        }

        if let Some(rhr) = derived.normalized.canonical.recovery.resting_hr_bpm {
            self.rhr_values.push_back(rhr);
            while self.rhr_values.len() > self.window_size {
                self.rhr_values.pop_front();
            }
        }

        if let Some(sleep) = derived.normalized.canonical.sleep.total_sleep_minutes {
            self.sleep_duration_values.push_back(sleep);
            while self.sleep_duration_values.len() > self.window_size {
                self.sleep_duration_values.pop_front();
            }
        }

        if let Some(efficiency) = derived.sleep_efficiency {
            self.sleep_efficiency_values.push_back(efficiency);
            while self.sleep_efficiency_values.len() > self.window_size {
                self.sleep_efficiency_values.pop_front();
            }
        }

        // Get updated baselines (including current data) for the output
        let baselines = self.get_baselines();

        // Capture bio daily context for snapshot API
        self.capture_bio_context(&derived, &baselines, hrv_deviation_pct, rhr_deviation_pct);

        ContextualSignals {
            derived,
            baselines,
            hrv_deviation_pct,
            rhr_deviation_pct,
            sleep_duration_deviation_pct,
        }
    }

    /// Capture bio daily context from the latest processing
    fn capture_bio_context(
        &mut self,
        derived: &DerivedSignals,
        _baselines: &Baselines,
        hrv_deviation_pct: Option<f64>,
        rhr_deviation_pct: Option<f64>,
    ) {
        let canonical = &derived.normalized.canonical;
        let now = Utc::now();

        let mut context = BioDailyContext::new(canonical.observed_at, now);

        // Sleep quality from normalized sleep score
        if let Some(score) = derived.normalized.sleep_score {
            context.sleep_quality = Some(score as f32);
        }

        // Recovery from normalized recovery score
        if let Some(score) = derived.normalized.recovery_score {
            context.recovery = Some(score as f32);
        }

        // HRV delta: normalize percentage deviation to -1..1
        // Typical deviation range: -50% to +50%, so divide by 50 and clamp
        if let Some(pct) = hrv_deviation_pct {
            context.hrv_delta = Some((pct / 50.0).clamp(-1.0, 1.0) as f32);
        }

        // RHR delta: normalize percentage deviation to -1..1
        // Note: for RHR, higher is worse, so we preserve the sign
        if let Some(pct) = rhr_deviation_pct {
            context.rhr_delta = Some((pct / 50.0).clamp(-1.0, 1.0) as f32);
        }

        // Source ID from device
        context.source_ids = vec![format!(
            "{}-{}",
            canonical.vendor.as_str(),
            canonical.device_id
        )];

        self.bio_daily_context_last = Some(context);
    }

    /// Get the last captured bio daily context
    pub fn get_bio_context(&self) -> Option<&BioDailyContext> {
        self.bio_daily_context_last.as_ref()
    }

    /// Get current baseline values
    pub fn get_baselines(&self) -> Baselines {
        Baselines {
            hrv_baseline_ms: Self::rolling_average(&self.hrv_values),
            rhr_baseline_bpm: Self::rolling_average(&self.rhr_values),
            sleep_baseline_minutes: Self::rolling_average(&self.sleep_duration_values),
            sleep_efficiency_baseline: Self::rolling_average(&self.sleep_efficiency_values),
            baseline_days: self.hrv_values.len().max(self.rhr_values.len()) as u32,
        }
    }

    /// Calculate deviation from baseline as percentage
    fn calculate_deviation(&self, current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
        match (current, baseline) {
            (Some(curr), Some(base)) if base > 0.0 => Some(((curr - base) / base) * 100.0),
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

    /// Load baseline store from JSON with version migration
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut store: Self = serde_json::from_str(json)?;

        // Migrate from V1 to V2 if needed
        if store.version < 2 {
            // V1 blobs don't have bio_daily_context_last, which defaults to None
            // Just update the version
            store.version = BASELINE_STORE_VERSION;
        }

        Ok(store)
    }

    /// Serialize baseline store to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CanonicalActivity, CanonicalRecovery, CanonicalSleep, CanonicalWearSignals,
        NormalizedSignals, Vendor,
    };
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_derived(hrv: f64, rhr: f64, sleep_min: f64) -> DerivedSignals {
        let canonical = CanonicalWearSignals {
            vendor: Vendor::Whoop,
            date: "2024-01-15".to_string(),
            device_id: "test".to_string(),
            timezone: "UTC".to_string(),
            observed_at: Utc::now(),
            sleep: CanonicalSleep {
                total_sleep_minutes: Some(sleep_min),
                time_in_bed_minutes: Some(sleep_min + 30.0),
                ..Default::default()
            },
            recovery: CanonicalRecovery {
                hrv_rmssd_ms: Some(hrv),
                resting_hr_bpm: Some(rhr),
                ..Default::default()
            },
            activity: CanonicalActivity::default(),
            vendor_raw: HashMap::new(),
        };

        let normalized = NormalizedSignals {
            canonical,
            sleep_score: Some(0.8),
            recovery_score: Some(0.75),
            strain_score: None,
            coverage: 0.8,
            quality_flags: vec![],
        };

        DerivedSignals {
            normalized,
            sleep_efficiency: Some(sleep_min / (sleep_min + 30.0)),
            sleep_fragmentation: None,
            deep_sleep_ratio: None,
            rem_sleep_ratio: None,
            normalized_load: None,
        }
    }

    #[test]
    fn test_baseline_accumulation() {
        let mut store = BaselineStore::new(7);

        // Add 7 days of data
        for i in 0..7 {
            let hrv = 60.0 + (i as f64);
            let derived = make_derived(hrv, 55.0, 420.0);
            store.update_and_contextualize(derived);
        }

        let baselines = store.get_baselines();
        assert!(baselines.hrv_baseline_ms.is_some());
        // Average of 60, 61, 62, 63, 64, 65, 66 = 63
        assert!((baselines.hrv_baseline_ms.unwrap() - 63.0).abs() < 0.001);
        assert_eq!(baselines.baseline_days, 7);
    }

    #[test]
    fn test_baseline_window_rolling() {
        let mut store = BaselineStore::new(3);

        // Add 5 days - only last 3 should be kept
        for i in 0..5 {
            let hrv = 60.0 + (i as f64) * 10.0; // 60, 70, 80, 90, 100
            let derived = make_derived(hrv, 55.0, 420.0);
            store.update_and_contextualize(derived);
        }

        let baselines = store.get_baselines();
        // Only 80, 90, 100 should be in window, average = 90
        assert!((baselines.hrv_baseline_ms.unwrap() - 90.0).abs() < 0.001);
        assert_eq!(baselines.baseline_days, 3);
    }

    #[test]
    fn test_deviation_calculation() {
        let mut store = BaselineStore::new(7);

        // Build baseline with HRV = 60
        for _ in 0..7 {
            let derived = make_derived(60.0, 55.0, 420.0);
            store.update_and_contextualize(derived);
        }

        // New day with HRV = 72 (20% above baseline)
        let derived = make_derived(72.0, 55.0, 420.0);
        let contextual = store.update_and_contextualize(derived);

        assert!(contextual.hrv_deviation_pct.is_some());
        // 72 vs baseline ~60.86 (rolling after 7 days of 60)
        // But before update, baseline was exactly 60, so deviation = (72-60)/60 * 100 = 20%
        let expected = ((72.0 - 60.0) / 60.0) * 100.0;
        assert!((contextual.hrv_deviation_pct.unwrap() - expected).abs() < 0.1);
    }

    #[test]
    fn test_serialization() {
        let mut store = BaselineStore::new(7);
        let derived = make_derived(65.0, 55.0, 420.0);
        store.update_and_contextualize(derived);

        let json = store.to_json().unwrap();
        let loaded = BaselineStore::from_json(&json).unwrap();

        let orig_baselines = store.get_baselines();
        let loaded_baselines = loaded.get_baselines();

        assert_eq!(
            orig_baselines.hrv_baseline_ms,
            loaded_baselines.hrv_baseline_ms
        );
    }

    #[test]
    fn test_bio_context_capture() {
        let mut store = BaselineStore::new(7);

        // Process data to capture bio context
        let derived = make_derived(65.0, 55.0, 420.0);
        store.update_and_contextualize(derived);

        let bio_ctx = store.get_bio_context();
        assert!(bio_ctx.is_some());

        let ctx = bio_ctx.unwrap();
        assert!(ctx.sleep_quality.is_some());
        assert!((ctx.sleep_quality.unwrap() - 0.8).abs() < 0.01);
        assert!(ctx.recovery.is_some());
        assert!((ctx.recovery.unwrap() - 0.75).abs() < 0.01);
        assert!(!ctx.source_ids.is_empty());
        assert!(ctx.source_ids[0].starts_with("whoop-"));
    }

    #[test]
    fn test_bio_context_serialization() {
        let mut store = BaselineStore::new(7);
        let derived = make_derived(65.0, 55.0, 420.0);
        store.update_and_contextualize(derived);

        let json = store.to_json().unwrap();
        let loaded = BaselineStore::from_json(&json).unwrap();

        // Bio context should persist
        let loaded_ctx = loaded.get_bio_context();
        assert!(loaded_ctx.is_some());
        let ctx = loaded_ctx.unwrap();
        assert!(ctx.sleep_quality.is_some());
        assert!(ctx.recovery.is_some());
    }

    #[test]
    fn test_backward_compatible_loading() {
        // Simulate a V1 JSON blob (no version field, no bio_daily_context_last)
        let v1_json = r#"{
            "hrv_values": [60.0, 62.0, 64.0],
            "rhr_values": [55.0, 54.0, 53.0],
            "sleep_duration_values": [420.0, 430.0],
            "sleep_efficiency_values": [0.9, 0.92],
            "window_size": 14
        }"#;

        let loaded = BaselineStore::from_json(v1_json).unwrap();

        // Should load successfully with bio_daily_context_last = None
        assert!(loaded.get_bio_context().is_none());

        // Baselines should be preserved
        let baselines = loaded.get_baselines();
        assert!(baselines.hrv_baseline_ms.is_some());
        assert!((baselines.hrv_baseline_ms.unwrap() - 62.0).abs() < 0.01);
    }

    #[test]
    fn test_hrv_delta_normalization() {
        let mut store = BaselineStore::new(7);

        // Build baseline first
        for _ in 0..7 {
            let derived = make_derived(60.0, 55.0, 420.0);
            store.update_and_contextualize(derived);
        }

        // Process data with deviation
        let derived = make_derived(72.0, 55.0, 420.0); // 20% above baseline
        store.update_and_contextualize(derived);

        let ctx = store.get_bio_context().unwrap();
        // 20% deviation / 50 = 0.4
        assert!(ctx.hrv_delta.is_some());
        assert!((ctx.hrv_delta.unwrap() - 0.4).abs() < 0.1);
    }
}
