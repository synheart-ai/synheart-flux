//! Feature derivation
//!
//! This module derives higher-order features from normalized signals:
//! - Sleep efficiency and fragmentation
//! - Sleep stage ratios
//! - Load normalization

use crate::types::{DerivedSignals, NormalizedSignals};

/// Feature deriver for computing derived signals
pub struct FeatureDeriver;

impl FeatureDeriver {
    /// Derive features from normalized signals
    pub fn derive(normalized: NormalizedSignals) -> DerivedSignals {
        let sleep_efficiency = compute_sleep_efficiency(&normalized);
        let sleep_fragmentation = compute_sleep_fragmentation(&normalized);
        let deep_sleep_ratio = compute_deep_sleep_ratio(&normalized);
        let rem_sleep_ratio = compute_rem_sleep_ratio(&normalized);
        let normalized_load = compute_normalized_load(&normalized);

        DerivedSignals {
            normalized,
            sleep_efficiency,
            sleep_fragmentation,
            deep_sleep_ratio,
            rem_sleep_ratio,
            normalized_load,
        }
    }
}

/// Calculate sleep efficiency: actual sleep time / time in bed
fn compute_sleep_efficiency(signals: &NormalizedSignals) -> Option<f64> {
    let sleep = &signals.canonical.sleep;

    match (sleep.total_sleep_minutes, sleep.time_in_bed_minutes) {
        (Some(sleep_min), Some(bed_min)) if bed_min > 0.0 => {
            Some((sleep_min / bed_min).clamp(0.0, 1.0))
        }
        _ => None,
    }
}

/// Calculate sleep fragmentation index based on awakenings and sleep duration
/// Higher values indicate more fragmented sleep
fn compute_sleep_fragmentation(signals: &NormalizedSignals) -> Option<f64> {
    let sleep = &signals.canonical.sleep;

    match (sleep.awakenings, sleep.total_sleep_minutes) {
        (Some(awakenings), Some(sleep_min)) if sleep_min > 0.0 => {
            // Fragmentation index: awakenings per hour of sleep, normalized to 0-1
            // Assuming 0 awakenings = 0 fragmentation, 6+ awakenings/hour = max fragmentation
            let awakenings_per_hour = (awakenings as f64) / (sleep_min / 60.0);
            Some((awakenings_per_hour / 6.0).clamp(0.0, 1.0))
        }
        _ => {
            // Alternative: use awake time ratio if awakenings not available
            match (sleep.awake_minutes, sleep.time_in_bed_minutes) {
                (Some(awake_min), Some(bed_min)) if bed_min > 0.0 => {
                    Some((awake_min / bed_min).clamp(0.0, 1.0))
                }
                _ => None,
            }
        }
    }
}

/// Calculate deep sleep ratio: deep sleep / total sleep
fn compute_deep_sleep_ratio(signals: &NormalizedSignals) -> Option<f64> {
    let sleep = &signals.canonical.sleep;

    match (sleep.deep_sleep_minutes, sleep.total_sleep_minutes) {
        (Some(deep_min), Some(total_min)) if total_min > 0.0 => {
            Some((deep_min / total_min).clamp(0.0, 1.0))
        }
        _ => None,
    }
}

/// Calculate REM sleep ratio: REM sleep / total sleep
fn compute_rem_sleep_ratio(signals: &NormalizedSignals) -> Option<f64> {
    let sleep = &signals.canonical.sleep;

    match (sleep.rem_sleep_minutes, sleep.total_sleep_minutes) {
        (Some(rem_min), Some(total_min)) if total_min > 0.0 => {
            Some((rem_min / total_min).clamp(0.0, 1.0))
        }
        _ => None,
    }
}

/// Calculate normalized load: strain adjusted by recovery
/// Higher recovery allows for higher sustainable load
fn compute_normalized_load(signals: &NormalizedSignals) -> Option<f64> {
    match (signals.strain_score, signals.recovery_score) {
        (Some(strain), Some(recovery)) if recovery > 0.0 => {
            // Normalized load: how much of recovery capacity was used
            // strain / recovery gives relative load intensity
            Some((strain / recovery).clamp(0.0, 2.0))
        }
        (Some(strain), None) => {
            // Without recovery, use raw strain
            Some(strain)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CanonicalActivity, CanonicalRecovery, CanonicalSleep, CanonicalWearSignals, Vendor,
    };
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_test_normalized() -> NormalizedSignals {
        let canonical = CanonicalWearSignals {
            vendor: Vendor::Whoop,
            date: "2024-01-15".to_string(),
            device_id: "test-device".to_string(),
            timezone: "UTC".to_string(),
            observed_at: Utc::now(),
            sleep: CanonicalSleep {
                total_sleep_minutes: Some(420.0),
                time_in_bed_minutes: Some(480.0),
                deep_sleep_minutes: Some(84.0),
                rem_sleep_minutes: Some(105.0),
                awake_minutes: Some(60.0),
                awakenings: Some(3),
                ..Default::default()
            },
            recovery: CanonicalRecovery {
                hrv_rmssd_ms: Some(65.0),
                resting_hr_bpm: Some(55.0),
                vendor_recovery_score: Some(75.0),
                ..Default::default()
            },
            activity: CanonicalActivity {
                vendor_strain_score: Some(12.5),
                calories: Some(2200.0),
                ..Default::default()
            },
            vendor_raw: HashMap::new(),
        };

        NormalizedSignals {
            canonical,
            sleep_score: Some(0.85),
            recovery_score: Some(0.75),
            strain_score: Some(0.595),
            coverage: 0.9,
            quality_flags: vec![],
        }
    }

    #[test]
    fn test_sleep_efficiency() {
        let normalized = make_test_normalized();
        let derived = FeatureDeriver::derive(normalized);

        assert!(derived.sleep_efficiency.is_some());
        // 420 / 480 = 0.875
        assert!((derived.sleep_efficiency.unwrap() - 0.875).abs() < 0.001);
    }

    #[test]
    fn test_sleep_fragmentation() {
        let normalized = make_test_normalized();
        let derived = FeatureDeriver::derive(normalized);

        assert!(derived.sleep_fragmentation.is_some());
        // 3 awakenings in 7 hours = 0.43/hour, normalized by 6 = 0.071
        let expected = (3.0 / 7.0) / 6.0;
        assert!((derived.sleep_fragmentation.unwrap() - expected).abs() < 0.01);
    }

    #[test]
    fn test_deep_sleep_ratio() {
        let normalized = make_test_normalized();
        let derived = FeatureDeriver::derive(normalized);

        assert!(derived.deep_sleep_ratio.is_some());
        // 84 / 420 = 0.2
        assert!((derived.deep_sleep_ratio.unwrap() - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_rem_sleep_ratio() {
        let normalized = make_test_normalized();
        let derived = FeatureDeriver::derive(normalized);

        assert!(derived.rem_sleep_ratio.is_some());
        // 105 / 420 = 0.25
        assert!((derived.rem_sleep_ratio.unwrap() - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_normalized_load() {
        let normalized = make_test_normalized();
        let derived = FeatureDeriver::derive(normalized);

        assert!(derived.normalized_load.is_some());
        // strain 0.595 / recovery 0.75 = 0.793
        assert!((derived.normalized_load.unwrap() - 0.793).abs() < 0.01);
    }
}
