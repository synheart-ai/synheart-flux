//! Pipeline orchestration
//!
//! This module provides the public API for Synheart Flux.
//! It orchestrates the full pipeline from raw vendor JSON to HSI output.

use crate::adapters::{GarminAdapter, VendorPayloadAdapter, WhoopAdapter};
use crate::baseline::BaselineStore;
use crate::behavior::adapter::{parse_session, session_to_canonical};
use crate::behavior::features::BehaviorFeatureDeriver;
use crate::behavior::normalizer::BehaviorNormalizer;
use crate::behavior::types::{HsiAxes, HsiAxesDomain, HsiPayload, HsiPrivacy, HsiProducer, HsiWindow};
use crate::context::{DecayedBioContext, DEFAULT_DECAY_HALF_LIFE_HOURS};
use crate::encoder::HsiEncoder;
use crate::error::ComputeError;
use crate::features::FeatureDeriver;
use crate::normalizer::Normalizer;
use crate::{FLUX_VERSION, PRODUCER_NAME};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

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

    /// Take a snapshot of current bio context with optional behavior session.
    ///
    /// This is a read-only operation that does NOT mutate baselines.
    /// It produces an HSI 1.0 payload with:
    /// - `axes.context`: Staleness-decayed bio context readings
    /// - `axes.behavior`: Behavior readings (if behavior_session_json provided)
    ///
    /// # Arguments
    /// * `now_utc` - Current time in RFC3339 format (e.g., "2024-01-15T14:00:00Z")
    /// * `timezone` - User's timezone (e.g., "America/New_York")
    /// * `device_id` - Device identifier for the snapshot
    /// * `behavior_session_json` - Optional behavior session JSON to process statelessly
    ///
    /// # Returns
    /// HSI 1.0 JSON payload with context and optionally behavior axes
    pub fn snapshot_now(
        &self,
        now_utc: &str,
        timezone: &str,
        device_id: &str,
        behavior_session_json: Option<&str>,
    ) -> Result<String, ComputeError> {
        // Parse the now_utc timestamp
        let now: DateTime<Utc> = now_utc
            .parse()
            .map_err(|e| ComputeError::ParseError(format!("Invalid now_utc timestamp: {e}")))?;

        // Generate IDs
        let window_id = format!("w_snapshot_{}", Uuid::new_v4().to_string().replace('-', "_"));
        let instance_id = Uuid::new_v4().to_string();

        // Build producer
        let producer = HsiProducer {
            name: PRODUCER_NAME.to_string(),
            version: FLUX_VERSION.to_string(),
            instance_id: Some(instance_id),
        };

        // Build window
        let mut windows = HashMap::new();
        windows.insert(
            window_id.clone(),
            HsiWindow {
                start: now.to_rfc3339(),
                end: now.to_rfc3339(),
                label: Some("snapshot".to_string()),
            },
        );

        // Build context axis from bio context
        let context_domain = self.build_context_domain(&window_id, now)?;

        // Optionally process behavior session (stateless)
        let behavior_domain = if let Some(session_json) = behavior_session_json {
            Some(self.process_behavior_stateless(session_json, &window_id)?)
        } else {
            None
        };

        // Check what domains we have before moving
        let has_context = context_domain.is_some();
        let has_behavior = behavior_domain.is_some();

        // Build axes
        let axes = HsiAxes {
            affect: None,
            engagement: None,
            behavior: behavior_domain,
            context: context_domain,
        };

        // Build privacy
        let privacy = HsiPrivacy {
            contains_pii: false,
            raw_biosignals_allowed: false,
            derived_metrics_allowed: true,
            embedding_allowed: None,
            consent: None,
            purposes: Some(vec!["context_snapshot".to_string()]),
            notes: None,
        };

        // Build metadata
        let mut meta = HashMap::new();
        meta.insert(
            "snapshot_type".to_string(),
            serde_json::Value::String("context_aware".to_string()),
        );
        meta.insert(
            "device_id".to_string(),
            serde_json::Value::String(device_id.to_string()),
        );
        meta.insert(
            "timezone".to_string(),
            serde_json::Value::String(timezone.to_string()),
        );
        if let Some(bio_ctx) = self.baseline_store.get_bio_context() {
            meta.insert(
                "bio_context_age_hours".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(
                        (now - bio_ctx.observed_at_utc).num_seconds() as f64 / 3600.0
                    ).unwrap_or(serde_json::Number::from(0))
                ),
            );
        }

        // Build source
        let source_id = format!("s_{}", device_id.replace('-', "_"));
        let mut sources = HashMap::new();
        sources.insert(
            source_id.clone(),
            crate::behavior::types::HsiSource {
                source_type: crate::behavior::types::HsiSourceType::Derived,
                quality: if has_context || has_behavior { 0.8 } else { 0.5 },
                degraded: !has_context && !has_behavior,
                notes: if !has_context {
                    Some("No bio context available".to_string())
                } else {
                    None
                },
            },
        );

        // Build payload
        let payload = HsiPayload {
            hsi_version: "1.0".to_string(),
            observed_at_utc: now.to_rfc3339(),
            computed_at_utc: Utc::now().to_rfc3339(),
            producer,
            window_ids: vec![window_id],
            windows,
            source_ids: Some(vec![source_id]),
            sources: Some(sources),
            axes: Some(axes),
            privacy,
            meta: Some(meta),
        };

        serde_json::to_string_pretty(&payload).map_err(ComputeError::JsonError)
    }

    /// Build context domain from bio context with staleness decay
    fn build_context_domain(
        &self,
        window_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<HsiAxesDomain>, ComputeError> {
        let bio_ctx = match self.baseline_store.get_bio_context() {
            Some(ctx) => ctx.clone(),
            None => return Ok(None),
        };

        // Estimate base confidence from data availability
        let mut fields_present = 0;
        if bio_ctx.sleep_quality.is_some() { fields_present += 1; }
        if bio_ctx.recovery.is_some() { fields_present += 1; }
        if bio_ctx.hrv_delta.is_some() { fields_present += 1; }
        if bio_ctx.rhr_delta.is_some() { fields_present += 1; }

        let base_confidence = if fields_present == 0 {
            0.5
        } else {
            0.6 + 0.1 * (fields_present as f64)
        };

        // Apply staleness decay
        let decayed = DecayedBioContext::from_context(
            bio_ctx,
            base_confidence,
            now,
            DEFAULT_DECAY_HALF_LIFE_HOURS,
        );

        // Generate HSI readings
        let readings = decayed.to_hsi_readings(window_id);

        if readings.is_empty() {
            Ok(None)
        } else {
            Ok(Some(HsiAxesDomain { readings }))
        }
    }

    /// Process behavior session statelessly (no baseline mutation)
    fn process_behavior_stateless(
        &self,
        session_json: &str,
        window_id: &str,
    ) -> Result<HsiAxesDomain, ComputeError> {
        // Parse and process session
        let session = parse_session(session_json)?;
        let canonical = session_to_canonical(&session)?;
        let normalized = BehaviorNormalizer::normalize(canonical);
        let derived = BehaviorFeatureDeriver::derive(normalized);

        // Build readings directly from derived signals (without baseline context)
        let confidence = derived.normalized.coverage;
        let source_id = format!("s_{}", session.device_id.replace('-', "_"));
        let source_ids = Some(vec![source_id]);

        use crate::behavior::types::{HsiAxisReading, HsiDirection};

        let readings = vec![
            HsiAxisReading {
                axis: "distraction".to_string(),
                score: Some(derived.distraction_score),
                confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: None,
                evidence_source_ids: source_ids.clone(),
                notes: None,
            },
            HsiAxisReading {
                axis: "focus".to_string(),
                score: Some(derived.focus_hint),
                confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: None,
                evidence_source_ids: source_ids.clone(),
                notes: None,
            },
            HsiAxisReading {
                axis: "task_switch_rate".to_string(),
                score: Some(derived.task_switch_rate),
                confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("normalized".to_string()),
                evidence_source_ids: source_ids.clone(),
                notes: None,
            },
            HsiAxisReading {
                axis: "burstiness".to_string(),
                score: Some(derived.burstiness),
                confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::Bidirectional),
                unit: Some("barabasi_index".to_string()),
                evidence_source_ids: source_ids.clone(),
                notes: None,
            },
            HsiAxisReading {
                axis: "interaction_intensity".to_string(),
                score: Some(derived.interaction_intensity.min(1.0)),
                confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("normalized".to_string()),
                evidence_source_ids: source_ids,
                notes: None,
            },
        ];

        Ok(HsiAxesDomain { readings })
    }

    /// Get direct access to the baseline store for snapshot operations
    pub fn get_baseline_store(&self) -> &BaselineStore {
        &self.baseline_store
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
        let result1 =
            processor.process_whoop(sample_whoop_json(), "America/New_York", "test-device");
        assert!(result1.is_ok());

        // Process same data again - baselines should be updated
        let result2 =
            processor.process_whoop(sample_whoop_json(), "America/New_York", "test-device");
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

    #[test]
    fn test_snapshot_now_without_bio_context() {
        let processor = FluxProcessor::new();

        // Snapshot without any prior wearable processing
        let result = processor.snapshot_now(
            "2024-01-15T14:00:00Z",
            "America/New_York",
            "device-1",
            None,
        );

        assert!(result.is_ok());
        let payload: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();

        // Should have HSI 1.0 structure
        assert_eq!(payload["hsi_version"], "1.0");
        assert_eq!(payload["producer"]["name"], "synheart-flux");

        // Context should be absent (no bio data)
        assert!(payload["axes"]["context"].is_null());

        // Source should be degraded
        let sources = payload["sources"].as_object().unwrap();
        let source = sources.values().next().unwrap();
        assert!(source["degraded"].as_bool().unwrap());
    }

    #[test]
    fn test_snapshot_now_with_bio_context() {
        let mut processor = FluxProcessor::new();

        // First, process wearable data to capture bio context
        processor
            .process_whoop(sample_whoop_json(), "America/New_York", "test-device")
            .unwrap();

        // Now take a snapshot
        let result = processor.snapshot_now(
            "2024-01-15T14:00:00Z",
            "America/New_York",
            "device-1",
            None,
        );

        assert!(result.is_ok());
        let payload: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();

        // Context should be present with readings
        let context = &payload["axes"]["context"];
        assert!(context.is_object());

        let readings = context["readings"].as_array().unwrap();
        assert!(!readings.is_empty());

        // Should have bio_freshness reading
        let freshness = readings.iter().find(|r| r["axis"] == "bio_freshness");
        assert!(freshness.is_some());
        assert!(freshness.unwrap()["score"].as_f64().is_some());

        // Should have recovery_context reading
        let recovery = readings.iter().find(|r| r["axis"] == "recovery_context");
        assert!(recovery.is_some());
    }

    #[test]
    fn test_snapshot_now_with_behavior() {
        let mut processor = FluxProcessor::new();

        // Process wearable data first
        processor
            .process_whoop(sample_whoop_json(), "America/New_York", "test-device")
            .unwrap();

        // Behavior session JSON
        let behavior_json = r#"{
            "session_id": "sess-123",
            "device_id": "device-456",
            "timezone": "America/New_York",
            "start_time": "2024-01-15T14:00:00Z",
            "end_time": "2024-01-15T14:30:00Z",
            "events": [
                {"timestamp": "2024-01-15T14:01:00Z", "event_type": "scroll", "scroll": {"velocity": 150.0, "direction": "down", "direction_reversal": false}},
                {"timestamp": "2024-01-15T14:02:00Z", "event_type": "tap", "tap": {"tap_duration_ms": 120, "long_press": false}},
                {"timestamp": "2024-01-15T14:03:00Z", "event_type": "notification", "interruption": {"action": "ignored"}}
            ]
        }"#;

        // Take snapshot with behavior
        let result = processor.snapshot_now(
            "2024-01-15T14:35:00Z",
            "America/New_York",
            "device-1",
            Some(behavior_json),
        );

        assert!(result.is_ok());
        let payload: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();

        // Should have both context and behavior axes
        let axes = &payload["axes"];

        // Context should be present
        assert!(axes["context"].is_object());

        // Behavior should be present with readings
        let behavior = &axes["behavior"];
        assert!(behavior.is_object());

        let readings = behavior["readings"].as_array().unwrap();
        assert!(!readings.is_empty());

        // Should have distraction and focus readings
        assert!(readings.iter().any(|r| r["axis"] == "distraction"));
        assert!(readings.iter().any(|r| r["axis"] == "focus"));
    }

    #[test]
    fn test_snapshot_is_read_only() {
        let mut processor = FluxProcessor::new();

        // Process wearable data
        processor
            .process_whoop(sample_whoop_json(), "America/New_York", "test-device")
            .unwrap();

        // Get baseline count before snapshot
        let baselines_before = processor.save_baselines().unwrap();

        // Take snapshot (should be read-only)
        processor.snapshot_now(
            "2024-01-15T14:00:00Z",
            "America/New_York",
            "device-1",
            None,
        ).unwrap();

        // Baselines should be unchanged
        let baselines_after = processor.save_baselines().unwrap();
        assert_eq!(baselines_before, baselines_after);
    }

    #[test]
    fn test_snapshot_staleness_decay() {
        let mut processor = FluxProcessor::new();

        // Process wearable data (this captures bio context with observed_at = Utc::now())
        processor
            .process_whoop(sample_whoop_json(), "America/New_York", "test-device")
            .unwrap();

        // Get the bio context to know the actual observed_at timestamp
        let bio_ctx = processor.get_baseline_store().get_bio_context().unwrap();
        let observed_at = bio_ctx.observed_at_utc;

        // Take snapshot immediately (high freshness)
        let now_fresh = observed_at.to_rfc3339();
        let result_fresh = processor.snapshot_now(
            &now_fresh, // Same time as observed_at = maximum freshness
            "America/New_York",
            "device-1",
            None,
        ).unwrap();
        let payload_fresh: serde_json::Value = serde_json::from_str(&result_fresh).unwrap();
        let readings_fresh = payload_fresh["axes"]["context"]["readings"].as_array().unwrap();
        let freshness_fresh = readings_fresh.iter()
            .find(|r| r["axis"] == "bio_freshness")
            .unwrap()["score"].as_f64().unwrap();

        // Take snapshot 24 hours later (lower freshness)
        let now_stale = (observed_at + chrono::Duration::hours(24)).to_rfc3339();
        let result_stale = processor.snapshot_now(
            &now_stale, // 24 hours after observed_at
            "America/New_York",
            "device-1",
            None,
        ).unwrap();
        let payload_stale: serde_json::Value = serde_json::from_str(&result_stale).unwrap();
        let readings_stale = payload_stale["axes"]["context"]["readings"].as_array().unwrap();
        let freshness_stale = readings_stale.iter()
            .find(|r| r["axis"] == "bio_freshness")
            .unwrap()["score"].as_f64().unwrap();

        // Freshness should be 1.0 when snapshot time equals observed_at
        assert!((freshness_fresh - 1.0).abs() < 0.001);

        // Freshness should decay over time
        assert!(freshness_fresh > freshness_stale);

        // After 24 hours (2x half-life of 12h), freshness should be ~25%
        assert!(freshness_stale < 0.3);
        assert!(freshness_stale > 0.2); // Should be around 0.25
    }
}
