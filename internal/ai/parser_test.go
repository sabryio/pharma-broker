package ai

import (
	"context"
	"errors"
	"fmt"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
)

func TestParser_ProcessBatch_HappyPath(t *testing.T) {
	// Setup Mocks
	mockRawRepo := &MockRawMessageRepo{}
	mockOfferRepo := &MockOfferRepo{}
	mockRequestRepo := &MockRequestRepo{}
	mockMedRepo := &MockMedicationRepo{}
	mockAI := &MockAIProvider{}
	mockQueueRepo := &MockMatchQueueRepo{}

	// Create Parser
	// Create Parser
	parser := NewParser(
		mockRawRepo,
		mockAI,
		mockOfferRepo,
		mockRequestRepo,
		nil, // matchRepo (added)
		mockMedRepo,
		mockQueueRepo,
		nil, // configRepo
		nil, // errorNotifier
		nil, // broadcaster
		zerolog.Nop(),
	)

	// Test Data
	msg := &domain.RawMessage{
		ID:        "msg-1",
		Content:   "I have Panadol",
		Timestamp: time.Now(),
	}

	parsedItem := domain.ParsedItem{
		Type:       domain.MessageTypeOffer,
		Medication: "Panadol",
		Quantity:   10,
	}

	// Expectations
	offerSaved := false
	msgMarked := false

	// FTS Mock
	mockMedRepo.OnSearch = func(ctx context.Context, query string) ([]*domain.MedicationMapping, error) {
		return []*domain.MedicationMapping{}, nil
	}

	mockAI.OnParseMessages = func(ctx context.Context, messages []*domain.RawMessage, mappings []*domain.MedicationMapping) ([]*domain.AIParseResult, error) {
		return []*domain.AIParseResult{
			{
				Items: []domain.ParsedItem{parsedItem},
			},
		}, nil
	}

	mockOfferRepo.OnSave = func(ctx context.Context, offer *domain.Offer) error {
		if offer.Medication == "Panadol" {
			offerSaved = true
		}
		return nil
	}

	// Mock Search for matches (avoid panic in go routine)
	mockRequestRepo.OnSearch = func(ctx context.Context, query string, limit, offset int) ([]*domain.Request, error) {
		return []*domain.Request{}, nil
	}

	mockRawRepo.OnMarkProcessed = func(ctx context.Context, id string, err error) error {
		if id == "msg-1" && err == nil {
			msgMarked = true
		}
		return nil
	}

	// Execute (Privately, via white-box testing since in same package)
	parser.processBatch(context.Background(), []*domain.RawMessage{msg})

	// Wait for async matching (if any)
	time.Sleep(50 * time.Millisecond)

	// Verify
	if !offerSaved {
		t.Error("Expected OfferRepo.Save to be called")
	}
	if !msgMarked {
		t.Error("Expected RawMessageRepo.MarkProcessed to be called with nil failure")
	}
}

func TestParser_ProcessBatch_AIError(t *testing.T) {
	// Setup Mocks
	mockRawRepo := &MockRawMessageRepo{}
	mockOfferRepo := &MockOfferRepo{}
	mockRequestRepo := &MockRequestRepo{}
	mockMedRepo := &MockMedicationRepo{}
	mockAI := &MockAIProvider{}
	mockQueueRepo := &MockMatchQueueRepo{}

	parser := NewParser(
		mockRawRepo,
		mockAI,
		mockOfferRepo,
		mockRequestRepo,
		nil, // matchRepo (added)
		mockMedRepo,
		mockQueueRepo,
		nil, // configRepo
		nil, // errorNotifier
		nil, // broadcaster
		zerolog.Nop(),
	)

	msg := &domain.RawMessage{ID: "msg-error"}

	// Expectations
	mockMedRepo.OnSearch = func(ctx context.Context, query string) ([]*domain.MedicationMapping, error) {
		return []*domain.MedicationMapping{}, nil
	}

	mockAI.OnParseMessages = func(ctx context.Context, messages []*domain.RawMessage, mappings []*domain.MedicationMapping) ([]*domain.AIParseResult, error) {
		return nil, errors.New("AI overloaded")
	}

	mockRawRepo.OnMarkProcessed = func(ctx context.Context, id string, err error) error {
		if id == "msg-error" && err != nil && err.Error() == "AI overloaded" {
			return nil
		}
		t.Errorf("MarkProcessed called with unexpected args: id=%s, err=%v", id, err)
		return nil
	}

	// Expect Enqueue instead of internal channel
	mockQueueRepo.OnEnqueue = func(ctx context.Context, item *domain.MatchQueueItem) error {
		if item.SourceType == "OFFER" && item.SourceID == "offer-123" {
			return nil
		}
		return fmt.Errorf("unexpected enqueue")
	}

	mockOfferRepo.OnSave = func(ctx context.Context, offer *domain.Offer) error {
		offer.ID = "offer-123" // Simulate ID generation
		return nil
	}

	// Execute via ProcessMessage (new entry point)
	parser.ProcessMessage(context.Background(), msg)

	// Wait for workers (async) - in unit test, we might typically call processBatch directly
	// But let's verify processBatch logic directly for simplicity as before
	parser.processBatch(context.Background(), []*domain.RawMessage{msg})
}

func TestGenerateTrigrams(t *testing.T) {
	tests := []struct {
		input    string
		expected []string
	}{
		{"abc", []string{"abc"}},
		{"abcd", []string{"abc", "bcd"}},
		{"hello", []string{"hel", "ell", "llo"}},
		{"ab", nil}, // Too short
		{"", nil},   // Empty
		{"اوجمنتين", []string{"اوج", "وجم", "جمن", "منت", "نتي", "تين"}}, // Arabic
	}

	for _, tt := range tests {
		result := generateTrigrams(tt.input)
		if len(result) != len(tt.expected) {
			t.Errorf("For input '%s', expected %d trigrams, got %d", tt.input, len(tt.expected), len(result))
		}
		for i, v := range result {
			if v != tt.expected[i] {
				t.Errorf("For input '%s', expected trigram %d to be '%s', got '%s'", tt.input, i, tt.expected[i], v)
			}
		}
	}
}

func TestParser_GetRelevantMappings_Fuzzy(t *testing.T) {
	// Setup Mocks
	mockMedRepo := &MockMedicationRepo{}
	mockQueueRepo := &MockMatchQueueRepo{}

	parser := &Parser{
		log:            zerolog.Nop(),
		medicationRepo: mockMedRepo,
		matchQueueRepo: mockQueueRepo,
		// matchQueueRepo not used here
	}

	messages := []*domain.RawMessage{
		{Content: "augmentin"}, // Typos handled by trigrams? "augmentin" -> "aug", "ugm"...
	}

	// 1. Exact search returns nothing
	// 2. Fuzzy search returns result

	callCount := 0
	mockMedRepo.OnSearch = func(ctx context.Context, query string) ([]*domain.MedicationMapping, error) {
		callCount++
		if callCount == 1 {
			// Exact match query
			return []*domain.MedicationMapping{}, nil
		}
		if callCount == 2 {
			// Fuzzy match query
			// Should contain trigrams for "augmentin"
			if len(query) == 0 {
				t.Error("Expected fuzzy query to be non-empty")
			}
			return []*domain.MedicationMapping{
				{ArabicName: "اوجمنتين", EnglishName: "Augmentin"},
			}, nil
		}
		return nil, nil
	}

	mappings := parser.getRelevantMappings(context.Background(), messages)

	if len(mappings) != 1 {
		t.Errorf("Expected 1 mapping, got %d", len(mappings))
	}
	if val, ok := mappings["اوجمنتين"]; !ok || val != "Augmentin" {
		t.Error("Expected correctly mapped fuzzy result")
	}
}
