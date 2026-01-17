//! Garmin vendor adapter
//!
//! Parses Garmin API payloads and maps them to canonical wear signals.

use crate::error::ComputeError;
use crate::types::{
    CanonicalActivity, CanonicalRecovery, CanonicalSleep, CanonicalWearSignals, Vendor,
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use std::collections::HashMap;

use super::VendorPayloadAdapter;

/// Garmin payload adapter
pub struct GarminAdapter;

impl VendorPayloadAdapter for GarminAdapter {
    fn parse(
        &self,
        raw_json: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<CanonicalWearSignals>, ComputeError> {
        let payload: GarminPayload = serde_json::from_str(raw_json)?;
        let mut signals = Vec::new();

        // Group data by date
        let mut by_date: HashMap<String, DayData> = HashMap::new();

        // Process daily summaries
        for summary in payload.dailies.unwrap_or_default() {
            let date = summary.calendar_date.clone();
            let entry = by_date.entry(date.clone()).or_insert_with(|| DayData {
                date,
                daily: None,
                sleep: None,
            });
            entry.daily = Some(summary);
        }

        // Process sleep records
        for sleep in payload.sleep.unwrap_or_default() {
            let date = sleep.calendar_date.clone();
            let entry = by_date.entry(date.clone()).or_insert_with(|| DayData {
                date,
                daily: None,
                sleep: None,
            });
            entry.sleep = Some(sleep);
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

/// Internal structure to group Garmin data by date
struct DayData {
    date: String,
    daily: Option<GarminDaily>,
    sleep: Option<GarminSleep>,
}

fn convert_day_to_canonical(
    day: DayData,
    timezone: &str,
    device_id: &str,
) -> Result<CanonicalWearSignals, ComputeError> {
    let observed_at = Utc::now();

    // Build canonical sleep
    let sleep = if let Some(s) = &day.sleep {
        let start_time = s
            .sleep_start_timestamp_gmt
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());
        let end_time = s
            .sleep_end_timestamp_gmt
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        CanonicalSleep {
            start_time,
            end_time,
            time_in_bed_minutes: s
                .sleep_time_seconds
                .map(|secs| secs as f64 / 60.0)
                .or_else(|| {
                    // Calculate from start/end if available
                    match (start_time, end_time) {
                        (Some(start), Some(end)) => {
                            Some((end - start).num_minutes() as f64)
                        }
                        _ => None,
                    }
                }),
            total_sleep_minutes: s.sleep_time_seconds.map(|secs| secs as f64 / 60.0),
            awake_minutes: s.awake_sleep_seconds.map(|secs| secs as f64 / 60.0),
            light_sleep_minutes: s.light_sleep_seconds.map(|secs| secs as f64 / 60.0),
            deep_sleep_minutes: s.deep_sleep_seconds.map(|secs| secs as f64 / 60.0),
            rem_sleep_minutes: s.rem_sleep_seconds.map(|secs| secs as f64 / 60.0),
            awakenings: s.awake_count,
            latency_minutes: None, // Garmin doesn't provide sleep latency directly
            vendor_sleep_score: s.sleep_scores.as_ref().and_then(|sc| sc.overall_score),
            respiratory_rate: s.avg_sleep_respiration,
        }
    } else {
        CanonicalSleep::default()
    };

    // Build canonical recovery (from daily summary)
    let recovery = if let Some(d) = &day.daily {
        CanonicalRecovery {
            hrv_rmssd_ms: d.resting_heart_rate_hrv, // Garmin provides HRV in some endpoints
            resting_hr_bpm: d.resting_heart_rate.map(|hr| hr as f64),
            vendor_recovery_score: d.body_battery_charged_value.map(|bb| bb as f64), // Body Battery as recovery proxy
            skin_temp_deviation_c: None, // Not available in basic Garmin API
            spo2_percentage: d.avg_spo2_value,
        }
    } else {
        CanonicalRecovery::default()
    };

    // Build canonical activity (from daily summary)
    let activity = if let Some(d) = &day.daily {
        CanonicalActivity {
            vendor_strain_score: d.training_load_balance, // Garmin's training load
            calories: d.total_kilocalories.map(|c| c as f64),
            active_calories: d.active_kilocalories.map(|c| c as f64),
            average_hr_bpm: d.average_heart_rate.map(|hr| hr as f64),
            max_hr_bpm: d.max_heart_rate.map(|hr| hr as f64),
            distance_meters: d.total_distance_meters.map(|d| d as f64),
            steps: d.total_steps,
            active_minutes: d
                .moderate_intensity_minutes
                .map(|m| m as f64)
                .and_then(|m| d.vigorous_intensity_minutes.map(|v| m + (v as f64))),
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
    if let Some(d) = &day.daily {
        vendor_raw.insert(
            "daily".to_string(),
            serde_json::to_value(d).unwrap_or(serde_json::Value::Null),
        );
    }

    Ok(CanonicalWearSignals {
        vendor: Vendor::Garmin,
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

// Garmin API response structures

#[derive(Debug, Deserialize)]
struct GarminPayload {
    dailies: Option<Vec<GarminDaily>>,
    sleep: Option<Vec<GarminSleep>>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GarminDaily {
    calendar_date: String,
    total_steps: Option<u32>,
    total_distance_meters: Option<i64>,
    total_kilocalories: Option<i32>,
    active_kilocalories: Option<i32>,
    resting_heart_rate: Option<i32>,
    resting_heart_rate_hrv: Option<f64>,
    average_heart_rate: Option<i32>,
    max_heart_rate: Option<i32>,
    avg_spo2_value: Option<f64>,
    body_battery_charged_value: Option<i32>,
    body_battery_drained_value: Option<i32>,
    training_load_balance: Option<f64>,
    moderate_intensity_minutes: Option<i32>,
    vigorous_intensity_minutes: Option<i32>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GarminSleep {
    calendar_date: String,
    sleep_start_timestamp_gmt: Option<i64>,
    sleep_end_timestamp_gmt: Option<i64>,
    sleep_time_seconds: Option<i64>,
    awake_sleep_seconds: Option<i64>,
    light_sleep_seconds: Option<i64>,
    deep_sleep_seconds: Option<i64>,
    rem_sleep_seconds: Option<i64>,
    awake_count: Option<u32>,
    avg_sleep_respiration: Option<f64>,
    sleep_scores: Option<GarminSleepScores>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GarminSleepScores {
    overall_score: Option<f64>,
    quality_score: Option<f64>,
    recovery_score: Option<f64>,
    restfulness_score: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_garmin_payload() {
        let json = r#"{
            "dailies": [{
                "calendarDate": "2024-01-15",
                "totalSteps": 8500,
                "totalDistanceMeters": 6500,
                "totalKilocalories": 2200,
                "activeKilocalories": 450,
                "restingHeartRate": 55,
                "averageHeartRate": 68,
                "maxHeartRate": 145,
                "avgSpo2Value": 96.5,
                "bodyBatteryChargedValue": 72,
                "trainingLoadBalance": 45.5,
                "moderateIntensityMinutes": 30,
                "vigorousIntensityMinutes": 15
            }],
            "sleep": [{
                "calendarDate": "2024-01-15",
                "sleepStartTimestampGmt": 1705357800000,
                "sleepEndTimestampGmt": 1705386600000,
                "sleepTimeSeconds": 25200,
                "awakeSleepSeconds": 1800,
                "lightSleepSeconds": 10800,
                "deepSleepSeconds": 6300,
                "remSleepSeconds": 6300,
                "awakeCount": 2,
                "avgSleepRespiration": 13.5,
                "sleepScores": {
                    "overallScore": 78.0,
                    "qualityScore": 80.0,
                    "recoveryScore": 75.0
                }
            }]
        }"#;

        let adapter = GarminAdapter;
        let signals = adapter
            .parse(json, "America/Los_Angeles", "garmin-device-456")
            .unwrap();

        assert_eq!(signals.len(), 1);
        let sig = &signals[0];
        assert_eq!(sig.vendor, Vendor::Garmin);
        assert_eq!(sig.date, "2024-01-15");
        assert!(sig.sleep.total_sleep_minutes.is_some());
        assert_eq!(sig.sleep.total_sleep_minutes.unwrap(), 420.0); // 25200 secs = 420 min
        assert!(sig.activity.steps.is_some());
        assert_eq!(sig.activity.steps.unwrap(), 8500);
        assert!(sig.recovery.resting_hr_bpm.is_some());
        assert_eq!(sig.recovery.resting_hr_bpm.unwrap(), 55.0);
    }
}
