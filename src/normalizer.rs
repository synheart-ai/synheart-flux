//! Signal normalization
//!
//! This module normalizes canonical wear signals into consistent units and scales.
//! - Vendor scores normalized to 0-1
//! - Coverage and quality flags computed
//! - Missing data detection

use crate::types::{CanonicalWearSignals, NormalizedSignals, QualityFlag, Vendor};

/// Normalizer for converting canonical signals to normalized signals
pub struct Normalizer;

impl Normalizer {
    /// Normalize canonical signals
    pub fn normalize(signals: &CanonicalWearSignals) -> NormalizedSignals {
        let mut quality_flags = Vec::new();
        let mut coverage_count = 0;
        let total_fields = 6; // Key fields we track for coverage

        // Check sleep data
        let sleep_score = normalize_sleep_score(signals);
        if signals.sleep.total_sleep_minutes.is_some() {
            coverage_count += 1;
        } else {
            quality_flags.push(QualityFlag::MissingSleepData);
        }

        // Check recovery data
        let recovery_score = normalize_recovery_score(signals);
        if signals.recovery.hrv_rmssd_ms.is_some() {
            coverage_count += 1;
        } else {
            quality_flags.push(QualityFlag::MissingHrv);
        }

        if signals.recovery.resting_hr_bpm.is_some() {
            coverage_count += 1;
        } else {
            quality_flags.push(QualityFlag::MissingRestingHr);
        }

        if recovery_score.is_some() {
            coverage_count += 1;
        } else if signals.recovery.vendor_recovery_score.is_none() {
            quality_flags.push(QualityFlag::MissingRecoveryData);
        }

        // Check activity data
        let strain_score = normalize_strain_score(signals);
        if strain_score.is_some() {
            coverage_count += 1;
        } else {
            quality_flags.push(QualityFlag::MissingActivityData);
        }

        // Check for additional activity coverage
        if signals.activity.calories.is_some() || signals.activity.steps.is_some() {
            coverage_count += 1;
        }

        let coverage = (coverage_count as f64) / (total_fields as f64);

        NormalizedSignals {
            canonical: signals.clone(),
            sleep_score,
            recovery_score,
            strain_score,
            coverage,
            quality_flags,
        }
    }
}

/// Normalize vendor sleep score to 0-1 scale
fn normalize_sleep_score(signals: &CanonicalWearSignals) -> Option<f64> {
    signals.sleep.vendor_sleep_score.map(|score| {
        match signals.vendor {
            Vendor::Whoop => {
                // WHOOP sleep performance is 0-100%
                (score / 100.0).clamp(0.0, 1.0)
            }
            Vendor::Garmin => {
                // Garmin sleep score is 0-100
                (score / 100.0).clamp(0.0, 1.0)
            }
        }
    })
}

/// Normalize vendor recovery score to 0-1 scale
fn normalize_recovery_score(signals: &CanonicalWearSignals) -> Option<f64> {
    signals.recovery.vendor_recovery_score.map(|score| {
        match signals.vendor {
            Vendor::Whoop => {
                // WHOOP recovery is 0-100%
                (score / 100.0).clamp(0.0, 1.0)
            }
            Vendor::Garmin => {
                // Garmin Body Battery is 0-100
                (score / 100.0).clamp(0.0, 1.0)
            }
        }
    })
}

/// Normalize vendor strain/load score to 0-1 scale
fn normalize_strain_score(signals: &CanonicalWearSignals) -> Option<f64> {
    signals.activity.vendor_strain_score.map(|score| {
        match signals.vendor {
            Vendor::Whoop => {
                // WHOOP strain is 0-21 scale
                (score / 21.0).clamp(0.0, 1.0)
            }
            Vendor::Garmin => {
                // Garmin training load balance varies; normalize assuming typical range 0-150
                (score / 150.0).clamp(0.0, 1.0)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CanonicalActivity, CanonicalRecovery, CanonicalSleep};
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_test_signals(vendor: Vendor) -> CanonicalWearSignals {
        CanonicalWearSignals {
            vendor,
            date: "2024-01-15".to_string(),
            device_id: "test-device".to_string(),
            timezone: "UTC".to_string(),
            observed_at: Utc::now(),
            sleep: CanonicalSleep {
                total_sleep_minutes: Some(420.0),
                vendor_sleep_score: Some(85.0),
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
        }
    }

    #[test]
    fn test_normalize_whoop_scores() {
        let signals = make_test_signals(Vendor::Whoop);
        let normalized = Normalizer::normalize(&signals);

        assert!(normalized.sleep_score.is_some());
        assert!((normalized.sleep_score.unwrap() - 0.85).abs() < 0.001);

        assert!(normalized.recovery_score.is_some());
        assert!((normalized.recovery_score.unwrap() - 0.75).abs() < 0.001);

        assert!(normalized.strain_score.is_some());
        assert!((normalized.strain_score.unwrap() - 12.5 / 21.0).abs() < 0.001);
    }

    #[test]
    fn test_coverage_calculation() {
        let mut signals = make_test_signals(Vendor::Whoop);
        let normalized = Normalizer::normalize(&signals);

        // Full data should have high coverage
        assert!(normalized.coverage > 0.8);

        // Remove some data
        signals.sleep.total_sleep_minutes = None;
        signals.recovery.hrv_rmssd_ms = None;
        let normalized = Normalizer::normalize(&signals);

        // Coverage should be lower
        assert!(normalized.coverage < 0.8);
        assert!(normalized
            .quality_flags
            .contains(&QualityFlag::MissingSleepData));
        assert!(normalized.quality_flags.contains(&QualityFlag::MissingHrv));
    }
}
