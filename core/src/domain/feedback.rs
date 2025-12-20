//! Feedback Record entity
//!
//! Stores user feedback (confirm/reject) on matches for learning system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Feedback record for match confirmation or rejection
/// Used by the weight learning system to optimize matching weights
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FeedbackRecord {
    pub id: Uuid,
    pub match_id: Uuid,
    pub user_id: String,
    pub confirmed: bool,
    /// Individual factor scores at the time of feedback
    pub medication_score: f64,
    pub dosage_score: f64,
    pub quantity_score: f64,
    pub price_score: f64,
    pub recency_score: f64,
    pub total_score: f64,
    pub created_at: DateTime<Utc>,
}

impl FeedbackRecord {
    /// Create a new feedback record
    pub fn new(
        match_id: Uuid,
        user_id: String,
        confirmed: bool,
        medication_score: f64,
        dosage_score: f64,
        quantity_score: f64,
        price_score: f64,
        recency_score: f64,
        total_score: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id,
            user_id,
            confirmed,
            medication_score,
            dosage_score,
            quantity_score,
            price_score,
            recency_score,
            total_score,
            created_at: Utc::now(),
        }
    }
}

/// Statistics for feedback-based weight learning
/// Aggregated by confirmation status
#[derive(Debug, Clone, Default)]
pub struct FeedbackStats {
    pub sample_count: i64,
    pub confirmed_count: i64,
    pub rejected_count: i64,
    /// Average scores for confirmed matches
    pub confirmed_avg: FeedbackAverage,
    /// Average scores for rejected matches
    pub rejected_avg: FeedbackAverage,
}

#[derive(Debug, Clone, Default)]
pub struct FeedbackAverage {
    pub medication: f64,
    pub dosage: f64,
    pub quantity: f64,
    pub price: f64,
    pub recency: f64,
    pub total: f64,
}
