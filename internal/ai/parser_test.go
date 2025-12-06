package ai

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

func TestParser_ProcessBatch_HappyPath(t *testing.T) {
	// Setup Mocks
	mockRawRepo := &MockRawMessageRepo{}
	mockOfferRepo := &MockOfferRepo{}
	mockRequestRepo := &MockRequestRepo{}
	mockMatchRepo := &MockMatchRepo{}
	mockMedRepo := &MockMedicationRepo{}
	mockAI := &MockAIProvider{}

	// Setup Config
	cfg := &config.ParserConfig{
		BatchInterval:     100 * time.Millisecond,
		MatchThreshold:    0.5,
		MessageBufferSize: 10,
	}

	// Create Parser
	parser := NewParser(
		mockAI,
		mockRawRepo,
		mockOfferRepo,
		mockRequestRepo,
		mockMatchRepo,
		mockMedRepo,
		make(chan *domain.RawMessage),
		cfg,
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

	mockMedRepo.OnGetAll = func(ctx context.Context) ([]*domain.MedicationMapping, error) {
		return []*domain.MedicationMapping{}, nil
	}

	mockAI.OnParseMessages = func(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
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

func TestParser_FilterRelevantMappings(t *testing.T) {
	parser := &Parser{} // Empty parser, we only need the method

	allMappings := map[string]string{
		"اوجمنتين": "Augmentin",
		"بنادول":   "Panadol",
		"فلاجيل":   "Flagyl",
		"ادول":     "Adol",
	}

	messages := []*domain.RawMessage{
		{Content: "I need 2 boxes of Augmentin (اوجمنتين) please"},
		{Content: "Also do you have adol?"},
	}

	relevant := parser.filterRelevantMappings(messages, allMappings)

	if len(relevant) != 2 {
		t.Errorf("Expected 2 relevant mappings, got %d", len(relevant))
	}
	if _, ok := relevant["اوجمنتين"]; !ok {
		t.Error("Expected 'اوجمنتين' to be in relevant mappings")
	}
	if _, ok := relevant["ادول"]; !ok {
		t.Error("Expected 'ادول' to be in relevant mappings")
	}
	if _, ok := relevant["بنادول"]; ok {
		t.Error("Expected 'بنادول' NOT to be in relevant mappings")
	}
}

func TestParser_ProcessBatch_AIError(t *testing.T) {
	// Setup Mocks
	mockRawRepo := &MockRawMessageRepo{}
	mockOfferRepo := &MockOfferRepo{}
	mockRequestRepo := &MockRequestRepo{}
	mockMatchRepo := &MockMatchRepo{}
	mockMedRepo := &MockMedicationRepo{}
	mockAI := &MockAIProvider{}

	cfg := &config.ParserConfig{BatchInterval: 100 * time.Millisecond}

	parser := NewParser(
		mockAI,
		mockRawRepo,
		mockOfferRepo,
		mockRequestRepo,
		mockMatchRepo,
		mockMedRepo,
		make(chan *domain.RawMessage),
		cfg,
		zerolog.Nop(),
	)

	msg := &domain.RawMessage{ID: "msg-error"}

	// Expectations
	mockMedRepo.OnGetAll = func(ctx context.Context) ([]*domain.MedicationMapping, error) {
		return []*domain.MedicationMapping{}, nil
	}

	mockAI.OnParseMessages = func(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
		return nil, errors.New("AI overloaded")
	}

	mockRawRepo.OnMarkProcessed = func(ctx context.Context, id string, err error) error {
		if id == "msg-error" && err != nil && err.Error() == "AI overloaded" {
			return nil
		}
		t.Errorf("MarkProcessed called with unexpected args: id=%s, err=%v", id, err)
		return nil
	}

	// Execute
	parser.processBatch(context.Background(), []*domain.RawMessage{msg})
}
