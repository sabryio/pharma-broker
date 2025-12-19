package parsing

import (
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
)

// =============================================================================
// MatchFilterConfig Tests
// =============================================================================

func TestDefaultMatchFilterConfig(t *testing.T) {
	cfg := DefaultMatchFilterConfig()

	if !cfg.EnableStaleFilter {
		t.Error("EnableStaleFilter should be true by default")
	}
	if cfg.MaxOfferAge != DefaultMaxOfferAge {
		t.Errorf("MaxOfferAge = %v, want %v", cfg.MaxOfferAge, DefaultMaxOfferAge)
	}
	if !cfg.EnableSameSenderExclusion {
		t.Error("EnableSameSenderExclusion should be true by default")
	}
}

// =============================================================================
// MatchFilterStats Tests
// =============================================================================

func TestMatchFilterStats_GetStats(t *testing.T) {
	stats := &MatchFilterStats{}
	stats.TotalCandidates.Store(100)
	stats.StaleFiltered.Store(10)
	stats.SameSenderFiltered.Store(5)
	stats.PassedFilters.Store(85)

	result := stats.GetStats()

	if result["total_candidates"] != 100 {
		t.Errorf("total_candidates = %d, want 100", result["total_candidates"])
	}
	if result["stale_filtered"] != 10 {
		t.Errorf("stale_filtered = %d, want 10", result["stale_filtered"])
	}
	if result["same_sender_filtered"] != 5 {
		t.Errorf("same_sender_filtered = %d, want 5", result["same_sender_filtered"])
	}
	if result["passed_filters"] != 85 {
		t.Errorf("passed_filters = %d, want 85", result["passed_filters"])
	}
}

// =============================================================================
// NewMatchFilter Tests
// =============================================================================

func TestNewMatchFilter_DefaultValues(t *testing.T) {
	log := zerolog.Nop()
	cfg := MatchFilterConfig{} // Zero values

	mf := NewMatchFilter(cfg, log)

	if mf.config.MaxOfferAge != DefaultMaxOfferAge {
		t.Errorf("MaxOfferAge = %v, want %v", mf.config.MaxOfferAge, DefaultMaxOfferAge)
	}
}

// =============================================================================
// Stale Offer Filter Tests
// =============================================================================

func TestMatchFilter_StaleOffer_Fresh(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "123",
		CreatedAt:   time.Now().Add(-1 * time.Hour), // 1 hour old
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "456",
	}

	result := mf.FilterOfferForRequest(offer, request)

	if !result.Passed {
		t.Errorf("Fresh offer should pass, got reason: %s", result.Reason)
	}
}

func TestMatchFilter_StaleOffer_Old(t *testing.T) {
	log := zerolog.Nop()
	cfg := MatchFilterConfig{
		EnableStaleFilter: true,
		MaxOfferAge:       24 * time.Hour, // 1 day
	}
	mf := NewMatchFilter(cfg, log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "123",
		CreatedAt:   time.Now().Add(-48 * time.Hour), // 2 days old
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "456",
	}

	result := mf.FilterOfferForRequest(offer, request)

	if result.Passed {
		t.Error("Stale offer should be filtered")
	}
	if result.Reason != "stale_offer" {
		t.Errorf("Reason = %s, want stale_offer", result.Reason)
	}
}

func TestMatchFilter_StaleOffer_Disabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := MatchFilterConfig{
		EnableStaleFilter:         false, // Disabled
		MaxOfferAge:               24 * time.Hour,
		EnableSameSenderExclusion: false,
	}
	mf := NewMatchFilter(cfg, log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "123",
		CreatedAt:   time.Now().Add(-48 * time.Hour), // 2 days old
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "456",
	}

	result := mf.FilterOfferForRequest(offer, request)

	if !result.Passed {
		t.Error("Stale filter disabled, should pass")
	}
}

// =============================================================================
// Same-Sender Exclusion Tests
// =============================================================================

func TestMatchFilter_SameSender_ByPhone(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "01012345678",
		SourceName:  "Ahmed",
		CreatedAt:   time.Now(),
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "01012345678", // Same phone
		SourceName:  "Ahmed",
	}

	result := mf.FilterOfferForRequest(offer, request)

	if result.Passed {
		t.Error("Same sender (by phone) should be filtered")
	}
	if result.Reason != "same_sender_phone" {
		t.Errorf("Reason = %s, want same_sender_phone", result.Reason)
	}
}

func TestMatchFilter_SameSender_ByName(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "", // No phone
		SourceName:  "Ahmed",
		CreatedAt:   time.Now(),
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "",      // No phone
		SourceName:  "Ahmed", // Same name
	}

	result := mf.FilterOfferForRequest(offer, request)

	if result.Passed {
		t.Error("Same sender (by name) should be filtered")
	}
	if result.Reason != "same_sender_name" {
		t.Errorf("Reason = %s, want same_sender_name", result.Reason)
	}
}

func TestMatchFilter_SameSender_DifferentSenders(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "01012345678",
		SourceName:  "Ahmed",
		CreatedAt:   time.Now(),
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "01098765432", // Different phone
		SourceName:  "Mohamed",
	}

	result := mf.FilterOfferForRequest(offer, request)

	if !result.Passed {
		t.Errorf("Different senders should pass, got reason: %s", result.Reason)
	}
}

func TestMatchFilter_SameSender_Disabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := MatchFilterConfig{
		EnableStaleFilter:         false,
		EnableSameSenderExclusion: false, // Disabled
	}
	mf := NewMatchFilter(cfg, log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "01012345678",
		CreatedAt:   time.Now(),
	}
	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "01012345678", // Same phone
	}

	result := mf.FilterOfferForRequest(offer, request)

	if !result.Passed {
		t.Error("Same-sender exclusion disabled, should pass")
	}
}

// =============================================================================
// FilterOffers and FilterRequests Tests
// =============================================================================

func TestMatchFilter_FilterOffers(t *testing.T) {
	log := zerolog.Nop()
	cfg := MatchFilterConfig{
		EnableStaleFilter:         true,
		MaxOfferAge:               24 * time.Hour,
		EnableSameSenderExclusion: true,
	}
	mf := NewMatchFilter(cfg, log)

	request := &entity.Request{
		ID:          "request-1",
		SourcePhone: "01098765432",
	}

	offers := []*entity.Offer{
		{ID: "offer-1", SourcePhone: "01011111111", CreatedAt: time.Now()},                      // Fresh, different sender - PASS
		{ID: "offer-2", SourcePhone: "01022222222", CreatedAt: time.Now().Add(-48 * time.Hour)}, // Stale - FAIL
		{ID: "offer-3", SourcePhone: "01098765432", CreatedAt: time.Now()},                      // Same sender - FAIL
		{ID: "offer-4", SourcePhone: "01044444444", CreatedAt: time.Now()},                      // Fresh, different sender - PASS
	}

	filtered := mf.FilterOffers(offers, request)

	if len(filtered) != 2 {
		t.Errorf("len(filtered) = %d, want 2", len(filtered))
	}

	// Verify correct offers passed
	passedIDs := make(map[string]bool)
	for _, o := range filtered {
		passedIDs[o.ID] = true
	}

	if !passedIDs["offer-1"] {
		t.Error("offer-1 should pass")
	}
	if passedIDs["offer-2"] {
		t.Error("offer-2 should be filtered (stale)")
	}
	if passedIDs["offer-3"] {
		t.Error("offer-3 should be filtered (same sender)")
	}
	if !passedIDs["offer-4"] {
		t.Error("offer-4 should pass")
	}
}

func TestMatchFilter_FilterRequests(t *testing.T) {
	log := zerolog.Nop()
	cfg := MatchFilterConfig{
		EnableStaleFilter:         true,
		MaxOfferAge:               24 * time.Hour,
		EnableSameSenderExclusion: true,
	}
	mf := NewMatchFilter(cfg, log)

	offer := &entity.Offer{
		ID:          "offer-1",
		SourcePhone: "01098765432",
		CreatedAt:   time.Now(), // Fresh offer
	}

	requests := []*entity.Request{
		{ID: "request-1", SourcePhone: "01011111111"}, // Different sender - PASS
		{ID: "request-2", SourcePhone: "01098765432"}, // Same sender - FAIL
		{ID: "request-3", SourcePhone: "01033333333"}, // Different sender - PASS
	}

	filtered := mf.FilterRequests(requests, offer)

	if len(filtered) != 2 {
		t.Errorf("len(filtered) = %d, want 2", len(filtered))
	}
}

// =============================================================================
// Configuration Methods Tests
// =============================================================================

func TestMatchFilter_SetMaxOfferAge(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	mf.SetMaxOfferAge(48 * time.Hour)

	if mf.config.MaxOfferAge != 48*time.Hour {
		t.Errorf("MaxOfferAge = %v, want 48h", mf.config.MaxOfferAge)
	}
}

func TestMatchFilter_EnableStaleFilter(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	mf.EnableStaleFilter(false)
	if mf.config.EnableStaleFilter {
		t.Error("EnableStaleFilter should be false")
	}

	mf.EnableStaleFilter(true)
	if !mf.config.EnableStaleFilter {
		t.Error("EnableStaleFilter should be true")
	}
}

func TestMatchFilter_EnableSameSenderExclusion(t *testing.T) {
	log := zerolog.Nop()
	mf := NewMatchFilter(DefaultMatchFilterConfig(), log)

	mf.EnableSameSenderExclusion(false)
	if mf.config.EnableSameSenderExclusion {
		t.Error("EnableSameSenderExclusion should be false")
	}

	mf.EnableSameSenderExclusion(true)
	if !mf.config.EnableSameSenderExclusion {
		t.Error("EnableSameSenderExclusion should be true")
	}
}

// =============================================================================
// Match Filter Constants Tests
// =============================================================================

func TestMatchFilterConstants(t *testing.T) {
	if DefaultMaxOfferAge <= 0 {
		t.Error("DefaultMaxOfferAge should be positive")
	}
	if DefaultMaxOfferAge < 24*time.Hour {
		t.Error("DefaultMaxOfferAge should be at least 24 hours")
	}
}
