//! Match Filter Module
//!
//! Ported from legacy/parsing/match_filter.go
//!
//! Filters match candidates based on configurable rules:
//! - Stale offer filtering (offers older than max age)
//! - Same-sender exclusion (prevent self-matching)

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::{Offer, Request};

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for match filtering
/// Ported from Go: MatchFilterConfig (match_filter.go:17-25)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchFilterConfig {
    /// Enable filtering of stale offers
    pub enable_stale_filter: bool,
    /// Maximum age for offers to be considered (default: 7 days)
    pub max_offer_age_days: i64,
    /// Enable same-sender exclusion
    pub enable_same_sender_exclusion: bool,
    /// Enable pharmaceutical validation (concentration and form checks)
    pub enable_pharmaceutical_validation: bool,
    /// Concentration tolerance percentage (default: 20.0)
    pub concentration_tolerance_percent: f64,
    /// Concentration reject threshold percentage (default: 50.0)
    pub concentration_reject_threshold_percent: f64,
    /// Penalty when one side is missing concentration (default: 0.15)
    pub missing_concentration_penalty: f64,
    /// Penalty when one side is missing form (default: 0.20)
    pub missing_form_penalty: f64,
}

impl Default for MatchFilterConfig {
    /// Default configuration
    /// Ported from Go: DefaultMatchFilterConfig (match_filter.go:28-33)
    fn default() -> Self {
        Self {
            enable_stale_filter: true,
            max_offer_age_days: 7,
            enable_same_sender_exclusion: true,
            enable_pharmaceutical_validation: true,
            concentration_tolerance_percent: 20.0,
            concentration_reject_threshold_percent: 50.0,
            missing_concentration_penalty: 0.15,
            missing_form_penalty: 0.20,
        }
    }
}

impl MatchFilterConfig {
    /// Create a permissive configuration (no filtering)
    pub fn permissive() -> Self {
        Self {
            enable_stale_filter: false,
            max_offer_age_days: 30,
            enable_same_sender_exclusion: false,
            enable_pharmaceutical_validation: false,
            concentration_tolerance_percent: 20.0,
            concentration_reject_threshold_percent: 50.0,
            missing_concentration_penalty: 0.15,
            missing_form_penalty: 0.20,
        }
    }

    /// Create a strict configuration
    pub fn strict() -> Self {
        Self {
            enable_stale_filter: true,
            max_offer_age_days: 3,
            enable_same_sender_exclusion: true,
            enable_pharmaceutical_validation: true,
            concentration_tolerance_percent: 15.0,
            concentration_reject_threshold_percent: 40.0,
            missing_concentration_penalty: 0.20,
            missing_form_penalty: 0.25,
        }
    }

    /// Get max offer age as Duration
    pub fn max_offer_age(&self) -> Duration {
        Duration::days(self.max_offer_age_days)
    }
}

// =============================================================================
// Statistics (Lock-free atomics)
// =============================================================================

/// Atomic statistics for filter tracking
/// Ported from Go: MatchFilterStats (match_filter.go:36-41)
#[derive(Debug, Default)]
pub struct MatchFilterStats {
    /// Total candidates evaluated
    total_candidates: AtomicU64,
    /// Filtered due to stale offer
    stale_filtered: AtomicU64,
    /// Filtered due to same sender
    same_sender_filtered: AtomicU64,
    /// Filtered due to concentration mismatch
    concentration_filtered: AtomicU64,
    /// Filtered due to form incompatibility
    form_filtered: AtomicU64,
    /// Candidates that passed all filters
    passed_filters: AtomicU64,
}

impl MatchFilterStats {
    /// Get a snapshot of current statistics
    /// Ported from Go: MatchFilterStats.GetStats (match_filter.go:44-51)
    pub fn snapshot(&self) -> MatchFilterStatsSnapshot {
        MatchFilterStatsSnapshot {
            total_candidates: self.total_candidates.load(Ordering::Relaxed),
            stale_filtered: self.stale_filtered.load(Ordering::Relaxed),
            same_sender_filtered: self.same_sender_filtered.load(Ordering::Relaxed),
            concentration_filtered: self.concentration_filtered.load(Ordering::Relaxed),
            form_filtered: self.form_filtered.load(Ordering::Relaxed),
            passed_filters: self.passed_filters.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_candidates.store(0, Ordering::Relaxed);
        self.stale_filtered.store(0, Ordering::Relaxed);
        self.same_sender_filtered.store(0, Ordering::Relaxed);
        self.concentration_filtered.store(0, Ordering::Relaxed);
        self.form_filtered.store(0, Ordering::Relaxed);
        self.passed_filters.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of filter statistics (for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchFilterStatsSnapshot {
    pub total_candidates: u64,
    pub stale_filtered: u64,
    pub same_sender_filtered: u64,
    pub concentration_filtered: u64,
    pub form_filtered: u64,
    pub passed_filters: u64,
}

impl MatchFilterStatsSnapshot {
    /// Calculate filter rate (percentage of candidates filtered out)
    pub fn filter_rate(&self) -> f64 {
        if self.total_candidates == 0 {
            return 0.0;
        }
        let filtered = self.stale_filtered
            + self.same_sender_filtered
            + self.concentration_filtered
            + self.form_filtered;
        filtered as f64 / self.total_candidates as f64 * 100.0
    }

    /// Calculate pass rate (percentage of candidates that passed)
    pub fn pass_rate(&self) -> f64 {
        if self.total_candidates == 0 {
            return 100.0;
        }
        self.passed_filters as f64 / self.total_candidates as f64 * 100.0
    }
}

// =============================================================================
// Filter Result
// =============================================================================

/// Result of filtering a candidate
/// Ported from Go: FilterResult (match_filter.go:62-65)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterResult {
    /// Whether the candidate passed all filters
    pub passed: bool,
    /// Reason for filtering (empty if passed)
    pub reason: FilterReason,
}

/// Reason why a candidate was filtered
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterReason {
    /// Candidate passed all filters
    Passed,
    /// Offer is too old
    StaleOffer,
    /// Same sender (by ID)
    SameSender,
    /// Concentration mismatch exceeds threshold
    ConcentrationMismatch(String),
    /// Form incompatibility
    FormIncompatible(String),
}

impl FilterResult {
    /// Create a passed result
    pub fn passed() -> Self {
        Self {
            passed: true,
            reason: FilterReason::Passed,
        }
    }

    /// Create a filtered result
    pub fn filtered(reason: FilterReason) -> Self {
        Self {
            passed: false,
            reason,
        }
    }
}

// =============================================================================
// Match Filter
// =============================================================================

/// Filters match candidates based on configurable rules
/// Ported from Go: MatchFilter (match_filter.go:54-59)
pub struct MatchFilter {
    config: std::sync::RwLock<MatchFilterConfig>,
    stats: MatchFilterStats,
    pharmaceutical_validator: std::sync::RwLock<crate::matching::PharmaceuticalValidator>,
}

impl Default for MatchFilter {
    fn default() -> Self {
        Self::new(MatchFilterConfig::default())
    }
}

impl MatchFilter {
    /// Create a new match filter
    /// Ported from Go: NewMatchFilter (match_filter.go:68-77)
    pub fn new(config: MatchFilterConfig) -> Self {
        let pharma_config = crate::matching::PharmaceuticalValidatorConfig {
            concentration_tolerance_percent: config.concentration_tolerance_percent,
            concentration_reject_threshold_percent: config.concentration_reject_threshold_percent,
            missing_concentration_penalty: config.missing_concentration_penalty,
            missing_form_penalty: config.missing_form_penalty,
            enable_concentration_check: config.enable_pharmaceutical_validation,
            enable_form_check: config.enable_pharmaceutical_validation,
        };

        Self {
            config: std::sync::RwLock::new(config),
            stats: MatchFilterStats::default(),
            pharmaceutical_validator: std::sync::RwLock::new(
                crate::matching::PharmaceuticalValidator::new(pharma_config),
            ),
        }
    }

    /// Create a permissive filter (no filtering)
    pub fn permissive() -> Self {
        Self::new(MatchFilterConfig::permissive())
    }

    // =========================================================================
    // Core Filtering Methods
    // =========================================================================

    /// Check if an offer should be considered for matching with a request
    /// Ported from Go: MatchFilter.FilterOfferForRequest (match_filter.go:80-99)
    pub fn filter_offer_for_request(&self, offer: &Offer, request: &Request) -> FilterResult {
        self.stats.total_candidates.fetch_add(1, Ordering::Relaxed);

        let config = self.config.read().unwrap();

        // Check stale offer
        if config.enable_stale_filter
            && let Some(result) = self.check_stale_offer(offer, &config)
        {
            self.stats.stale_filtered.fetch_add(1, Ordering::Relaxed);
            return result;
        }

        // Check same sender
        if config.enable_same_sender_exclusion && offer.participant_id == request.participant_id {
            self.stats
                .same_sender_filtered
                .fetch_add(1, Ordering::Relaxed);
            return FilterResult::filtered(FilterReason::SameSender);
        }

        // Check pharmaceutical validation
        if config.enable_pharmaceutical_validation {
            let validator = self.pharmaceutical_validator.read().unwrap();
            let (should_reject, rejection_reason) = validator.should_reject(offer, request);

            if should_reject && let Some(reason) = rejection_reason {
                // Determine if it's concentration or form rejection
                if reason.contains("Concentration") {
                    self.stats
                        .concentration_filtered
                        .fetch_add(1, Ordering::Relaxed);
                    return FilterResult::filtered(FilterReason::ConcentrationMismatch(reason));
                } else {
                    self.stats.form_filtered.fetch_add(1, Ordering::Relaxed);
                    return FilterResult::filtered(FilterReason::FormIncompatible(reason));
                }
            }
        }

        self.stats.passed_filters.fetch_add(1, Ordering::Relaxed);
        FilterResult::passed()
    }

    /// Check if a request should be considered for matching with an offer
    /// Ported from Go: MatchFilter.FilterRequestForOffer (match_filter.go:102-121)
    pub fn filter_request_for_offer(&self, request: &Request, offer: &Offer) -> FilterResult {
        // Delegate to the offer filter (same logic, different direction)
        self.filter_offer_for_request(offer, request)
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Check if an offer is too old
    /// Ported from Go: MatchFilter.checkStaleOffer (match_filter.go:124-133)
    fn check_stale_offer(&self, offer: &Offer, config: &MatchFilterConfig) -> Option<FilterResult> {
        let age = Utc::now() - offer.created_at;
        if age > config.max_offer_age() {
            return Some(FilterResult::filtered(FilterReason::StaleOffer));
        }
        None
    }

    // check_same_sender removed as it's now directly compared in filter_offer_for_request

    // =========================================================================
    // Batch Filtering
    // =========================================================================

    /// Filter a slice of offers for matching with a request
    /// Returns only offers that pass all filters
    /// Ported from Go: MatchFilter.FilterOffers (match_filter.go:158-181)
    pub fn filter_offers<'a>(&self, offers: &'a [Offer], request: &Request) -> Vec<&'a Offer> {
        if offers.is_empty() {
            return Vec::new();
        }

        let mut filtered = Vec::with_capacity(offers.len());
        let mut removed = 0;

        for offer in offers {
            let result = self.filter_offer_for_request(offer, request);
            if result.passed {
                filtered.push(offer);
            } else {
                removed += 1;
                tracing::debug!(
                    offer_id = %offer.id,
                    request_id = %request.id,
                    reason = ?result.reason,
                    "Filtered offer from matching"
                );
            }
        }

        if removed > 0 {
            tracing::info!(
                original = offers.len(),
                filtered = filtered.len(),
                removed = removed,
                "🔍 Filtered match candidates"
            );
        }

        filtered
    }

    /// Filter a slice of requests for matching with an offer
    /// Returns only requests that pass all filters
    /// Ported from Go: MatchFilter.FilterRequests (match_filter.go:184-207)
    pub fn filter_requests<'a>(&self, requests: &'a [Request], offer: &Offer) -> Vec<&'a Request> {
        if requests.is_empty() {
            return Vec::new();
        }

        let mut filtered = Vec::with_capacity(requests.len());
        let mut removed = 0;

        for request in requests {
            let result = self.filter_request_for_offer(request, offer);
            if result.passed {
                filtered.push(request);
            } else {
                removed += 1;
                tracing::debug!(
                    request_id = %request.id,
                    offer_id = %offer.id,
                    reason = ?result.reason,
                    "Filtered request from matching"
                );
            }
        }

        if removed > 0 {
            tracing::info!(
                original = requests.len(),
                filtered = filtered.len(),
                removed = removed,
                "🔍 Filtered match candidates"
            );
        }

        filtered
    }

    // =========================================================================
    // Statistics & Configuration
    // =========================================================================

    /// Get current statistics snapshot
    /// Ported from Go: MatchFilter.GetStats (match_filter.go:210-212)
    pub fn get_stats(&self) -> MatchFilterStatsSnapshot {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// Get current configuration
    /// Ported from Go: MatchFilter.GetConfig (match_filter.go:215-217)
    pub fn get_config(&self) -> MatchFilterConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    /// Ported from Go: MatchFilter.SetConfig (match_filter.go:220-228)
    pub fn set_config(&self, config: MatchFilterConfig) {
        tracing::info!(
            stale_filter = config.enable_stale_filter,
            max_offer_age_days = config.max_offer_age_days,
            same_sender_exclusion = config.enable_same_sender_exclusion,
            pharmaceutical_validation = config.enable_pharmaceutical_validation,
            "Match filter configuration updated"
        );

        // Update pharmaceutical validator config
        let pharma_config = crate::matching::PharmaceuticalValidatorConfig {
            concentration_tolerance_percent: config.concentration_tolerance_percent,
            concentration_reject_threshold_percent: config.concentration_reject_threshold_percent,
            missing_concentration_penalty: config.missing_concentration_penalty,
            missing_form_penalty: config.missing_form_penalty,
            enable_concentration_check: config.enable_pharmaceutical_validation,
            enable_form_check: config.enable_pharmaceutical_validation,
        };
        self.pharmaceutical_validator
            .write()
            .unwrap()
            .set_config(pharma_config);

        *self.config.write().unwrap() = config;
    }

    /// Set maximum offer age for stale filtering
    /// Ported from Go: MatchFilter.SetMaxOfferAge (match_filter.go:231-236)
    pub fn set_max_offer_age_days(&self, days: i64) {
        self.config.write().unwrap().max_offer_age_days = days;
        tracing::info!(max_offer_age_days = days, "Max offer age updated");
    }

    /// Enable or disable stale offer filtering
    /// Ported from Go: MatchFilter.EnableStaleFilter (match_filter.go:239-244)
    pub fn enable_stale_filter(&self, enabled: bool) {
        self.config.write().unwrap().enable_stale_filter = enabled;
        tracing::info!(enabled = enabled, "Stale filter toggled");
    }

    /// Enable or disable same-sender exclusion
    /// Ported from Go: MatchFilter.EnableSameSenderExclusion (match_filter.go:247-252)
    pub fn enable_same_sender_exclusion(&self, enabled: bool) {
        self.config.write().unwrap().enable_same_sender_exclusion = enabled;
        tracing::info!(enabled = enabled, "Same-sender exclusion toggled");
    }

    /// Enable or disable pharmaceutical validation
    pub fn enable_pharmaceutical_validation(&self, enabled: bool) {
        self.config
            .write()
            .unwrap()
            .enable_pharmaceutical_validation = enabled;

        // Update pharmaceutical validator
        let validator = self.pharmaceutical_validator.read().unwrap();
        let mut pharma_config = validator.get_config();
        pharma_config.enable_concentration_check = enabled;
        pharma_config.enable_form_check = enabled;
        validator.set_config(pharma_config);

        tracing::info!(enabled = enabled, "Pharmaceutical validation toggled");
    }
}
