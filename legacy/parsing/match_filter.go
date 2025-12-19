package parsing

import (
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
)

// =============================================================================
// Match Filter Configuration
// =============================================================================

// MatchFilterConfig holds configuration for match filtering.
type MatchFilterConfig struct {
	// Stale offer filtering
	EnableStaleFilter bool          // Enable filtering of stale offers
	MaxOfferAge       time.Duration // Maximum age for offers to be considered (default: 7 days)

	// Same-sender exclusion
	EnableSameSenderExclusion bool // Prevent matching offers/requests from same sender
}

// DefaultMatchFilterConfig returns sensible defaults for match filtering.
func DefaultMatchFilterConfig() MatchFilterConfig {
	return MatchFilterConfig{
		EnableStaleFilter:         true,
		MaxOfferAge:               DefaultMaxOfferAge,
		EnableSameSenderExclusion: true,
	}
}

// MatchFilterStats tracks filtering statistics.
type MatchFilterStats struct {
	TotalCandidates    atomic.Int64 // Total candidates evaluated
	StaleFiltered      atomic.Int64 // Filtered due to stale offer
	SameSenderFiltered atomic.Int64 // Filtered due to same sender
	PassedFilters      atomic.Int64 // Candidates that passed all filters
}

// GetStats returns a snapshot of filter statistics.
func (s *MatchFilterStats) GetStats() map[string]int64 {
	return map[string]int64{
		"total_candidates":     s.TotalCandidates.Load(),
		"stale_filtered":       s.StaleFiltered.Load(),
		"same_sender_filtered": s.SameSenderFiltered.Load(),
		"passed_filters":       s.PassedFilters.Load(),
	}
}

// =============================================================================
// Match Filter
// =============================================================================

// MatchFilter filters match candidates based on configurable rules.
type MatchFilter struct {
	config MatchFilterConfig
	stats  MatchFilterStats
	log    zerolog.Logger
}

// NewMatchFilter creates a new match filter.
func NewMatchFilter(cfg MatchFilterConfig, log zerolog.Logger) *MatchFilter {
	if cfg.MaxOfferAge <= 0 {
		cfg.MaxOfferAge = DefaultMaxOfferAge
	}

	return &MatchFilter{
		config: cfg,
		log:    log.With().Str("component", "match-filter").Logger(),
	}
}

// FilterResult contains the result of filtering a candidate.
type FilterResult struct {
	Passed bool
	Reason string // Reason for filtering (empty if passed)
}

// FilterOfferForRequest checks if an offer should be considered for matching with a request.
func (mf *MatchFilter) FilterOfferForRequest(offer *entity.Offer, request *entity.Request) FilterResult {
	mf.stats.TotalCandidates.Add(1)

	// Check stale offer
	if mf.config.EnableStaleFilter {
		if result := mf.checkStaleOffer(offer); !result.Passed {
			mf.stats.StaleFiltered.Add(1)
			return result
		}
	}

	// Check same sender
	if mf.config.EnableSameSenderExclusion {
		if result := mf.checkSameSender(offer.SourcePhone, request.SourcePhone, offer.SourceName, request.SourceName); !result.Passed {
			mf.stats.SameSenderFiltered.Add(1)
			return result
		}
	}

	mf.stats.PassedFilters.Add(1)
	return FilterResult{Passed: true}
}

// FilterRequestForOffer checks if a request should be considered for matching with an offer.
func (mf *MatchFilter) FilterRequestForOffer(request *entity.Request, offer *entity.Offer) FilterResult {
	mf.stats.TotalCandidates.Add(1)

	// Check stale offer (the offer is still the one being checked for staleness)
	if mf.config.EnableStaleFilter {
		if result := mf.checkStaleOffer(offer); !result.Passed {
			mf.stats.StaleFiltered.Add(1)
			return result
		}
	}

	// Check same sender
	if mf.config.EnableSameSenderExclusion {
		if result := mf.checkSameSender(offer.SourcePhone, request.SourcePhone, offer.SourceName, request.SourceName); !result.Passed {
			mf.stats.SameSenderFiltered.Add(1)
			return result
		}
	}

	mf.stats.PassedFilters.Add(1)
	return FilterResult{Passed: true}
}

// checkStaleOffer checks if an offer is too old.
func (mf *MatchFilter) checkStaleOffer(offer *entity.Offer) FilterResult {
	age := time.Since(offer.CreatedAt)
	if age > mf.config.MaxOfferAge {
		return FilterResult{
			Passed: false,
			Reason: "stale_offer",
		}
	}
	return FilterResult{Passed: true}
}

// checkSameSender checks if offer and request are from the same sender.
func (mf *MatchFilter) checkSameSender(offerPhone, requestPhone, offerName, requestName string) FilterResult {
	// Check by phone number (most reliable)
	if offerPhone != "" && requestPhone != "" && offerPhone == requestPhone {
		return FilterResult{
			Passed: false,
			Reason: "same_sender_phone",
		}
	}

	// Fallback: check by name if phones not available
	if offerPhone == "" && requestPhone == "" {
		if offerName != "" && requestName != "" && offerName == requestName {
			return FilterResult{
				Passed: false,
				Reason: "same_sender_name",
			}
		}
	}

	return FilterResult{Passed: true}
}

// FilterOffers filters a slice of offers for matching with a request.
// Returns only offers that pass all filters.
func (mf *MatchFilter) FilterOffers(offers []*entity.Offer, request *entity.Request) []*entity.Offer {
	if len(offers) == 0 {
		return offers
	}

	filtered := make([]*entity.Offer, 0, len(offers))
	for _, offer := range offers {
		result := mf.FilterOfferForRequest(offer, request)
		if result.Passed {
			filtered = append(filtered, offer)
		} else {
			mf.log.Debug().
				Str("offer_id", offer.ID).
				Str("request_id", request.ID).
				Str("reason", result.Reason).
				Msg("Filtered offer from matching")
		}
	}

	if len(filtered) < len(offers) {
		mf.log.Info().
			Int("original", len(offers)).
			Int("filtered", len(filtered)).
			Int("removed", len(offers)-len(filtered)).
			Msg("🔍 Filtered match candidates")
	}

	return filtered
}

// FilterRequests filters a slice of requests for matching with an offer.
// Returns only requests that pass all filters.
func (mf *MatchFilter) FilterRequests(requests []*entity.Request, offer *entity.Offer) []*entity.Request {
	if len(requests) == 0 {
		return requests
	}

	filtered := make([]*entity.Request, 0, len(requests))
	for _, request := range requests {
		result := mf.FilterRequestForOffer(request, offer)
		if result.Passed {
			filtered = append(filtered, request)
		} else {
			mf.log.Debug().
				Str("request_id", request.ID).
				Str("offer_id", offer.ID).
				Str("reason", result.Reason).
				Msg("Filtered request from matching")
		}
	}

	if len(filtered) < len(requests) {
		mf.log.Info().
			Int("original", len(requests)).
			Int("filtered", len(filtered)).
			Int("removed", len(requests)-len(filtered)).
			Msg("🔍 Filtered match candidates")
	}

	return filtered
}

// GetStats returns the current filter statistics.
func (mf *MatchFilter) GetStats() map[string]int64 {
	return mf.stats.GetStats()
}

// GetConfig returns the current configuration.
func (mf *MatchFilter) GetConfig() MatchFilterConfig {
	return mf.config
}

// SetConfig updates the filter configuration.
func (mf *MatchFilter) SetConfig(cfg MatchFilterConfig) {
	mf.config = cfg
	mf.log.Info().
		Bool("stale_filter", cfg.EnableStaleFilter).
		Dur("max_offer_age", cfg.MaxOfferAge).
		Bool("same_sender_exclusion", cfg.EnableSameSenderExclusion).
		Msg("Match filter configuration updated")
}

// SetMaxOfferAge sets the maximum offer age for stale filtering.
func (mf *MatchFilter) SetMaxOfferAge(age time.Duration) {
	mf.config.MaxOfferAge = age
	mf.log.Info().
		Dur("max_offer_age", age).
		Msg("Max offer age updated")
}

// EnableStaleFilter enables or disables stale offer filtering.
func (mf *MatchFilter) EnableStaleFilter(enabled bool) {
	mf.config.EnableStaleFilter = enabled
	mf.log.Info().
		Bool("enabled", enabled).
		Msg("Stale filter toggled")
}

// EnableSameSenderExclusion enables or disables same-sender exclusion.
func (mf *MatchFilter) EnableSameSenderExclusion(enabled bool) {
	mf.config.EnableSameSenderExclusion = enabled
	mf.log.Info().
		Bool("enabled", enabled).
		Msg("Same-sender exclusion toggled")
}
