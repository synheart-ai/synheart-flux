//! Core types for the Synheart Flux pipeline
//!
//! This module defines the data structures that flow through each stage of the
//! pipeline: canonical signals, normalized signals, derived signals, and HSI output.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Vendor identifier for provenance tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Whoop,
    Garmin,
}

impl Vendor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Vendor::Whoop => "whoop",
            Vendor::Garmin => "garmin",
        }
    }
}

/// Sleep stage classification (vendor-agnostic)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SleepStage {
    Awake,
    Light,
    Deep,
    Rem,
    Unknown,
}

/// Canonical sleep data extracted from vendor payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalSleep {
    /// Sleep start time (UTC)
    pub start_time: Option<DateTime<Utc>>,
    /// Sleep end time (UTC)
    pub end_time: Option<DateTime<Utc>>,
    /// Total time in bed (minutes)
    pub time_in_bed_minutes: Option<f64>,
    /// Total sleep duration (minutes)
    pub total_sleep_minutes: Option<f64>,
    /// Time awake during sleep period (minutes)
    pub awake_minutes: Option<f64>,
    /// Light sleep duration (minutes)
    pub light_sleep_minutes: Option<f64>,
    /// Deep sleep duration (minutes)
    pub deep_sleep_minutes: Option<f64>,
    /// REM sleep duration (minutes)
    pub rem_sleep_minutes: Option<f64>,
    /// Number of awakenings
    pub awakenings: Option<u32>,
    /// Sleep latency - time to fall asleep (minutes)
    pub latency_minutes: Option<f64>,
    /// Vendor-provided sleep score (raw, vendor-specific scale)
    pub vendor_sleep_score: Option<f64>,
    /// Respiratory rate during sleep (breaths per minute)
    pub respiratory_rate: Option<f64>,
}

impl Default for CanonicalSleep {
    fn default() -> Self {
        Self {
            start_time: None,
            end_time: None,
            time_in_bed_minutes: None,
            total_sleep_minutes: None,
            awake_minutes: None,
            light_sleep_minutes: None,
            deep_sleep_minutes: None,
            rem_sleep_minutes: None,
            awakenings: None,
            latency_minutes: None,
            vendor_sleep_score: None,
            respiratory_rate: None,
        }
    }
}

/// Canonical recovery/physiology data extracted from vendor payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRecovery {
    /// Heart rate variability (ms, RMSSD)
    pub hrv_rmssd_ms: Option<f64>,
    /// Resting heart rate (bpm)
    pub resting_hr_bpm: Option<f64>,
    /// Vendor-provided recovery score (raw, vendor-specific scale)
    pub vendor_recovery_score: Option<f64>,
    /// Skin temperature deviation (celsius)
    pub skin_temp_deviation_c: Option<f64>,
    /// Blood oxygen saturation (percentage, 0-100)
    pub spo2_percentage: Option<f64>,
}

impl Default for CanonicalRecovery {
    fn default() -> Self {
        Self {
            hrv_rmssd_ms: None,
            resting_hr_bpm: None,
            vendor_recovery_score: None,
            skin_temp_deviation_c: None,
            spo2_percentage: None,
        }
    }
}

/// Canonical activity/strain data extracted from vendor payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalActivity {
    /// Vendor-provided strain/load score (raw, vendor-specific scale)
    pub vendor_strain_score: Option<f64>,
    /// Total calories burned
    pub calories: Option<f64>,
    /// Active calories burned
    pub active_calories: Option<f64>,
    /// Average heart rate during activity (bpm)
    pub average_hr_bpm: Option<f64>,
    /// Maximum heart rate during activity (bpm)
    pub max_hr_bpm: Option<f64>,
    /// Total distance (meters)
    pub distance_meters: Option<f64>,
    /// Number of steps
    pub steps: Option<u32>,
    /// Active duration (minutes)
    pub active_minutes: Option<f64>,
}

impl Default for CanonicalActivity {
    fn default() -> Self {
        Self {
            vendor_strain_score: None,
            calories: None,
            active_calories: None,
            average_hr_bpm: None,
            max_hr_bpm: None,
            distance_meters: None,
            steps: None,
            active_minutes: None,
        }
    }
}

/// Canonical wear signals - vendor-agnostic representation of wearable data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalWearSignals {
    /// Source vendor
    pub vendor: Vendor,
    /// Date this data represents (YYYY-MM-DD)
    pub date: String,
    /// Device identifier
    pub device_id: String,
    /// Timezone of the user
    pub timezone: String,
    /// When the data was observed/recorded by the vendor
    pub observed_at: DateTime<Utc>,
    /// Sleep data
    pub sleep: CanonicalSleep,
    /// Recovery/physiology data
    pub recovery: CanonicalRecovery,
    /// Activity/strain data
    pub activity: CanonicalActivity,
    /// Raw vendor-specific metrics preserved for transparency
    pub vendor_raw: HashMap<String, serde_json::Value>,
}

/// Normalized signals with consistent units and scales
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSignals {
    /// Source canonical signals
    pub canonical: CanonicalWearSignals,
    /// Normalized sleep score (0-1)
    pub sleep_score: Option<f64>,
    /// Normalized recovery score (0-1)
    pub recovery_score: Option<f64>,
    /// Normalized strain/load score (0-1)
    pub strain_score: Option<f64>,
    /// Data completeness (0-1)
    pub coverage: f64,
    /// Flags for missing or estimated data
    pub quality_flags: Vec<QualityFlag>,
}

/// Quality flag indicating data issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityFlag {
    MissingSleepData,
    MissingRecoveryData,
    MissingActivityData,
    MissingHrv,
    MissingRestingHr,
    EstimatedValue,
    PartialDayData,
    LowConfidence,
}

/// Derived features computed from normalized signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedSignals {
    /// Source normalized signals
    pub normalized: NormalizedSignals,
    /// Sleep efficiency (actual sleep / time in bed, 0-1)
    pub sleep_efficiency: Option<f64>,
    /// Sleep fragmentation index (0-1, higher = more fragmented)
    pub sleep_fragmentation: Option<f64>,
    /// Deep sleep ratio (deep sleep / total sleep, 0-1)
    pub deep_sleep_ratio: Option<f64>,
    /// REM sleep ratio (REM / total sleep, 0-1)
    pub rem_sleep_ratio: Option<f64>,
    /// Normalized load (strain adjusted by recovery)
    pub normalized_load: Option<f64>,
}

/// Baseline values for relative interpretation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Baselines {
    /// Baseline HRV (rolling average, ms)
    pub hrv_baseline_ms: Option<f64>,
    /// Baseline resting HR (rolling average, bpm)
    pub rhr_baseline_bpm: Option<f64>,
    /// Baseline sleep duration (rolling average, minutes)
    pub sleep_baseline_minutes: Option<f64>,
    /// Baseline sleep efficiency (rolling average, 0-1)
    pub sleep_efficiency_baseline: Option<f64>,
    /// Number of days used to compute baselines
    pub baseline_days: u32,
}

/// Contextual signals with baseline comparisons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualSignals {
    /// Source derived signals
    pub derived: DerivedSignals,
    /// Current baselines
    pub baselines: Baselines,
    /// HRV deviation from baseline (percentage)
    pub hrv_deviation_pct: Option<f64>,
    /// RHR deviation from baseline (percentage)
    pub rhr_deviation_pct: Option<f64>,
    /// Sleep duration deviation from baseline (percentage)
    pub sleep_duration_deviation_pct: Option<f64>,
}

/// HSI producer metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiProducer {
    pub name: String,
    pub version: String,
    pub instance_id: String,
}

/// HSI provenance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiProvenance {
    pub source_vendor: String,
    pub source_device_id: String,
    pub observed_at_utc: String,
    pub computed_at_utc: String,
}

/// HSI quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiQuality {
    /// Data completeness (0-1)
    pub coverage: f64,
    /// Seconds since observation
    pub freshness_sec: i64,
    /// Overall confidence in the signals (0-1)
    pub confidence: f64,
    /// Quality flags
    pub flags: Vec<String>,
}

/// HSI sleep namespace signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiSleep {
    pub duration_minutes: Option<f64>,
    pub efficiency: Option<f64>,
    pub fragmentation: Option<f64>,
    pub deep_ratio: Option<f64>,
    pub rem_ratio: Option<f64>,
    pub latency_minutes: Option<f64>,
    pub score: Option<f64>,
    pub vendor: HashMap<String, serde_json::Value>,
}

/// HSI physiology namespace signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiPhysiology {
    pub hrv_rmssd_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub respiratory_rate: Option<f64>,
    pub spo2_percentage: Option<f64>,
    pub recovery_score: Option<f64>,
    pub vendor: HashMap<String, serde_json::Value>,
}

/// HSI activity namespace signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiActivity {
    pub strain_score: Option<f64>,
    pub normalized_load: Option<f64>,
    pub calories: Option<f64>,
    pub active_calories: Option<f64>,
    pub steps: Option<u32>,
    pub active_minutes: Option<f64>,
    pub distance_meters: Option<f64>,
    pub vendor: HashMap<String, serde_json::Value>,
}

/// HSI baseline namespace signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiBaseline {
    pub hrv_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub sleep_duration_minutes: Option<f64>,
    pub sleep_efficiency: Option<f64>,
    pub hrv_deviation_pct: Option<f64>,
    pub rhr_deviation_pct: Option<f64>,
    pub sleep_deviation_pct: Option<f64>,
    pub days_in_baseline: u32,
}

/// HSI daily window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiDailyWindow {
    pub date: String,
    pub timezone: String,
    pub sleep: HsiSleep,
    pub physiology: HsiPhysiology,
    pub activity: HsiActivity,
    pub baseline: HsiBaseline,
}

/// Complete HSI payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsiPayload {
    pub hsi_version: String,
    pub producer: HsiProducer,
    pub provenance: HsiProvenance,
    pub quality: HsiQuality,
    pub windows: Vec<HsiDailyWindow>,
}
