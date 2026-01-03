//! Expiry Scorer - Validation and scoring for medication expiry dates
//!
//! Factors in medication expiry dates to prevent matching expired or
//! soon-to-expire medications. This is a safety-critical component that
//! ensures patients receive medications with adequate shelf life.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for expiry scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpiryConfig {
    /// Days before expiry to start applying decay (default: 30)
    pub decay_start_days: u32,
    /// Weight in total score calculation (default: 0.05)
    pub weight: f64,
    /// Score for missing expiry date (default: 0.5)
    pub missing_score: f64,
    /// Enable expiry scoring (default: true)
    pub enabled: bool,
}

impl Default for ExpiryConfig {
    fn default() -> Self {
        Self {
            decay_start_days: 30,
            weight: 0.05,
            missing_score: 0.5,
            enabled: true,
        }
    }
}

/// Warning types for expiry-related issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpiryWarning {
    /// Expiry date is missing from the offer
    MissingExpiry,
    /// Offer is expired
    Expired,
    /// Offer expires within decay_start_days
    NearExpiry { days_remaining: i64 },
    /// Offer does not meet minimum shelf life requirement
    InsufficientShelfLife {
        required_days: u32,
        actual_days: i64,
    },
}

/// Result of expiry scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpiryResult {
    /// The calculated expiry score (0.0-1.0)
    pub score: f64,
    /// Whether the offer is expired
    pub is_expired: bool,
    /// Days until expiry (negative if expired), None if no expiry date
    pub days_until_expiry: Option<i64>,
    /// Warning if any expiry-related issue detected
    pub warning: Option<ExpiryWarning>,
}

impl ExpiryResult {
    /// Create a result for an expired offer
    pub fn expired(days_past: i64) -> Self {
        Self {
            score: 0.0,
            is_expired: true,
            days_until_expiry: Some(-days_past),
            warning: Some(ExpiryWarning::Expired),
        }
    }

    /// Create a result for a missing expiry date
    pub fn missing(missing_score: f64) -> Self {
        Self {
            score: missing_score,
            is_expired: false,
            days_until_expiry: None,
            warning: Some(ExpiryWarning::MissingExpiry),
        }
    }

    /// Create a result for a valid expiry date
    pub fn valid(score: f64, days_remaining: i64, warning: Option<ExpiryWarning>) -> Self {
        Self {
            score,
            is_expired: false,
            days_until_expiry: Some(days_remaining),
            warning,
        }
    }
}

/// Expiry Scorer component for evaluating medication expiry dates
#[derive(Debug, Clone)]
pub struct ExpiryScorer {
    config: ExpiryConfig,
}

impl Default for ExpiryScorer {
    fn default() -> Self {
        Self::new(ExpiryConfig::default())
    }
}

impl ExpiryScorer {
    /// Create a new ExpiryScorer with the given configuration
    pub fn new(config: ExpiryConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration
    pub fn config(&self) -> &ExpiryConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: ExpiryConfig) {
        self.config = config;
    }

    /// Calculate days until expiry from a NaiveDate
    fn days_until_expiry_from_date(expiry_date: NaiveDate, now: DateTime<Utc>) -> i64 {
        let today = now.date_naive();
        (expiry_date - today).num_days()
    }

    /// Calculate days until expiry from a DateTime
    fn days_until_expiry_from_datetime(expiry_date: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
        let today = now.date_naive();
        let expiry_naive = expiry_date.date_naive();
        (expiry_naive - today).num_days()
    }

    /// Score an expiry date
    ///
    /// # Arguments
    /// * `expiry_date` - The expiry date as DateTime<Utc>, or None if missing
    /// * `now` - The current time for comparison
    ///
    /// # Returns
    /// An `ExpiryResult` with score and any warnings
    pub fn score(&self, expiry_date: Option<DateTime<Utc>>, now: DateTime<Utc>) -> ExpiryResult {
        // If scoring is disabled, return perfect score
        if !self.config.enabled {
            return ExpiryResult::valid(1.0, 0, None);
        }

        // Handle missing expiry date
        let expiry = match expiry_date {
            Some(date) => date,
            None => return ExpiryResult::missing(self.config.missing_score),
        };

        let days_remaining = Self::days_until_expiry_from_datetime(expiry, now);

        // Check if expired
        if days_remaining < 0 {
            return ExpiryResult::expired(-days_remaining);
        }

        // Check if within decay period
        let decay_days = self.config.decay_start_days as i64;
        if days_remaining <= decay_days {
            // Linear decay from 1.0 at decay_start_days to 0.0 at expiry
            let score = days_remaining as f64 / decay_days as f64;
            return ExpiryResult::valid(
                score,
                days_remaining,
                Some(ExpiryWarning::NearExpiry { days_remaining }),
            );
        }

        // Not expired and not near expiry - full score
        ExpiryResult::valid(1.0, days_remaining, None)
    }

    /// Score an expiry date from a NaiveDate (common in database models)
    ///
    /// # Arguments
    /// * `expiry_date` - The expiry date as NaiveDate, or None if missing
    /// * `now` - The current time for comparison
    ///
    /// # Returns
    /// An `ExpiryResult` with score and any warnings
    pub fn score_naive_date(
        &self,
        expiry_date: Option<NaiveDate>,
        now: DateTime<Utc>,
    ) -> ExpiryResult {
        // If scoring is disabled, return perfect score
        if !self.config.enabled {
            return ExpiryResult::valid(1.0, 0, None);
        }

        // Handle missing expiry date
        let expiry = match expiry_date {
            Some(date) => date,
            None => return ExpiryResult::missing(self.config.missing_score),
        };

        let days_remaining = Self::days_until_expiry_from_date(expiry, now);

        // Check if expired
        if days_remaining < 0 {
            return ExpiryResult::expired(-days_remaining);
        }

        // Check if within decay period
        let decay_days = self.config.decay_start_days as i64;
        if days_remaining <= decay_days {
            // Linear decay from 1.0 at decay_start_days to 0.0 at expiry
            let score = days_remaining as f64 / decay_days as f64;
            return ExpiryResult::valid(
                score,
                days_remaining,
                Some(ExpiryWarning::NearExpiry { days_remaining }),
            );
        }

        // Not expired and not near expiry - full score
        ExpiryResult::valid(1.0, days_remaining, None)
    }

    /// Check if an offer meets a minimum shelf life requirement
    ///
    /// # Arguments
    /// * `expiry_date` - The expiry date, or None if missing
    /// * `min_days` - Minimum required days until expiry
    ///
    /// # Returns
    /// `true` if the offer meets the shelf life requirement, `false` otherwise
    pub fn meets_shelf_life(&self, expiry_date: Option<DateTime<Utc>>, min_days: u32) -> bool {
        let now = Utc::now();
        self.meets_shelf_life_at(expiry_date, min_days, now)
    }

    /// Check if an offer meets a minimum shelf life requirement at a specific time
    ///
    /// # Arguments
    /// * `expiry_date` - The expiry date, or None if missing
    /// * `min_days` - Minimum required days until expiry
    /// * `now` - The current time for comparison
    ///
    /// # Returns
    /// `true` if the offer meets the shelf life requirement, `false` otherwise
    pub fn meets_shelf_life_at(
        &self,
        expiry_date: Option<DateTime<Utc>>,
        min_days: u32,
        now: DateTime<Utc>,
    ) -> bool {
        match expiry_date {
            Some(expiry) => {
                let days_remaining = Self::days_until_expiry_from_datetime(expiry, now);
                days_remaining >= min_days as i64
            }
            // Missing expiry date doesn't meet shelf life requirement
            None => false,
        }
    }

    /// Check if an offer meets a minimum shelf life requirement (NaiveDate version)
    ///
    /// # Arguments
    /// * `expiry_date` - The expiry date as NaiveDate, or None if missing
    /// * `min_days` - Minimum required days until expiry
    /// * `now` - The current time for comparison
    ///
    /// # Returns
    /// `true` if the offer meets the shelf life requirement, `false` otherwise
    pub fn meets_shelf_life_naive(
        &self,
        expiry_date: Option<NaiveDate>,
        min_days: u32,
        now: DateTime<Utc>,
    ) -> bool {
        match expiry_date {
            Some(expiry) => {
                let days_remaining = Self::days_until_expiry_from_date(expiry, now);
                days_remaining >= min_days as i64
            }
            // Missing expiry date doesn't meet shelf life requirement
            None => false,
        }
    }

    /// Check if an offer is expired
    ///
    /// # Arguments
    /// * `expiry_date` - The expiry date, or None if missing
    /// * `now` - The current time for comparison
    ///
    /// # Returns
    /// `true` if the offer is expired, `false` otherwise (including missing dates)
    pub fn is_expired(&self, expiry_date: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        match expiry_date {
            Some(expiry) => Self::days_until_expiry_from_datetime(expiry, now) < 0,
            // Missing expiry date is not considered expired
            None => false,
        }
    }

    /// Check if an offer is expired (NaiveDate version)
    pub fn is_expired_naive(&self, expiry_date: Option<NaiveDate>, now: DateTime<Utc>) -> bool {
        match expiry_date {
            Some(expiry) => Self::days_until_expiry_from_date(expiry, now) < 0,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    /// Helper to create a fixed "now" time for deterministic tests
    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap()
    }

    /// Helper to create a NaiveDate from days offset from a fixed date
    fn naive_date_offset(base: DateTime<Utc>, days: i64) -> NaiveDate {
        (base + Duration::days(days)).date_naive()
    }

    // =========================================================================
    // ExpiryConfig Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = ExpiryConfig::default();
        assert_eq!(config.decay_start_days, 30);
        assert!((config.weight - 0.05).abs() < 0.001);
        assert!((config.missing_score - 0.5).abs() < 0.001);
        assert!(config.enabled);
    }

    // =========================================================================
    // ExpiryResult Tests
    // =========================================================================

    #[test]
    fn test_result_expired() {
        let result = ExpiryResult::expired(5);
        assert!((result.score - 0.0).abs() < 0.001);
        assert!(result.is_expired);
        assert_eq!(result.days_until_expiry, Some(-5));
        assert_eq!(result.warning, Some(ExpiryWarning::Expired));
    }

    #[test]
    fn test_result_missing() {
        let result = ExpiryResult::missing(0.5);
        assert!((result.score - 0.5).abs() < 0.001);
        assert!(!result.is_expired);
        assert!(result.days_until_expiry.is_none());
        assert_eq!(result.warning, Some(ExpiryWarning::MissingExpiry));
    }

    #[test]
    fn test_result_valid() {
        let result = ExpiryResult::valid(
            0.8,
            24,
            Some(ExpiryWarning::NearExpiry { days_remaining: 24 }),
        );
        assert!((result.score - 0.8).abs() < 0.001);
        assert!(!result.is_expired);
        assert_eq!(result.days_until_expiry, Some(24));
        assert!(matches!(
            result.warning,
            Some(ExpiryWarning::NearExpiry { .. })
        ));
    }

    // =========================================================================
    // ExpiryScorer Score Tests
    // =========================================================================

    #[test]
    fn test_score_expired_offer() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // Expired 5 days ago
        let expiry = now - Duration::days(5);

        let result = scorer.score(Some(expiry), now);
        assert!((result.score - 0.0).abs() < 0.001);
        assert!(result.is_expired);
        assert_eq!(result.warning, Some(ExpiryWarning::Expired));
    }

    #[test]
    fn test_score_expires_today() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // Expires today (0 days remaining)
        let expiry = now;

        let result = scorer.score(Some(expiry), now);
        // 0 days remaining / 30 decay days = 0.0
        assert!((result.score - 0.0).abs() < 0.001);
        assert!(!result.is_expired);
        assert!(matches!(
            result.warning,
            Some(ExpiryWarning::NearExpiry { days_remaining: 0 })
        ));
    }

    #[test]
    fn test_score_near_expiry_15_days() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // Expires in 15 days
        let expiry = now + Duration::days(15);

        let result = scorer.score(Some(expiry), now);
        // 15 days remaining / 30 decay days = 0.5
        assert!((result.score - 0.5).abs() < 0.001);
        assert!(!result.is_expired);
        assert!(matches!(
            result.warning,
            Some(ExpiryWarning::NearExpiry { days_remaining: 15 })
        ));
    }

    #[test]
    fn test_score_near_expiry_at_threshold() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // Expires in exactly 30 days (at decay threshold)
        let expiry = now + Duration::days(30);

        let result = scorer.score(Some(expiry), now);
        // 30 days remaining / 30 decay days = 1.0
        assert!((result.score - 1.0).abs() < 0.001);
        assert!(!result.is_expired);
        // At exactly 30 days, still within decay period
        assert!(matches!(
            result.warning,
            Some(ExpiryWarning::NearExpiry { .. })
        ));
    }

    #[test]
    fn test_score_well_before_expiry() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // Expires in 90 days (well beyond decay threshold)
        let expiry = now + Duration::days(90);

        let result = scorer.score(Some(expiry), now);
        assert!((result.score - 1.0).abs() < 0.001);
        assert!(!result.is_expired);
        assert!(result.warning.is_none());
    }

    #[test]
    fn test_score_missing_expiry() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        let result = scorer.score(None, now);
        assert!((result.score - 0.5).abs() < 0.001);
        assert!(!result.is_expired);
        assert!(result.days_until_expiry.is_none());
        assert_eq!(result.warning, Some(ExpiryWarning::MissingExpiry));
    }

    #[test]
    fn test_score_disabled() {
        let config = ExpiryConfig {
            enabled: false,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();
        // Even expired offers get full score when disabled
        let expiry = now - Duration::days(10);

        let result = scorer.score(Some(expiry), now);
        assert!((result.score - 1.0).abs() < 0.001);
    }

    // =========================================================================
    // ExpiryScorer NaiveDate Score Tests
    // =========================================================================

    #[test]
    fn test_score_naive_date_expired() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = naive_date_offset(now, -5);

        let result = scorer.score_naive_date(Some(expiry), now);
        assert!((result.score - 0.0).abs() < 0.001);
        assert!(result.is_expired);
    }

    #[test]
    fn test_score_naive_date_valid() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = naive_date_offset(now, 60);

        let result = scorer.score_naive_date(Some(expiry), now);
        assert!((result.score - 1.0).abs() < 0.001);
        assert!(!result.is_expired);
    }

    // =========================================================================
    // Shelf Life Tests
    // =========================================================================

    #[test]
    fn test_meets_shelf_life_sufficient() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now + Duration::days(60);

        assert!(scorer.meets_shelf_life_at(Some(expiry), 30, now));
    }

    #[test]
    fn test_meets_shelf_life_exact() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now + Duration::days(30);

        assert!(scorer.meets_shelf_life_at(Some(expiry), 30, now));
    }

    #[test]
    fn test_meets_shelf_life_insufficient() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now + Duration::days(20);

        assert!(!scorer.meets_shelf_life_at(Some(expiry), 30, now));
    }

    #[test]
    fn test_meets_shelf_life_expired() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now - Duration::days(5);

        assert!(!scorer.meets_shelf_life_at(Some(expiry), 30, now));
    }

    #[test]
    fn test_meets_shelf_life_missing() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        // Missing expiry date doesn't meet shelf life requirement
        assert!(!scorer.meets_shelf_life_at(None, 30, now));
    }

    #[test]
    fn test_meets_shelf_life_naive() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = naive_date_offset(now, 60);

        assert!(scorer.meets_shelf_life_naive(Some(expiry), 30, now));
    }

    // =========================================================================
    // Is Expired Tests
    // =========================================================================

    #[test]
    fn test_is_expired_true() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now - Duration::days(1);

        assert!(scorer.is_expired(Some(expiry), now));
    }

    #[test]
    fn test_is_expired_false() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now + Duration::days(1);

        assert!(!scorer.is_expired(Some(expiry), now));
    }

    #[test]
    fn test_is_expired_today() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // Same day is not expired
        let expiry = now;

        assert!(!scorer.is_expired(Some(expiry), now));
    }

    #[test]
    fn test_is_expired_missing() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        // Missing expiry is not considered expired
        assert!(!scorer.is_expired(None, now));
    }

    // =========================================================================
    // Custom Configuration Tests
    // =========================================================================

    #[test]
    fn test_custom_decay_start_days() {
        let config = ExpiryConfig {
            decay_start_days: 60,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();
        // 30 days remaining with 60 day decay = 0.5 score
        let expiry = now + Duration::days(30);

        let result = scorer.score(Some(expiry), now);
        assert!((result.score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_custom_missing_score() {
        let config = ExpiryConfig {
            missing_score: 0.3,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();

        let result = scorer.score(None, now);
        assert!((result.score - 0.3).abs() < 0.001);
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_score_one_day_remaining() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = now + Duration::days(1);

        let result = scorer.score(Some(expiry), now);
        // 1 day / 30 days = 0.0333...
        assert!((result.score - (1.0 / 30.0)).abs() < 0.001);
        assert!(matches!(
            result.warning,
            Some(ExpiryWarning::NearExpiry { days_remaining: 1 })
        ));
    }

    #[test]
    fn test_score_just_past_decay_threshold() {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        // 31 days remaining (just past 30 day threshold)
        let expiry = now + Duration::days(31);

        let result = scorer.score(Some(expiry), now);
        assert!((result.score - 1.0).abs() < 0.001);
        assert!(result.warning.is_none());
    }
}
