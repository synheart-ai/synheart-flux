//! HSI encoding
//!
//! This module encodes contextual signals into HSI-compliant JSON payloads.
//! Ensures all required fields are present and properly formatted.

use crate::error::ComputeError;
use crate::types::{
    ContextualSignals, HsiActivity, HsiBaseline, HsiDailyWindow, HsiPayload, HsiPhysiology,
    HsiProducer, HsiProvenance, HsiQuality, HsiSleep,
};
use crate::{FLUX_VERSION, PRODUCER_NAME};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

/// Current HSI schema version
pub const HSI_VERSION: &str = "1.0.0";

/// HSI encoder for producing compliant JSON payloads
pub struct HsiEncoder {
    instance_id: String,
}

impl Default for HsiEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl HsiEncoder {
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

    /// Encode contextual signals into an HSI payload
    pub fn encode(&self, signals: &ContextualSignals) -> Result<HsiPayload, ComputeError> {
        let canonical = &signals.derived.normalized.canonical;
        let computed_at = Utc::now();

        // Build producer metadata
        let producer = HsiProducer {
            name: PRODUCER_NAME.to_string(),
            version: FLUX_VERSION.to_string(),
            instance_id: self.instance_id.clone(),
        };

        // Build provenance
        let provenance = HsiProvenance {
            source_vendor: canonical.vendor.as_str().to_string(),
            source_device_id: canonical.device_id.clone(),
            observed_at_utc: canonical.observed_at.to_rfc3339(),
            computed_at_utc: computed_at.to_rfc3339(),
        };

        // Build quality metrics
        let quality = self.build_quality(signals, computed_at);

        // Build daily window
        let window = self.build_daily_window(signals);

        Ok(HsiPayload {
            hsi_version: HSI_VERSION.to_string(),
            producer,
            provenance,
            quality,
            windows: vec![window],
        })
    }

    /// Encode to JSON string
    pub fn encode_to_json(&self, signals: &ContextualSignals) -> Result<String, ComputeError> {
        let payload = self.encode(signals)?;
        serde_json::to_string_pretty(&payload).map_err(ComputeError::JsonError)
    }

    fn build_quality(
        &self,
        signals: &ContextualSignals,
        computed_at: chrono::DateTime<Utc>,
    ) -> HsiQuality {
        let canonical = &signals.derived.normalized.canonical;
        let freshness_sec = (computed_at - canonical.observed_at).num_seconds();

        // Calculate confidence based on coverage and baseline availability
        let base_confidence = signals.derived.normalized.coverage;
        let baseline_bonus = if signals.baselines.baseline_days >= 7 {
            0.1
        } else {
            0.0
        };
        let confidence = (base_confidence + baseline_bonus).min(1.0);

        let flags: Vec<String> = signals
            .derived
            .normalized
            .quality_flags
            .iter()
            .map(|f| format!("{f:?}").to_lowercase())
            .collect();

        HsiQuality {
            coverage: signals.derived.normalized.coverage,
            freshness_sec,
            confidence,
            flags,
        }
    }

    fn build_daily_window(&self, signals: &ContextualSignals) -> HsiDailyWindow {
        let canonical = &signals.derived.normalized.canonical;
        let derived = &signals.derived;
        let normalized = &derived.normalized;

        // Build sleep namespace
        let sleep = HsiSleep {
            duration_minutes: canonical.sleep.total_sleep_minutes,
            efficiency: derived.sleep_efficiency,
            fragmentation: derived.sleep_fragmentation,
            deep_ratio: derived.deep_sleep_ratio,
            rem_ratio: derived.rem_sleep_ratio,
            latency_minutes: canonical.sleep.latency_minutes,
            score: normalized.sleep_score,
            vendor: self.extract_vendor_sleep(canonical),
        };

        // Build physiology namespace
        let physiology = HsiPhysiology {
            hrv_rmssd_ms: canonical.recovery.hrv_rmssd_ms,
            resting_hr_bpm: canonical.recovery.resting_hr_bpm,
            respiratory_rate: canonical.sleep.respiratory_rate,
            spo2_percentage: canonical.recovery.spo2_percentage,
            recovery_score: normalized.recovery_score,
            vendor: self.extract_vendor_recovery(canonical),
        };

        // Build activity namespace
        let activity = HsiActivity {
            strain_score: normalized.strain_score,
            normalized_load: derived.normalized_load,
            calories: canonical.activity.calories,
            active_calories: canonical.activity.active_calories,
            steps: canonical.activity.steps,
            active_minutes: canonical.activity.active_minutes,
            distance_meters: canonical.activity.distance_meters,
            vendor: self.extract_vendor_activity(canonical),
        };

        // Build baseline namespace
        let baseline = HsiBaseline {
            hrv_ms: signals.baselines.hrv_baseline_ms,
            resting_hr_bpm: signals.baselines.rhr_baseline_bpm,
            sleep_duration_minutes: signals.baselines.sleep_baseline_minutes,
            sleep_efficiency: signals.baselines.sleep_efficiency_baseline,
            hrv_deviation_pct: signals.hrv_deviation_pct,
            rhr_deviation_pct: signals.rhr_deviation_pct,
            sleep_deviation_pct: signals.sleep_duration_deviation_pct,
            days_in_baseline: signals.baselines.baseline_days,
        };

        HsiDailyWindow {
            date: canonical.date.clone(),
            timezone: canonical.timezone.clone(),
            sleep,
            physiology,
            activity,
            baseline,
        }
    }

    fn extract_vendor_sleep(
        &self,
        canonical: &crate::types::CanonicalWearSignals,
    ) -> HashMap<String, serde_json::Value> {
        let mut vendor = HashMap::new();

        // Include vendor-specific raw sleep score
        if let Some(score) = canonical.sleep.vendor_sleep_score {
            vendor.insert(
                format!("{}_sleep_score", canonical.vendor.as_str()),
                serde_json::Value::from(score),
            );
        }

        // Include original vendor data if present
        if let Some(raw) = canonical.vendor_raw.get("sleep") {
            vendor.insert("raw".to_string(), raw.clone());
        }

        vendor
    }

    fn extract_vendor_recovery(
        &self,
        canonical: &crate::types::CanonicalWearSignals,
    ) -> HashMap<String, serde_json::Value> {
        let mut vendor = HashMap::new();

        if let Some(score) = canonical.recovery.vendor_recovery_score {
            vendor.insert(
                format!("{}_recovery_score", canonical.vendor.as_str()),
                serde_json::Value::from(score),
            );
        }

        if let Some(raw) = canonical.vendor_raw.get("recovery") {
            vendor.insert("raw".to_string(), raw.clone());
        }

        vendor
    }

    fn extract_vendor_activity(
        &self,
        canonical: &crate::types::CanonicalWearSignals,
    ) -> HashMap<String, serde_json::Value> {
        let mut vendor = HashMap::new();

        if let Some(score) = canonical.activity.vendor_strain_score {
            vendor.insert(
                format!("{}_strain_score", canonical.vendor.as_str()),
                serde_json::Value::from(score),
            );
        }

        if let Some(raw) = canonical.vendor_raw.get("cycle") {
            vendor.insert("raw".to_string(), raw.clone());
        }
        if let Some(raw) = canonical.vendor_raw.get("daily") {
            vendor.insert("raw".to_string(), raw.clone());
        }

        vendor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Baselines, CanonicalActivity, CanonicalRecovery, CanonicalSleep, CanonicalWearSignals,
        DerivedSignals, NormalizedSignals, Vendor,
    };

    fn make_test_contextual() -> ContextualSignals {
        let canonical = CanonicalWearSignals {
            vendor: Vendor::Whoop,
            date: "2024-01-15".to_string(),
            device_id: "test-device".to_string(),
            timezone: "America/New_York".to_string(),
            observed_at: Utc::now(),
            sleep: CanonicalSleep {
                total_sleep_minutes: Some(420.0),
                time_in_bed_minutes: Some(480.0),
                deep_sleep_minutes: Some(84.0),
                rem_sleep_minutes: Some(105.0),
                vendor_sleep_score: Some(85.0),
                latency_minutes: Some(12.0),
                respiratory_rate: Some(14.5),
                ..Default::default()
            },
            recovery: CanonicalRecovery {
                hrv_rmssd_ms: Some(65.0),
                resting_hr_bpm: Some(55.0),
                vendor_recovery_score: Some(75.0),
                spo2_percentage: Some(97.0),
                ..Default::default()
            },
            activity: CanonicalActivity {
                vendor_strain_score: Some(12.5),
                calories: Some(2200.0),
                active_calories: Some(450.0),
                steps: Some(8500),
                ..Default::default()
            },
            vendor_raw: HashMap::new(),
        };

        let normalized = NormalizedSignals {
            canonical,
            sleep_score: Some(0.85),
            recovery_score: Some(0.75),
            strain_score: Some(0.595),
            coverage: 0.9,
            quality_flags: vec![],
        };

        let derived = DerivedSignals {
            normalized,
            sleep_efficiency: Some(0.875),
            sleep_fragmentation: Some(0.05),
            deep_sleep_ratio: Some(0.2),
            rem_sleep_ratio: Some(0.25),
            normalized_load: Some(0.79),
        };

        let baselines = Baselines {
            hrv_baseline_ms: Some(62.0),
            rhr_baseline_bpm: Some(54.0),
            sleep_baseline_minutes: Some(410.0),
            sleep_efficiency_baseline: Some(0.86),
            baseline_days: 14,
        };

        ContextualSignals {
            derived,
            baselines,
            hrv_deviation_pct: Some(4.8),
            rhr_deviation_pct: Some(1.9),
            sleep_duration_deviation_pct: Some(2.4),
        }
    }

    #[test]
    fn test_encode_hsi_payload() {
        let signals = make_test_contextual();
        let encoder = HsiEncoder::with_instance_id("test-instance".to_string());
        let payload = encoder.encode(&signals).unwrap();

        assert_eq!(payload.hsi_version, HSI_VERSION);
        assert_eq!(payload.producer.name, PRODUCER_NAME);
        assert_eq!(payload.producer.version, FLUX_VERSION);
        assert_eq!(payload.producer.instance_id, "test-instance");

        assert_eq!(payload.provenance.source_vendor, "whoop");
        assert_eq!(payload.provenance.source_device_id, "test-device");

        assert!(payload.quality.coverage > 0.8);
        assert!(payload.quality.confidence > 0.8);

        assert_eq!(payload.windows.len(), 1);
        let window = &payload.windows[0];
        assert_eq!(window.date, "2024-01-15");
        assert_eq!(window.timezone, "America/New_York");

        // Check sleep
        assert_eq!(window.sleep.duration_minutes, Some(420.0));
        assert_eq!(window.sleep.efficiency, Some(0.875));
        assert_eq!(window.sleep.score, Some(0.85));

        // Check physiology
        assert_eq!(window.physiology.hrv_rmssd_ms, Some(65.0));
        assert_eq!(window.physiology.resting_hr_bpm, Some(55.0));
        assert_eq!(window.physiology.recovery_score, Some(0.75));

        // Check activity
        assert_eq!(window.activity.strain_score, Some(0.595));
        assert_eq!(window.activity.steps, Some(8500));

        // Check baseline
        assert_eq!(window.baseline.hrv_ms, Some(62.0));
        assert_eq!(window.baseline.hrv_deviation_pct, Some(4.8));
        assert_eq!(window.baseline.days_in_baseline, 14);
    }

    #[test]
    fn test_encode_to_json() {
        let signals = make_test_contextual();
        let encoder = HsiEncoder::new();
        let json = encoder.encode_to_json(&signals).unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("hsi_version").is_some());
        assert!(parsed.get("producer").is_some());
        assert!(parsed.get("provenance").is_some());
        assert!(parsed.get("quality").is_some());
        assert!(parsed.get("windows").is_some());
    }
}
