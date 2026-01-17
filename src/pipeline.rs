//! Pipeline orchestration
//!
//! This module provides the public API for Synheart Flux.
//! It orchestrates the full pipeline from raw vendor JSON to HSI output.

use crate::adapters::{GarminAdapter, VendorPayloadAdapter, WhoopAdapter};
use crate::baseline::BaselineStore;
use crate::encoder::HsiEncoder;
use crate::error::ComputeError;
use crate::features::FeatureDeriver;
use crate::normalizer::Normalizer;

/// Convert raw WHOOP JSON payload to HSI-compliant daily payloads.
///
/// # Arguments
/// * `raw_json` - Raw WHOOP API response JSON
/// * `timezone` - User's timezone (e.g., "America/New_York")
/// * `device_id` - Unique device identifier
///
/// # Returns
/// Vector of HSI JSON payloads (one per day in the input)
///
/// # Example
/// ```ignore
/// let hsi_payloads = whoop_to_hsi_daily(
///     whoop_json,
///     "America/New_York".to_string(),
///     "device-123".to_string()
/// )?;
/// ```
pub fn whoop_to_hsi_daily(
    raw_json: String,
    timezone: String,
    device_id: String,
) -> Result<Vec<String>, ComputeError> {
    let adapter = WhoopAdapter;
    process_vendor_payload(&adapter, &raw_json, &timezone, &device_id)
}

/// Convert raw Garmin JSON payload to HSI-compliant daily payloads.
///
/// # Arguments
/// * `raw_json` - Raw Garmin API response JSON
/// * `timezone` - User's timezone (e.g., "America/Los_Angeles")
/// * `device_id` - Unique device identifier
///
/// # Returns
/// Vector of HSI JSON payloads (one per day in the input)
///
/// # Example
/// ```ignore
/// let hsi_payloads = garmin_to_hsi_daily(
///     garmin_json,
///     "America/Los_Angeles".to_string(),
///     "garmin-device-456".to_string()
/// )?;
/// ```
pub fn garmin_to_hsi_daily(
    raw_json: String,
    timezone: String,
    device_id: String,
) -> Result<Vec<String>, ComputeError> {
    let adapter = GarminAdapter;
    process_vendor_payload(&adapter, &raw_json, &timezone, &device_id)
}

/// Process vendor payload through the full pipeline.
///
/// Pipeline stages:
/// 1. VendorPayloadAdapter - Parse and map to canonical signals
/// 2. Normalizer - Normalize units and scales
/// 3. FeatureDeriver - Compute derived features
/// 4. BaselineStore - Apply baseline context
/// 5. HsiEncoder - Encode to HSI JSON
fn process_vendor_payload(
    adapter: &dyn VendorPayloadAdapter,
    raw_json: &str,
    timezone: &str,
    device_id: &str,
) -> Result<Vec<String>, ComputeError> {
    // Stage 1: Parse vendor payload to canonical signals
    let canonical_signals = adapter.parse(raw_json, timezone, device_id)?;

    if canonical_signals.is_empty() {
        return Ok(Vec::new());
    }

    // Initialize baseline store and encoder
    let mut baseline_store = BaselineStore::default();
    let encoder = HsiEncoder::new();

    let mut hsi_payloads = Vec::new();

    // Process each day's signals through the pipeline
    for canonical in canonical_signals {
        // Stage 2: Normalize signals
        let normalized = Normalizer::normalize(&canonical);

        // Stage 3: Derive features
        let derived = FeatureDeriver::derive(normalized);

        // Stage 4: Apply baselines and create contextual signals
        let contextual = baseline_store.update_and_contextualize(derived);

        // Stage 5: Encode to HSI JSON
        let hsi_json = encoder.encode_to_json(&contextual)?;
        hsi_payloads.push(hsi_json);
    }

    Ok(hsi_payloads)
}

/// Stateful processor for incremental processing with persistent baselines.
///
/// Use this when you need to maintain baselines across multiple API calls.
pub struct FluxProcessor {
    baseline_store: BaselineStore,
    encoder: HsiEncoder,
}

impl Default for FluxProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl FluxProcessor {
    /// Create a new processor with default settings
    pub fn new() -> Self {
        Self {
            baseline_store: BaselineStore::default(),
            encoder: HsiEncoder::new(),
        }
    }

    /// Create a processor with a specific baseline window size
    pub fn with_baseline_window(window_days: usize) -> Self {
        Self {
            baseline_store: BaselineStore::new(window_days),
            encoder: HsiEncoder::new(),
        }
    }

    /// Load baseline state from JSON
    pub fn load_baselines(&mut self, json: &str) -> Result<(), ComputeError> {
        self.baseline_store =
            BaselineStore::from_json(json).map_err(|e| ComputeError::ParseError(e.to_string()))?;
        Ok(())
    }

    /// Save baseline state to JSON
    pub fn save_baselines(&self) -> Result<String, ComputeError> {
        self.baseline_store
            .to_json()
            .map_err(|e| ComputeError::EncodingError(e.to_string()))
    }

    /// Process WHOOP payload with persistent baselines
    pub fn process_whoop(
        &mut self,
        raw_json: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<String>, ComputeError> {
        let adapter = WhoopAdapter;
        self.process_with_adapter(&adapter, raw_json, timezone, device_id)
    }

    /// Process Garmin payload with persistent baselines
    pub fn process_garmin(
        &mut self,
        raw_json: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<String>, ComputeError> {
        let adapter = GarminAdapter;
        self.process_with_adapter(&adapter, raw_json, timezone, device_id)
    }

    fn process_with_adapter(
        &mut self,
        adapter: &dyn VendorPayloadAdapter,
        raw_json: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<String>, ComputeError> {
        let canonical_signals = adapter.parse(raw_json, timezone, device_id)?;

        let mut hsi_payloads = Vec::new();

        for canonical in canonical_signals {
            let normalized = Normalizer::normalize(&canonical);
            let derived = FeatureDeriver::derive(normalized);
            let contextual = self.baseline_store.update_and_contextualize(derived);
            let hsi_json = self.encoder.encode_to_json(&contextual)?;
            hsi_payloads.push(hsi_json);
        }

        Ok(hsi_payloads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_whoop_json() -> &'static str {
        r#"{
            "sleep": [{
                "id": 1,
                "start": "2024-01-15T22:30:00.000Z",
                "end": "2024-01-16T06:30:00.000Z",
                "score": {
                    "stage_summary": {
                        "total_in_bed_time_milli": 28800000,
                        "total_awake_time_milli": 1800000,
                        "total_light_sleep_time_milli": 12600000,
                        "total_slow_wave_sleep_time_milli": 7200000,
                        "total_rem_sleep_time_milli": 7200000,
                        "total_sleep_time_milli": 27000000,
                        "disturbance_count": 3
                    },
                    "sleep_performance_percentage": 85.0,
                    "sleep_efficiency_percentage": 93.75,
                    "respiratory_rate": 14.5
                }
            }],
            "recovery": [{
                "cycle_id": 1,
                "created_at": "2024-01-15T06:30:00.000Z",
                "score": {
                    "recovery_score": 75.0,
                    "resting_heart_rate": 52.0,
                    "hrv_rmssd_milli": 65.0,
                    "spo2_percentage": 97.0
                }
            }],
            "cycle": [{
                "id": 1,
                "start": "2024-01-15T06:30:00.000Z",
                "end": "2024-01-15T22:30:00.000Z",
                "score": {
                    "strain": 12.5,
                    "kilojoule": 8500.0,
                    "average_heart_rate": 72.0,
                    "max_heart_rate": 165.0
                }
            }]
        }"#
    }

    fn sample_garmin_json() -> &'static str {
        r#"{
            "dailies": [{
                "calendarDate": "2024-01-15",
                "totalSteps": 8500,
                "totalDistanceMeters": 6500,
                "totalKilocalories": 2200,
                "activeKilocalories": 450,
                "restingHeartRate": 55,
                "averageHeartRate": 68,
                "maxHeartRate": 145,
                "bodyBatteryChargedValue": 72,
                "trainingLoadBalance": 45.5
            }],
            "sleep": [{
                "calendarDate": "2024-01-15",
                "sleepTimeSeconds": 25200,
                "awakeSleepSeconds": 1800,
                "lightSleepSeconds": 10800,
                "deepSleepSeconds": 6300,
                "remSleepSeconds": 6300,
                "sleepScores": {
                    "overallScore": 78.0
                }
            }]
        }"#
    }

    #[test]
    fn test_whoop_to_hsi_daily() {
        let result = whoop_to_hsi_daily(
            sample_whoop_json().to_string(),
            "America/New_York".to_string(),
            "test-device".to_string(),
        );

        assert!(result.is_ok());
        let payloads = result.unwrap();
        assert_eq!(payloads.len(), 1);

        // Verify JSON is valid and contains expected fields
        let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
        assert_eq!(payload["hsi_version"], "1.0.0");
        assert_eq!(payload["producer"]["name"], "synheart-flux");
        assert_eq!(payload["provenance"]["source_vendor"], "whoop");

        // Verify sleep data
        let sleep = &payload["windows"][0]["sleep"];
        assert_eq!(sleep["duration_minutes"], 450.0);
        assert!(sleep["efficiency"].as_f64().unwrap() > 0.9);

        // Verify physiology data
        let physiology = &payload["windows"][0]["physiology"];
        assert_eq!(physiology["hrv_rmssd_ms"], 65.0);
        assert_eq!(physiology["resting_hr_bpm"], 52.0);
    }

    #[test]
    fn test_garmin_to_hsi_daily() {
        let result = garmin_to_hsi_daily(
            sample_garmin_json().to_string(),
            "America/Los_Angeles".to_string(),
            "garmin-device".to_string(),
        );

        assert!(result.is_ok());
        let payloads = result.unwrap();
        assert_eq!(payloads.len(), 1);

        let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
        assert_eq!(payload["provenance"]["source_vendor"], "garmin");

        // Verify activity data
        let activity = &payload["windows"][0]["activity"];
        assert_eq!(activity["steps"], 8500);
        assert_eq!(activity["calories"], 2200.0);
    }

    #[test]
    fn test_flux_processor_persistent_baselines() {
        let mut processor = FluxProcessor::with_baseline_window(7);

        // Process first day
        let result1 = processor.process_whoop(
            sample_whoop_json(),
            "America/New_York",
            "test-device",
        );
        assert!(result1.is_ok());

        // Process same data again - baselines should be updated
        let result2 = processor.process_whoop(
            sample_whoop_json(),
            "America/New_York",
            "test-device",
        );
        assert!(result2.is_ok());

        let payload: serde_json::Value = serde_json::from_str(&result2.unwrap()[0]).unwrap();
        let baseline = &payload["windows"][0]["baseline"];

        // After 2 days of same data, baseline should be established
        assert!(baseline["hrv_ms"].as_f64().is_some());
        assert_eq!(baseline["days_in_baseline"], 2);
    }

    #[test]
    fn test_baseline_serialization() {
        let mut processor = FluxProcessor::new();

        // Process data
        processor
            .process_whoop(sample_whoop_json(), "America/New_York", "test-device")
            .unwrap();

        // Save baselines
        let saved = processor.save_baselines().unwrap();

        // Create new processor and load baselines
        let mut new_processor = FluxProcessor::new();
        new_processor.load_baselines(&saved).unwrap();

        // Process more data - baselines should be preserved
        let result = new_processor
            .process_whoop(sample_whoop_json(), "America/New_York", "test-device")
            .unwrap();

        let payload: serde_json::Value = serde_json::from_str(&result[0]).unwrap();
        let baseline = &payload["windows"][0]["baseline"];

        // Baselines should show 2 days (1 from saved + 1 new)
        assert_eq!(baseline["days_in_baseline"], 2);
    }

    #[test]
    fn test_empty_payload() {
        let result = whoop_to_hsi_daily(
            r#"{"sleep": [], "recovery": [], "cycle": []}"#.to_string(),
            "UTC".to_string(),
            "device".to_string(),
        );

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_invalid_json() {
        let result = whoop_to_hsi_daily(
            "not valid json".to_string(),
            "UTC".to_string(),
            "device".to_string(),
        );

        assert!(result.is_err());
    }
}
