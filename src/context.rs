//! Staleness-aware biosignal context
//!
//! This module provides types for capturing daily biosignal context from wearables
//! and applying staleness decay to produce time-valid confidence scores.
//!
//! The key insight is that wearable data (sleep, recovery, HRV) provides honest
//! background context but decays in relevance over time. Behavior data provides
//! realtime state signals.

use crate::behavior::types::{HsiAxisReading, HsiDirection};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default half-life for staleness decay in hours.
/// After 12 hours, confidence drops to 50%.
pub const DEFAULT_DECAY_HALF_LIFE_HOURS: f64 = 12.0;

/// Bio daily context captured from wearable data processing.
///
/// This represents a snapshot of biosignal context from a single day's
/// wearable data (sleep quality, recovery, HRV/RHR deviations).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BioDailyContext {
    /// When the underlying data was observed (e.g., sleep end time)
    pub observed_at_utc: DateTime<Utc>,
    /// When this context was computed
    pub computed_at_utc: DateTime<Utc>,
    /// Sleep quality score (0.0 - 1.0), if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleep_quality: Option<f32>,
    /// Recovery score (0.0 - 1.0), if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<f32>,
    /// HRV deviation from baseline, normalized to -1.0 to 1.0
    /// Positive = above baseline, negative = below baseline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hrv_delta: Option<f32>,
    /// RHR deviation from baseline, normalized to -1.0 to 1.0
    /// Positive = above baseline (worse), negative = below baseline (better)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rhr_delta: Option<f32>,
    /// Source identifiers that contributed to this context
    #[serde(default)]
    pub source_ids: Vec<String>,
}

impl BioDailyContext {
    /// Create a new BioDailyContext with the given timestamps
    pub fn new(observed_at_utc: DateTime<Utc>, computed_at_utc: DateTime<Utc>) -> Self {
        Self {
            observed_at_utc,
            computed_at_utc,
            sleep_quality: None,
            recovery: None,
            hrv_delta: None,
            rhr_delta: None,
            source_ids: Vec::new(),
        }
    }

    /// Check if this context has any meaningful data
    pub fn has_data(&self) -> bool {
        self.sleep_quality.is_some()
            || self.recovery.is_some()
            || self.hrv_delta.is_some()
            || self.rhr_delta.is_some()
    }
}

/// Bio context with staleness decay applied.
///
/// Wraps a BioDailyContext and applies exponential decay to confidence
/// based on how old the data is relative to a reference time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayedBioContext {
    /// The underlying bio context
    pub context: BioDailyContext,
    /// Base confidence before decay (typically from data quality/coverage)
    pub base_confidence: f64,
    /// Confidence after applying staleness decay
    pub decayed_confidence: f64,
    /// Time until which this context is considered reasonably valid
    /// (when confidence drops below ~10%)
    pub valid_until_utc: DateTime<Utc>,
    /// Age of the data in seconds
    pub age_seconds: i64,
}

impl DecayedBioContext {
    /// Create a decayed context from a BioDailyContext and reference time.
    ///
    /// # Arguments
    /// * `context` - The bio daily context
    /// * `base_confidence` - Initial confidence (0.0 - 1.0) before decay
    /// * `now_utc` - Reference time for calculating age/decay
    /// * `half_life_hours` - Half-life for exponential decay (default 12 hours)
    pub fn from_context(
        context: BioDailyContext,
        base_confidence: f64,
        now_utc: DateTime<Utc>,
        half_life_hours: f64,
    ) -> Self {
        let age_seconds = (now_utc - context.observed_at_utc).num_seconds();
        let age_hours = age_seconds as f64 / 3600.0;

        // Exponential decay: confidence = base * 0.5^(age/half_life)
        let decay_factor = 0.5_f64.powf(age_hours / half_life_hours);
        let decayed_confidence = (base_confidence * decay_factor).clamp(0.0, 1.0);

        // Calculate valid_until_utc (when confidence drops to ~10% of base)
        // 0.1 = 0.5^(hours/half_life) => hours = half_life * log2(10) ≈ half_life * 3.32
        let valid_hours = half_life_hours * 3.32;
        let valid_until_utc = context.observed_at_utc
            + chrono::Duration::seconds((valid_hours * 3600.0) as i64);

        Self {
            context,
            base_confidence,
            decayed_confidence,
            valid_until_utc,
            age_seconds,
        }
    }

    /// Create a decayed context using the default half-life (12 hours)
    pub fn from_context_default(
        context: BioDailyContext,
        base_confidence: f64,
        now_utc: DateTime<Utc>,
    ) -> Self {
        Self::from_context(context, base_confidence, now_utc, DEFAULT_DECAY_HALF_LIFE_HOURS)
    }

    /// Get the freshness score (inverse of staleness), 0.0 - 1.0
    pub fn freshness(&self) -> f64 {
        // Freshness is the decay factor itself (1.0 = fresh, 0.0 = stale)
        if self.base_confidence > 0.0 {
            (self.decayed_confidence / self.base_confidence).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Check if the context is still reasonably valid (>10% confidence)
    pub fn is_valid(&self, now_utc: DateTime<Utc>) -> bool {
        now_utc < self.valid_until_utc
    }

    /// Produce HSI axis readings for this bio context.
    ///
    /// # Arguments
    /// * `window_id` - The window ID to use for the readings
    ///
    /// # Returns
    /// Vector of HsiAxisReading for bio_freshness, recovery_context, sleep_context
    pub fn to_hsi_readings(&self, window_id: &str) -> Vec<HsiAxisReading> {
        let mut readings = Vec::new();
        let source_ids = if self.context.source_ids.is_empty() {
            None
        } else {
            Some(self.context.source_ids.clone())
        };

        // Bio freshness - how fresh the wearable data is
        readings.push(HsiAxisReading {
            axis: "bio_freshness".to_string(),
            score: Some(self.freshness()),
            confidence: self.base_confidence,
            window_id: window_id.to_string(),
            direction: Some(HsiDirection::HigherIsMore),
            unit: Some("freshness".to_string()),
            evidence_source_ids: source_ids.clone(),
            notes: Some(format!(
                "Age: {} seconds, half-life: {} hours",
                self.age_seconds, DEFAULT_DECAY_HALF_LIFE_HOURS
            )),
        });

        // Recovery context - recovery score with decayed confidence
        if let Some(recovery) = self.context.recovery {
            readings.push(HsiAxisReading {
                axis: "recovery_context".to_string(),
                score: Some(recovery as f64),
                confidence: self.decayed_confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("score".to_string()),
                evidence_source_ids: source_ids.clone(),
                notes: None,
            });
        }

        // Sleep context - sleep quality with decayed confidence
        if let Some(sleep_quality) = self.context.sleep_quality {
            readings.push(HsiAxisReading {
                axis: "sleep_context".to_string(),
                score: Some(sleep_quality as f64),
                confidence: self.decayed_confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("score".to_string()),
                evidence_source_ids: source_ids.clone(),
                notes: None,
            });
        }

        // HRV delta context - normalized deviation with decayed confidence
        if let Some(hrv_delta) = self.context.hrv_delta {
            // Convert -1..1 to 0..1 for HSI score (0.5 = baseline)
            let normalized_score = ((hrv_delta as f64 + 1.0) / 2.0).clamp(0.0, 1.0);
            readings.push(HsiAxisReading {
                axis: "hrv_delta_context".to_string(),
                score: Some(normalized_score),
                confidence: self.decayed_confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::Bidirectional),
                unit: Some("normalized_deviation".to_string()),
                evidence_source_ids: source_ids.clone(),
                notes: Some("0.5 = baseline, >0.5 = above baseline, <0.5 = below baseline".to_string()),
            });
        }

        // RHR delta context - normalized deviation with decayed confidence
        if let Some(rhr_delta) = self.context.rhr_delta {
            // Convert -1..1 to 0..1, but RHR higher is worse so invert
            let normalized_score = ((1.0 - rhr_delta as f64) / 2.0).clamp(0.0, 1.0);
            readings.push(HsiAxisReading {
                axis: "rhr_delta_context".to_string(),
                score: Some(normalized_score),
                confidence: self.decayed_confidence,
                window_id: window_id.to_string(),
                direction: Some(HsiDirection::HigherIsMore),
                unit: Some("normalized_deviation".to_string()),
                evidence_source_ids: source_ids,
                notes: Some("0.5 = baseline, >0.5 = below baseline (better), <0.5 = above baseline (worse)".to_string()),
            });
        }

        readings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_test_context() -> BioDailyContext {
        BioDailyContext {
            observed_at_utc: Utc.with_ymd_and_hms(2024, 1, 15, 6, 30, 0).unwrap(),
            computed_at_utc: Utc.with_ymd_and_hms(2024, 1, 15, 6, 31, 0).unwrap(),
            sleep_quality: Some(0.85),
            recovery: Some(0.75),
            hrv_delta: Some(0.1),  // 10% above baseline
            rhr_delta: Some(-0.05), // 5% below baseline (good)
            source_ids: vec!["whoop-device-123".to_string()],
        }
    }

    #[test]
    fn test_decay_at_zero_age() {
        let context = make_test_context();
        let now = context.observed_at_utc; // Same time = zero age

        let decayed = DecayedBioContext::from_context_default(context, 0.9, now);

        assert_eq!(decayed.age_seconds, 0);
        assert!((decayed.decayed_confidence - 0.9).abs() < 0.001);
        assert!((decayed.freshness() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_decay_at_half_life() {
        let context = make_test_context();
        let now = context.observed_at_utc + chrono::Duration::hours(12); // Exactly 12 hours

        let decayed = DecayedBioContext::from_context_default(context, 1.0, now);

        // At half-life, confidence should be 50%
        assert!((decayed.decayed_confidence - 0.5).abs() < 0.001);
        assert!((decayed.freshness() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_decay_at_double_half_life() {
        let context = make_test_context();
        let now = context.observed_at_utc + chrono::Duration::hours(24); // 24 hours = 2x half-life

        let decayed = DecayedBioContext::from_context_default(context, 1.0, now);

        // At 2x half-life, confidence should be 25%
        assert!((decayed.decayed_confidence - 0.25).abs() < 0.001);
        assert!((decayed.freshness() - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_decay_with_custom_half_life() {
        let context = make_test_context();
        let now = context.observed_at_utc + chrono::Duration::hours(6); // 6 hours

        // With 6 hour half-life, this should be exactly 50%
        let decayed = DecayedBioContext::from_context(context, 1.0, now, 6.0);

        assert!((decayed.decayed_confidence - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_is_valid() {
        let context = make_test_context();
        let observed = context.observed_at_utc;

        let decayed = DecayedBioContext::from_context_default(context, 0.9, observed);

        // Should be valid at observed time
        assert!(decayed.is_valid(observed));

        // Should be valid 30 hours later (still above 10%)
        assert!(decayed.is_valid(observed + chrono::Duration::hours(30)));

        // Should be invalid after ~40 hours (12 * 3.32 ≈ 40)
        assert!(!decayed.is_valid(observed + chrono::Duration::hours(50)));
    }

    #[test]
    fn test_hsi_readings() {
        let context = make_test_context();
        let now = context.observed_at_utc + chrono::Duration::hours(6);

        let decayed = DecayedBioContext::from_context_default(context, 0.9, now);
        let readings = decayed.to_hsi_readings("w_snapshot");

        // Should have bio_freshness, recovery_context, sleep_context, hrv_delta_context, rhr_delta_context
        assert_eq!(readings.len(), 5);

        // Check bio_freshness
        let freshness = readings.iter().find(|r| r.axis == "bio_freshness").unwrap();
        assert!(freshness.score.is_some());
        assert!(freshness.score.unwrap() > 0.5); // Should still be fairly fresh at 6 hours

        // Check recovery_context
        let recovery = readings.iter().find(|r| r.axis == "recovery_context").unwrap();
        // Use approximate comparison due to f32/f64 conversion
        assert!((recovery.score.unwrap() - 0.75).abs() < 0.001);
        assert!(recovery.confidence < 0.9); // Should be decayed

        // Check sleep_context
        let sleep = readings.iter().find(|r| r.axis == "sleep_context").unwrap();
        // Use approximate comparison due to f32/f64 conversion
        assert!((sleep.score.unwrap() - 0.85).abs() < 0.001);

        // Check window_id
        for reading in &readings {
            assert_eq!(reading.window_id, "w_snapshot");
        }

        // Check source_ids
        for reading in &readings {
            assert_eq!(reading.evidence_source_ids, Some(vec!["whoop-device-123".to_string()]));
        }
    }

    #[test]
    fn test_hsi_readings_minimal_context() {
        let context = BioDailyContext {
            observed_at_utc: Utc::now(),
            computed_at_utc: Utc::now(),
            sleep_quality: None,
            recovery: Some(0.6),
            hrv_delta: None,
            rhr_delta: None,
            source_ids: vec![],
        };
        let now = context.observed_at_utc;

        let decayed = DecayedBioContext::from_context_default(context, 0.8, now);
        let readings = decayed.to_hsi_readings("w_test");

        // Should have bio_freshness and recovery_context only
        assert_eq!(readings.len(), 2);

        let axes: Vec<&str> = readings.iter().map(|r| r.axis.as_str()).collect();
        assert!(axes.contains(&"bio_freshness"));
        assert!(axes.contains(&"recovery_context"));
    }

    #[test]
    fn test_has_data() {
        let empty_context = BioDailyContext::new(Utc::now(), Utc::now());
        assert!(!empty_context.has_data());

        let with_recovery = BioDailyContext {
            recovery: Some(0.7),
            ..BioDailyContext::new(Utc::now(), Utc::now())
        };
        assert!(with_recovery.has_data());
    }

    #[test]
    fn test_hrv_delta_normalization() {
        // Test that hrv_delta -1..1 maps to 0..1 correctly
        let context = BioDailyContext {
            observed_at_utc: Utc::now(),
            computed_at_utc: Utc::now(),
            sleep_quality: None,
            recovery: None,
            hrv_delta: Some(0.0), // At baseline
            rhr_delta: None,
            source_ids: vec![],
        };

        let decayed = DecayedBioContext::from_context_default(context.clone(), 1.0, Utc::now());
        let readings = decayed.to_hsi_readings("w_test");
        let hrv = readings.iter().find(|r| r.axis == "hrv_delta_context").unwrap();
        assert!((hrv.score.unwrap() - 0.5).abs() < 0.001); // 0 maps to 0.5

        // Test positive delta
        let positive_context = BioDailyContext {
            hrv_delta: Some(1.0), // Max positive
            ..context.clone()
        };
        let decayed = DecayedBioContext::from_context_default(positive_context, 1.0, Utc::now());
        let readings = decayed.to_hsi_readings("w_test");
        let hrv = readings.iter().find(|r| r.axis == "hrv_delta_context").unwrap();
        assert!((hrv.score.unwrap() - 1.0).abs() < 0.001); // 1 maps to 1.0

        // Test negative delta
        let negative_context = BioDailyContext {
            hrv_delta: Some(-1.0), // Max negative
            ..context
        };
        let decayed = DecayedBioContext::from_context_default(negative_context, 1.0, Utc::now());
        let readings = decayed.to_hsi_readings("w_test");
        let hrv = readings.iter().find(|r| r.axis == "hrv_delta_context").unwrap();
        assert!((hrv.score.unwrap() - 0.0).abs() < 0.001); // -1 maps to 0.0
    }
}
