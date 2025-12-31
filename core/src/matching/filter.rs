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
}

impl Default for MatchFilterConfig {
    /// Default configuration
    /// Ported from Go: DefaultMatchFilterConfig (match_filter.go:28-33)
    fn default() -> Self {
        Self {
            enable_stale_filter: true,
            max_offer_age_days: 7,
            enable_same_sender_exclusion: true,
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
        }
    }

    /// Create a strict configuration
    pub fn strict() -> Self {
        Self {
            enable_stale_filter: true,
            max_offer_age_days: 3,
            enable_same_sender_exclusion: true,
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
            passed_filters: self.passed_filters.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_candidates.store(0, Ordering::Relaxed);
        self.stale_filtered.store(0, Ordering::Relaxed);
        self.same_sender_filtered.store(0, Ordering::Relaxed);
        self.passed_filters.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of filter statistics (for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchFilterStatsSnapshot {
    pub total_candidates: u64,
    pub stale_filtered: u64,
    pub same_sender_filtered: u64,
    pub passed_filters: u64,
}

impl MatchFilterStatsSnapshot {
    /// Calculate filter rate (percentage of candidates filtered out)
    pub fn filter_rate(&self) -> f64 {
        if self.total_candidates == 0 {
            return 0.0;
        }
        let filtered = self.stale_filtered + self.same_sender_filtered;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterReason {
    /// Candidate passed all filters
    Passed,
    /// Offer is too old
    StaleOffer,
    /// Same sender (by ID)
    SameSender,
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
        Self {
            config: std::sync::RwLock::new(config),
            stats: MatchFilterStats::default(),
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
            "Match filter configuration updated"
        );
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
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use uuid::Uuid;

    fn create_offer(participant_id: Uuid, age_days: i64) -> Offer {
        Offer {
            id: Uuid::new_v4(),
            participant_id,
            created_at: Utc::now() - Duration::days(age_days),
            ..Default::default()
        }
    }

    fn create_request(participant_id: Uuid) -> Request {
        Request {
            id: Uuid::new_v4(),
            participant_id,
            ..Default::default()
        }
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = MatchFilterConfig::default();

        assert!(config.enable_stale_filter);
        assert_eq!(config.max_offer_age_days, 7);
        assert!(config.enable_same_sender_exclusion);
    }

    #[rstest]
    #[case(MatchFilterConfig::default(), true, 7, true)]
    #[case(MatchFilterConfig::permissive(), false, 30, false)]
    #[case(MatchFilterConfig::strict(), true, 3, true)]
    fn test_config_presets(
        #[case] config: MatchFilterConfig,
        #[case] stale: bool,
        #[case] days: i64,
        #[case] same_sender: bool,
    ) {
        assert_eq!(config.enable_stale_filter, stale);
        assert_eq!(config.max_offer_age_days, days);
        assert_eq!(config.enable_same_sender_exclusion, same_sender);
    }

    // =========================================================================
    // Stale Offer Tests
    // =========================================================================

    #[rstest]
    #[case(0, true)] // Fresh offer
    #[case(3, true)] // 3 days old
    #[case(6, true)] // 6 days old
    #[case(7, false)] // Exactly 7 days (boundary - stale)
    #[case(8, false)] // 8 days old (stale)
    #[case(30, false)] // Very old
    fn test_stale_offer_filter(#[case] age_days: i64, #[case] should_pass: bool) {
        let filter = MatchFilter::default();
        let offer = create_offer(Uuid::new_v4(), age_days);
        let request = create_request(Uuid::new_v4());

        let result = filter.filter_offer_for_request(&offer, &request);

        if should_pass {
            assert!(result.passed, "Offer {} days old should pass", age_days);
        } else {
            assert!(
                !result.passed,
                "Offer {} days old should be filtered",
                age_days
            );
            assert_eq!(result.reason, FilterReason::StaleOffer);
        }
    }

    #[test]
    fn test_stale_filter_disabled() {
        let filter = MatchFilter::new(MatchFilterConfig {
            enable_stale_filter: false,
            ..Default::default()
        });

        let offer = create_offer(Uuid::new_v4(), 30); // Very old
        let request = create_request(Uuid::new_v4());

        let result = filter.filter_offer_for_request(&offer, &request);
        assert!(
            result.passed,
            "Old offer should pass when stale filter disabled"
        );
    }

    // =========================================================================
    // Same Sender Tests
    // =========================================================================

    #[test]
    fn test_same_sender_by_id() {
        let filter = MatchFilter::default();
        let p_id = Uuid::new_v4();
        let offer = create_offer(p_id, 0);
        let request = create_request(p_id); // Same ID

        let result = filter.filter_offer_for_request(&offer, &request);

        assert!(!result.passed);
        assert_eq!(result.reason, FilterReason::SameSender);
    }

    #[test]
    fn test_different_sender_passes() {
        let filter = MatchFilter::default();
        let offer = create_offer(Uuid::new_v4(), 0);
        let request = create_request(Uuid::new_v4());

        let result = filter.filter_offer_for_request(&offer, &request);

        assert!(result.passed);
        assert_eq!(result.reason, FilterReason::Passed);
    }

    #[test]
    fn test_same_sender_exclusion_disabled() {
        let filter = MatchFilter::new(MatchFilterConfig {
            enable_same_sender_exclusion: false,
            ..Default::default()
        });

        let p_id = Uuid::new_v4();
        let offer = create_offer(p_id, 0);
        let request = create_request(p_id); // Same ID

        let result = filter.filter_offer_for_request(&offer, &request);
        assert!(
            result.passed,
            "Same sender should pass when exclusion disabled"
        );
    }

    // =========================================================================
    // Batch Filtering Tests
    // =========================================================================

    #[test]
    fn test_filter_offers_batch() {
        let filter = MatchFilter::default();
        let b_id = Uuid::new_v4();
        let request = create_request(b_id);

        let s1_id = Uuid::new_v4();
        let s4_id = Uuid::new_v4();

        let offers = vec![
            create_offer(s1_id, 0),           // Fresh, different sender
            create_offer(b_id, 0),            // Same ID as buyer
            create_offer(Uuid::new_v4(), 10), // Stale
            create_offer(s4_id, 0),           // Fresh, different sender
        ];

        let filtered = filter.filter_offers(&offers, &request);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].participant_id, s1_id);
        assert_eq!(filtered[1].participant_id, s4_id);
    }

    #[test]
    fn test_filter_requests_batch() {
        let filter = MatchFilter::default();
        let s_id = Uuid::new_v4();
        let offer = create_offer(s_id, 0);

        let b1_id = Uuid::new_v4();
        let b3_id = Uuid::new_v4();

        let requests = vec![
            create_request(b1_id),
            create_request(s_id), // Same ID as seller
            create_request(b3_id),
        ];

        let filtered = filter.filter_requests(&requests, &offer);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].participant_id, b1_id);
        assert_eq!(filtered[1].participant_id, b3_id);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_statistics_tracking() {
        let filter = MatchFilter::default();

        // Process some candidates
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();

        let fresh_offer = create_offer(p1, 0);
        let stale_offer = create_offer(p2, 10);
        let same_sender_offer = create_offer(p3, 0);

        let request = create_request(p3); // Same ID as third offer

        filter.filter_offer_for_request(&fresh_offer, &request);
        filter.filter_offer_for_request(&stale_offer, &request);
        filter.filter_offer_for_request(&same_sender_offer, &request);

        let stats = filter.get_stats();

        assert_eq!(stats.total_candidates, 3);
        assert_eq!(stats.stale_filtered, 1);
        assert_eq!(stats.same_sender_filtered, 1);
        assert_eq!(stats.passed_filters, 1);
    }

    #[test]
    fn test_statistics_rates() {
        let stats = MatchFilterStatsSnapshot {
            total_candidates: 100,
            stale_filtered: 20,
            same_sender_filtered: 10,
            passed_filters: 70,
        };

        assert!((stats.filter_rate() - 30.0).abs() < 0.001);
        assert!((stats.pass_rate() - 70.0).abs() < 0.001);
    }

    // =========================================================================
    // Configuration Update Tests
    // =========================================================================

    #[test]
    fn test_config_update() {
        let filter = MatchFilter::default();

        filter.set_max_offer_age_days(14);
        assert_eq!(filter.get_config().max_offer_age_days, 14);

        filter.enable_stale_filter(false);
        assert!(!filter.get_config().enable_stale_filter);

        filter.enable_same_sender_exclusion(false);
        assert!(!filter.get_config().enable_same_sender_exclusion);
    }

    #[test]
    fn test_reset_stats() {
        let filter = MatchFilter::default();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let offer = create_offer(p1, 0);
        let request = create_request(p2);

        filter.filter_offer_for_request(&offer, &request);
        assert_eq!(filter.get_stats().total_candidates, 1);

        filter.reset_stats();
        assert_eq!(filter.get_stats().total_candidates, 0);
    }
}
