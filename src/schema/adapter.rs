//! Adapter for converting wear.raw_event.v1 to CanonicalWearSignals
//!
//! This module handles aggregating individual raw events into daily canonical
//! signals that can be processed through the existing Flux pipeline.

use crate::error::ComputeError;
use crate::schema::raw_event::*;
use crate::types::{
    CanonicalActivity, CanonicalRecovery, CanonicalSleep, CanonicalWearSignals, Vendor,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Adapter for converting raw events to canonical signals
pub struct RawEventAdapter;

impl RawEventAdapter {
    /// Parse a JSON string containing an array of RawEvents
    pub fn parse_array(json: &str) -> Result<Vec<RawEvent>, ComputeError> {
        let events: Vec<RawEvent> = serde_json::from_str(json)?;
        Ok(events)
    }

    /// Parse NDJSON (newline-delimited JSON) containing RawEvents
    pub fn parse_ndjson(ndjson: &str) -> Result<Vec<RawEvent>, ComputeError> {
        let mut events = Vec::new();
        for (line_num, line) in ndjson.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<RawEvent>(trimmed) {
                Ok(event) => events.push(event),
                Err(e) => {
                    return Err(ComputeError::ParseError(format!(
                        "Failed to parse line {}: {}",
                        line_num + 1,
                        e
                    )));
                }
            }
        }
        Ok(events)
    }

    /// Convert raw events to canonical daily signals
    ///
    /// Groups events by date and provider, then aggregates into daily canonical signals.
    pub fn to_canonical(
        events: &[RawEvent],
        timezone: &str,
        device_id: &str,
    ) -> Result<Vec<CanonicalWearSignals>, ComputeError> {
        // Group events by (date, provider)
        let mut by_date_provider: HashMap<(String, String), DayAccumulator> = HashMap::new();

        for event in events {
            // Validate each event
            if let Err(e) = event.validate() {
                return Err(ComputeError::ParseError(format!(
                    "Invalid event: {}",
                    e
                )));
            }

            let date = extract_date(&event.timestamp, event.context.as_ref());
            let provider = event.source.provider.as_str().to_string();
            let key = (date, provider);

            let accumulator = by_date_provider.entry(key).or_insert_with(|| {
                DayAccumulator::new(event.source.provider.clone())
            });

            accumulator.add_event(event);
        }

        // Convert accumulators to canonical signals
        let mut signals = Vec::new();
        for ((date, _), accumulator) in by_date_provider {
            let canonical = accumulator.to_canonical(&date, timezone, device_id)?;
            signals.push(canonical);
        }

        // Sort by date
        signals.sort_by(|a, b| a.date.cmp(&b.date));

        Ok(signals)
    }

    /// Validate a batch of events
    pub fn validate_events(events: &[RawEvent]) -> Vec<ValidationResult> {
        events
            .iter()
            .enumerate()
            .map(|(idx, event)| ValidationResult {
                index: idx,
                event_id: event.event_id.clone(),
                result: event.validate().err(),
            })
            .filter(|r| r.result.is_some())
            .collect()
    }
}

/// Result of event validation
#[derive(Debug)]
pub struct ValidationResult {
    pub index: usize,
    pub event_id: Option<String>,
    pub result: Option<ValidationError>,
}

/// Accumulator for aggregating events into a single day
struct DayAccumulator {
    provider: Provider,
    // Sleep data
    sleep_sessions: Vec<SleepData>,
    // Recovery/physiology data
    hrv_readings: Vec<f64>,
    resting_hr_readings: Vec<f64>,
    spo2_readings: Vec<f64>,
    skin_temp_readings: Vec<f64>,
    respiratory_rate_readings: Vec<f64>,
    recovery_score: Option<f64>,
    // Activity data
    strain_score: Option<f64>,
    total_calories: Option<f64>,
    active_calories: Option<f64>,
    total_steps: Option<u32>,
    distance_meters: Option<f64>,
    active_minutes: Option<f64>,
    hr_readings: Vec<f64>,
    max_hr: Option<f64>,
    // Raw vendor data
    vendor_raw: HashMap<String, serde_json::Value>,
}

struct SleepData {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    metrics: HashMap<String, MetricValue>,
}

impl DayAccumulator {
    fn new(provider: Provider) -> Self {
        DayAccumulator {
            provider,
            sleep_sessions: Vec::new(),
            hrv_readings: Vec::new(),
            resting_hr_readings: Vec::new(),
            spo2_readings: Vec::new(),
            skin_temp_readings: Vec::new(),
            respiratory_rate_readings: Vec::new(),
            recovery_score: None,
            strain_score: None,
            total_calories: None,
            active_calories: None,
            total_steps: None,
            distance_meters: None,
            active_minutes: None,
            hr_readings: Vec::new(),
            max_hr: None,
            vendor_raw: HashMap::new(),
        }
    }

    fn add_event(&mut self, event: &RawEvent) {
        // Preserve vendor raw if present
        if let Some(raw) = &event.vendor_raw {
            let key = event.event_id.clone().unwrap_or_else(|| {
                format!("event_{}", self.vendor_raw.len())
            });
            self.vendor_raw.insert(key, raw.clone());
        }

        match &event.payload {
            Payload::Signal { signal } => self.add_signal(signal),
            Payload::Session { session } => self.add_session(session),
            Payload::Summary { summary } => self.add_summary(summary),
            Payload::Score { score } => self.add_score(score),
        }
    }

    fn add_signal(&mut self, signal: &SignalPayload) {
        match signal.signal_type {
            SignalType::HeartRate => {
                self.hr_readings.push(signal.value);
                if self.max_hr.map_or(true, |m| signal.value > m) {
                    self.max_hr = Some(signal.value);
                }
            }
            SignalType::HeartRateVariability => {
                self.hrv_readings.push(signal.value);
            }
            SignalType::RestingHeartRate => {
                self.resting_hr_readings.push(signal.value);
            }
            SignalType::Spo2 => {
                self.spo2_readings.push(signal.value);
            }
            SignalType::SkinTemperature => {
                self.skin_temp_readings.push(signal.value);
            }
            SignalType::RespiratoryRate => {
                self.respiratory_rate_readings.push(signal.value);
            }
            SignalType::Steps => {
                let current = self.total_steps.unwrap_or(0);
                self.total_steps = Some(current + signal.value as u32);
            }
            SignalType::Calories => {
                let current = self.total_calories.unwrap_or(0.0);
                self.total_calories = Some(current + signal.value);
            }
            SignalType::ActiveCalories => {
                let current = self.active_calories.unwrap_or(0.0);
                self.active_calories = Some(current + signal.value);
            }
            SignalType::Distance => {
                let current = self.distance_meters.unwrap_or(0.0);
                self.distance_meters = Some(current + signal.value);
            }
            SignalType::ActiveMinutes => {
                let current = self.active_minutes.unwrap_or(0.0);
                self.active_minutes = Some(current + signal.value);
            }
            _ => {} // Ignore unknown signal types
        }
    }

    fn add_session(&mut self, session: &SessionPayload) {
        match session.session_type {
            SessionType::Sleep | SessionType::Nap => {
                self.sleep_sessions.push(SleepData {
                    start_time: session.start_time,
                    end_time: session.end_time,
                    metrics: session.metrics.clone(),
                });
            }
            SessionType::Workout => {
                // Extract workout metrics
                if let Some(v) = session.metrics.get("calories").and_then(|m| m.as_f64()) {
                    let current = self.active_calories.unwrap_or(0.0);
                    self.active_calories = Some(current + v);
                }
                if let Some(v) = session.metrics.get("distance_meters").and_then(|m| m.as_f64()) {
                    let current = self.distance_meters.unwrap_or(0.0);
                    self.distance_meters = Some(current + v);
                }
            }
            _ => {}
        }
    }

    fn add_summary(&mut self, summary: &SummaryPayload) {
        // Daily summaries typically contain aggregated data
        for (key, value) in &summary.metrics {
            match key.as_str() {
                "total_steps" | "steps" => {
                    if let Some(v) = value.as_i64() {
                        self.total_steps = Some(v as u32);
                    }
                }
                "total_calories" | "calories" => {
                    if let Some(v) = value.as_f64() {
                        self.total_calories = Some(v);
                    }
                }
                "active_calories" => {
                    if let Some(v) = value.as_f64() {
                        self.active_calories = Some(v);
                    }
                }
                "distance_meters" | "distance" => {
                    if let Some(v) = value.as_f64() {
                        self.distance_meters = Some(v);
                    }
                }
                "active_minutes" => {
                    if let Some(v) = value.as_f64() {
                        self.active_minutes = Some(v);
                    }
                }
                "resting_heart_rate" | "resting_hr" => {
                    if let Some(v) = value.as_f64() {
                        self.resting_hr_readings.push(v);
                    }
                }
                "hrv" | "hrv_rmssd" => {
                    if let Some(v) = value.as_f64() {
                        self.hrv_readings.push(v);
                    }
                }
                "spo2" | "avg_spo2" => {
                    if let Some(v) = value.as_f64() {
                        self.spo2_readings.push(v);
                    }
                }
                "body_battery" => {
                    if let Some(v) = value.as_f64() {
                        self.recovery_score = Some(v);
                    }
                }
                "training_load" | "strain" => {
                    if let Some(v) = value.as_f64() {
                        self.strain_score = Some(v);
                    }
                }
                _ => {}
            }
        }
    }

    fn add_score(&mut self, score: &ScorePayload) {
        // Normalize score to 0-100 range
        let normalized = normalize_score(score.value, score.scale.min, score.scale.max);

        match score.score_type {
            ScoreType::Recovery | ScoreType::BodyBattery => {
                self.recovery_score = Some(normalized);
            }
            ScoreType::Strain | ScoreType::TrainingLoad => {
                self.strain_score = Some(normalized);
            }
            _ => {}
        }
    }

    fn to_canonical(
        self,
        date: &str,
        timezone: &str,
        device_id: &str,
    ) -> Result<CanonicalWearSignals, ComputeError> {
        // Build canonical sleep from sessions
        let sleep = self.build_canonical_sleep();

        // Build canonical recovery from readings
        let recovery = CanonicalRecovery {
            hrv_rmssd_ms: average(&self.hrv_readings),
            resting_hr_bpm: average(&self.resting_hr_readings),
            vendor_recovery_score: self.recovery_score,
            skin_temp_deviation_c: average(&self.skin_temp_readings),
            spo2_percentage: average(&self.spo2_readings),
        };

        // Build canonical activity
        let activity = CanonicalActivity {
            vendor_strain_score: self.strain_score,
            calories: self.total_calories,
            active_calories: self.active_calories,
            average_hr_bpm: average(&self.hr_readings),
            max_hr_bpm: self.max_hr,
            distance_meters: self.distance_meters,
            steps: self.total_steps,
            active_minutes: self.active_minutes,
        };

        Ok(CanonicalWearSignals {
            vendor: provider_to_vendor(&self.provider),
            date: date.to_string(),
            device_id: device_id.to_string(),
            timezone: timezone.to_string(),
            observed_at: Utc::now(),
            sleep,
            recovery,
            activity,
            vendor_raw: self.vendor_raw,
        })
    }

    fn build_canonical_sleep(&self) -> CanonicalSleep {
        if self.sleep_sessions.is_empty() {
            return CanonicalSleep::default();
        }

        // Find the main sleep session (longest one that's not a nap)
        let main_sleep = self
            .sleep_sessions
            .iter()
            .max_by_key(|s| (s.end_time - s.start_time).num_minutes());

        match main_sleep {
            Some(sleep) => {
                let duration_minutes = (sleep.end_time - sleep.start_time).num_minutes() as f64;

                CanonicalSleep {
                    start_time: Some(sleep.start_time),
                    end_time: Some(sleep.end_time),
                    time_in_bed_minutes: sleep
                        .metrics
                        .get("time_in_bed_minutes")
                        .and_then(|v| v.as_f64())
                        .or(Some(duration_minutes)),
                    total_sleep_minutes: sleep
                        .metrics
                        .get("total_sleep_minutes")
                        .and_then(|v| v.as_f64()),
                    awake_minutes: sleep
                        .metrics
                        .get("awake_minutes")
                        .and_then(|v| v.as_f64()),
                    light_sleep_minutes: sleep
                        .metrics
                        .get("light_sleep_minutes")
                        .and_then(|v| v.as_f64()),
                    deep_sleep_minutes: sleep
                        .metrics
                        .get("deep_sleep_minutes")
                        .and_then(|v| v.as_f64()),
                    rem_sleep_minutes: sleep
                        .metrics
                        .get("rem_sleep_minutes")
                        .and_then(|v| v.as_f64()),
                    awakenings: sleep
                        .metrics
                        .get("awakenings")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as u32),
                    latency_minutes: sleep
                        .metrics
                        .get("latency_minutes")
                        .and_then(|v| v.as_f64()),
                    vendor_sleep_score: sleep
                        .metrics
                        .get("sleep_score")
                        .and_then(|v| v.as_f64()),
                    respiratory_rate: sleep
                        .metrics
                        .get("respiratory_rate")
                        .and_then(|v| v.as_f64())
                        .or_else(|| average(&self.respiratory_rate_readings)),
                }
            }
            None => CanonicalSleep::default(),
        }
    }
}

fn extract_date(timestamp: &DateTime<Utc>, context: Option<&Context>) -> String {
    // Try to use timezone from context if available
    let date = if let Some(ctx) = context {
        if let Some(_tz) = &ctx.timezone {
            // For simplicity, just use UTC date
            // In production, would parse timezone and convert
            timestamp.format("%Y-%m-%d").to_string()
        } else {
            timestamp.format("%Y-%m-%d").to_string()
        }
    } else {
        timestamp.format("%Y-%m-%d").to_string()
    };
    date
}

fn provider_to_vendor(provider: &Provider) -> Vendor {
    match provider {
        Provider::Whoop => Vendor::Whoop,
        Provider::Garmin => Vendor::Garmin,
        // Default to Garmin for unknown providers (could be extended)
        _ => Vendor::Garmin,
    }
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn normalize_score(value: f64, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return value;
    }
    ((value - min) / (max - min)) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_events() -> Vec<RawEvent> {
        let source = Source {
            provider: Provider::Whoop,
            device_model: Some("WHOOP 4.0".to_string()),
            device_id: Some("test-device".to_string()),
            firmware_version: None,
        };

        let timestamp = "2024-01-15T08:00:00Z".parse::<DateTime<Utc>>().unwrap();

        // Create sleep session
        let mut sleep_metrics = HashMap::new();
        sleep_metrics.insert("total_sleep_minutes".to_string(), MetricValue::Number(420.0));
        sleep_metrics.insert("deep_sleep_minutes".to_string(), MetricValue::Number(90.0));
        sleep_metrics.insert("rem_sleep_minutes".to_string(), MetricValue::Number(100.0));
        sleep_metrics.insert("light_sleep_minutes".to_string(), MetricValue::Number(200.0));
        sleep_metrics.insert("awake_minutes".to_string(), MetricValue::Number(30.0));
        sleep_metrics.insert("awakenings".to_string(), MetricValue::Integer(3));
        sleep_metrics.insert("sleep_score".to_string(), MetricValue::Number(85.0));

        let sleep_session = SessionPayload {
            session_type: SessionType::Sleep,
            start_time: timestamp - chrono::Duration::hours(8),
            end_time: timestamp,
            metrics: sleep_metrics,
        };

        let sleep_event = RawEvent::session(timestamp, source.clone(), sleep_session);

        // Create recovery score
        let recovery_score = ScorePayload {
            score_type: ScoreType::Recovery,
            value: 78.0,
            scale: ScoreScale { min: 0.0, max: 100.0 },
            components: HashMap::new(),
        };

        let recovery_event = RawEvent::score(timestamp, source.clone(), recovery_score);

        // Create HRV signal
        let hrv_signal = SignalPayload {
            signal_type: SignalType::HeartRateVariability,
            value: 65.0,
            unit: Unit::Ms,
            quality: Some(0.95),
        };

        let hrv_event = RawEvent::signal(timestamp, source.clone(), hrv_signal);

        // Create resting HR signal
        let rhr_signal = SignalPayload {
            signal_type: SignalType::RestingHeartRate,
            value: 52.0,
            unit: Unit::Bpm,
            quality: Some(0.98),
        };

        let rhr_event = RawEvent::signal(timestamp, source.clone(), rhr_signal);

        vec![sleep_event, recovery_event, hrv_event, rhr_event]
    }

    #[test]
    fn test_to_canonical() {
        let events = create_test_events();
        let signals = RawEventAdapter::to_canonical(&events, "America/New_York", "test-device")
            .unwrap();

        assert_eq!(signals.len(), 1);
        let sig = &signals[0];

        assert_eq!(sig.vendor, Vendor::Whoop);
        assert_eq!(sig.date, "2024-01-15");
        assert_eq!(sig.sleep.total_sleep_minutes, Some(420.0));
        assert_eq!(sig.sleep.deep_sleep_minutes, Some(90.0));
        assert_eq!(sig.recovery.hrv_rmssd_ms, Some(65.0));
        assert_eq!(sig.recovery.resting_hr_bpm, Some(52.0));
        assert_eq!(sig.recovery.vendor_recovery_score, Some(78.0));
    }

    #[test]
    fn test_parse_ndjson() {
        let ndjson = r#"{"schema_version":"wear.raw_event.v1","timestamp":"2024-01-15T08:00:00Z","source":{"provider":"whoop"},"record_type":"signal","payload":{"signal":{"type":"heart_rate","value":72.0,"unit":"bpm"}}}
{"schema_version":"wear.raw_event.v1","timestamp":"2024-01-15T08:01:00Z","source":{"provider":"whoop"},"record_type":"signal","payload":{"signal":{"type":"heart_rate","value":74.0,"unit":"bpm"}}}"#;

        let events = RawEventAdapter::parse_ndjson(ndjson).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_validate_events() {
        let events = create_test_events();
        let results = RawEventAdapter::validate_events(&events);
        assert!(results.is_empty()); // All events should be valid
    }
}
