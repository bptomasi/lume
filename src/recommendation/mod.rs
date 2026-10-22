//! Explainable ranking over publisher evidence and explicit local preferences.
//!
//! SQLite owns text matching. This module owns validation, default interests,
//! transparent scoring and bounded diversity. Reading is never positive feedback.
//! Scores order candidates; they cannot establish factual or technical quality.

mod ranking;
mod validation;

pub use ranking::{Candidate, select};
pub use validation::validate_profile;

pub mod session;
