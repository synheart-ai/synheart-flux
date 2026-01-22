//! Behavioral metrics computation module
//!
//! This module processes smartphone behavioral data (taps, scrolls, notifications, etc.)
//! and computes derived metrics like distraction score, focus hint, and burstiness.
//!
//! Pipeline: Session JSON → Adapter → Normalizer → Features → Baseline → Encoder → HSI JSON

pub mod adapter;
pub mod baseline;
pub mod encoder;
pub mod features;
pub mod normalizer;
pub mod pipeline;
pub mod types;

pub use pipeline::{behavior_to_hsi, BehaviorProcessor};
pub use types::{
    BehaviorEvent, BehaviorEventType, BehaviorSession, CanonicalBehaviorSignals,
    ContextualBehaviorSignals, DerivedBehaviorSignals, HsiAxes, HsiAxesDomain, HsiAxisReading,
    HsiDirection, HsiPayload, HsiPrivacy, HsiProducer, HsiSource, HsiSourceType, HsiWindow,
    NormalizedBehaviorSignals,
};
