//! WHOOP vendor adapter
//!
//! Parses WHOOP API payloads and maps them to canonical wear signals.

use crate::error::ComputeError;
use crate::types::{
    CanonicalActivity, CanonicalRecovery, CanonicalSleep, CanonicalWearSignals, Vendor,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::HashMap;

use super::VendorPayloadAdapter;

/// WHOOP payload adapter
pub struct WhoopAdapter;

impl VendorPayloadAdapter for WhoopAdapter {
    fn parse(
        &self,
        raw_json: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<CanonicalWearSignals>, ComputeError> {
        let payload: WhoopPayload = serde_json::from_str(raw_json)?;
        let mut signals = Vec::new();

        // Group data by date
        let mut by_date: HashMap<String, DayData> = HashMap::new();

        // Process sleep records
        for sleep in payload.sleep.unwrap_or_default() {
            if let Some(date) = extract_date_from_whoop_time(&sleep.start) {
                let entry = by_date.entry(date.clone()).or_insert_with(|| DayData {
                    date,
                    sleep: None,
                    recovery: None,
                    cycle: None,
                });
                entry.sleep = Some(sleep);
            }
        }

        // Process recovery records
        for recovery in payload.recovery.unwrap_or_default() {
            if let Some(date) = extract_date_from_whoop_time(&recovery.created_at) {
                let entry = by_date.entry(date.clone()).or_insert_with(|| DayData {
                    date,
                    sleep: None,
                    recovery: None,
                    cycle: None,
                });
                entry.recovery = Some(recovery);
            }
        }

        // Process cycle (strain) records
        for cycle in payload.cycle.unwrap_or_default() {
            if let Some(date) = extract_date_from_whoop_time(&cycle.start) {
                let entry = by_date.entry(date.clone()).or_insert_with(|| DayData {
                    date,
                    sleep: None,
                    recovery: None,
                    cycle: None,
                });
                entry.cycle = Some(cycle);
            }
        }

        // Convert grouped data to canonical signals
        for (_date, day) in by_date {
            let canonical = convert_day_to_canonical(day, timezone, device_id)?;
            signals.push(canonical);
        }

        // Sort by date
        signals.sort_by(|a, b| a.date.cmp(&b.date));

        Ok(signals)
    }
}

/// Internal structure to group WHOOP data by date
struct DayData {
    date: String,
    sleep: Option<WhoopSleep>,
    recovery: Option<WhoopRecovery>,
    cycle: Option<WhoopCycle>,
}

fn convert_day_to_canonical(
    day: DayData,
    timezone: &str,
    device_id: &str,
) -> Result<CanonicalWearSignals, ComputeError> {
    let observed_at = Utc::now(); // Use current time as observation time

    // Build canonical sleep
    let sleep = if let Some(s) = &day.sleep {
        CanonicalSleep {
            start_time: parse_whoop_time(&s.start),
            end_time: parse_whoop_time(&s.end),
            time_in_bed_minutes: s.score.as_ref().and_then(|sc| {
                sc.stage_summary
                    .as_ref()
                    .map(|ss| ss.total_in_bed_time_milli as f64 / 60_000.0)
            }),
            total_sleep_minutes: s.score.as_ref().and_then(|sc| {
                sc.stage_summary
                    .as_ref()
                    .map(|ss| ss.total_sleep_time_milli as f64 / 60_000.0)
            }),
            awake_minutes: s.score.as_ref().and_then(|sc| {
                sc.stage_summary
                    .as_ref()
                    .map(|ss| ss.total_awake_time_milli as f64 / 60_000.0)
            }),
            light_sleep_minutes: s.score.as_ref().and_then(|sc| {
                sc.stage_summary
                    .as_ref()
                    .map(|ss| ss.total_light_sleep_time_milli as f64 / 60_000.0)
            }),
            deep_sleep_minutes: s.score.as_ref().and_then(|sc| {
                sc.stage_summary
                    .as_ref()
                    .map(|ss| ss.total_slow_wave_sleep_time_milli as f64 / 60_000.0)
            }),
            rem_sleep_minutes: s.score.as_ref().and_then(|sc| {
                sc.stage_summary
                    .as_ref()
                    .map(|ss| ss.total_rem_sleep_time_milli as f64 / 60_000.0)
            }),
            awakenings: s
                .score
                .as_ref()
                .and_then(|sc| sc.stage_summary.as_ref().map(|ss| ss.disturbance_count)),
            latency_minutes: s
                .score
                .as_ref()
                .and_then(|sc| sc.sleep_latency_time_milli.map(|l| l as f64 / 60_000.0)),
            vendor_sleep_score: s
                .score
                .as_ref()
                .and_then(|sc| sc.sleep_performance_percentage),
            respiratory_rate: s.score.as_ref().and_then(|sc| sc.respiratory_rate),
        }
    } else {
        CanonicalSleep::default()
    };

    // Build canonical recovery
    let recovery = if let Some(r) = &day.recovery {
        CanonicalRecovery {
            hrv_rmssd_ms: r.score.as_ref().and_then(|sc| sc.hrv_rmssd_milli),
            resting_hr_bpm: r.score.as_ref().and_then(|sc| sc.resting_heart_rate),
            vendor_recovery_score: r.score.as_ref().and_then(|sc| sc.recovery_score),
            skin_temp_deviation_c: r.score.as_ref().and_then(|sc| sc.skin_temp_celsius),
            spo2_percentage: r.score.as_ref().and_then(|sc| sc.spo2_percentage),
        }
    } else {
        CanonicalRecovery::default()
    };

    // Build canonical activity
    let activity = if let Some(c) = &day.cycle {
        CanonicalActivity {
            vendor_strain_score: c.score.as_ref().and_then(|sc| sc.strain),
            calories: c
                .score
                .as_ref()
                .and_then(|sc| sc.kilojoule.map(|kj| kj * 0.239006)), // kJ to kcal
            active_calories: None, // WHOOP doesn't separate active vs total in basic API
            average_hr_bpm: c.score.as_ref().and_then(|sc| sc.average_heart_rate),
            max_hr_bpm: c.score.as_ref().and_then(|sc| sc.max_heart_rate),
            distance_meters: None, // Not in WHOOP cycle data
            steps: None,           // WHOOP doesn't track steps
            active_minutes: None,  // Could be derived from workouts
        }
    } else {
        CanonicalActivity::default()
    };

    // Build vendor_raw with original data
    let mut vendor_raw = HashMap::new();
    if let Some(s) = &day.sleep {
        vendor_raw.insert(
            "sleep".to_string(),
            serde_json::to_value(s).unwrap_or(serde_json::Value::Null),
        );
    }
    if let Some(r) = &day.recovery {
        vendor_raw.insert(
            "recovery".to_string(),
            serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
        );
    }
    if let Some(c) = &day.cycle {
        vendor_raw.insert(
            "cycle".to_string(),
            serde_json::to_value(c).unwrap_or(serde_json::Value::Null),
        );
    }

    Ok(CanonicalWearSignals {
        vendor: Vendor::Whoop,
        date: day.date,
        device_id: device_id.to_string(),
        timezone: timezone.to_string(),
        observed_at,
        sleep,
        recovery,
        activity,
        vendor_raw,
    })
}

fn parse_whoop_time(time_str: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(time_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn extract_date_from_whoop_time(time_str: &str) -> Option<String> {
    // WHOOP times are in ISO 8601 format: "2024-01-15T08:30:00.000Z"
    NaiveDate::parse_from_str(&time_str[..10], "%Y-%m-%d")
        .ok()
        .map(|d| d.format("%Y-%m-%d").to_string())
}

// WHOOP API response structures

#[derive(Debug, Deserialize)]
struct WhoopPayload {
    sleep: Option<Vec<WhoopSleep>>,
    recovery: Option<Vec<WhoopRecovery>>,
    cycle: Option<Vec<WhoopCycle>>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopSleep {
    id: Option<i64>,
    start: String,
    end: String,
    score: Option<WhoopSleepScore>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopSleepScore {
    stage_summary: Option<WhoopStageSummary>,
    sleep_performance_percentage: Option<f64>,
    sleep_consistency_percentage: Option<f64>,
    sleep_efficiency_percentage: Option<f64>,
    sleep_latency_time_milli: Option<i64>,
    respiratory_rate: Option<f64>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopStageSummary {
    total_in_bed_time_milli: i64,
    total_awake_time_milli: i64,
    total_light_sleep_time_milli: i64,
    total_slow_wave_sleep_time_milli: i64,
    total_rem_sleep_time_milli: i64,
    total_sleep_time_milli: i64,
    disturbance_count: u32,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopRecovery {
    cycle_id: Option<i64>,
    created_at: String,
    score: Option<WhoopRecoveryScore>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopRecoveryScore {
    recovery_score: Option<f64>,
    resting_heart_rate: Option<f64>,
    hrv_rmssd_milli: Option<f64>,
    spo2_percentage: Option<f64>,
    skin_temp_celsius: Option<f64>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopCycle {
    id: Option<i64>,
    start: String,
    end: Option<String>,
    score: Option<WhoopCycleScore>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct WhoopCycleScore {
    strain: Option<f64>,
    kilojoule: Option<f64>,
    average_heart_rate: Option<f64>,
    max_heart_rate: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_whoop_payload() {
        let json = r#"{
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
        }"#;

        let adapter = WhoopAdapter;
        let signals = adapter
            .parse(json, "America/New_York", "device-123")
            .unwrap();

        assert_eq!(signals.len(), 1);
        let sig = &signals[0];
        assert_eq!(sig.vendor, Vendor::Whoop);
        assert_eq!(sig.date, "2024-01-15");
        assert!(sig.sleep.total_sleep_minutes.is_some());
        assert_eq!(sig.sleep.total_sleep_minutes.unwrap(), 450.0); // 27000000ms = 450 min
        assert!(sig.recovery.hrv_rmssd_ms.is_some());
        assert_eq!(sig.recovery.hrv_rmssd_ms.unwrap(), 65.0);
        assert!(sig.activity.vendor_strain_score.is_some());
        assert_eq!(sig.activity.vendor_strain_score.unwrap(), 12.5);
    }
}
