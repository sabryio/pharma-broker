package parsing

import (
	"context"
	"fmt"
	"math/rand"
	"pharmabroker/domain/entity"
	"sync"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"
)

// TestParser_Stress_Concurrency simulates high load to check for race conditions and panics
func TestParser_Stress_Concurrency(t *testing.T) {
	// Setup Mocks
	mockRawRepo := &MockRawMessageRepo{}
	mockOfferRepo := &MockOfferRepo{}
	mockRequestRepo := &MockRequestRepo{}
	mockMedRepo := &MockMedicationRepo{}
	mockAI := &MockAIProvider{}
	mockQueueRepo := &MockMatchQueueRepo{}

	// Config Mocks
	mockRawRepo.OnSave = func(ctx context.Context, msg *entity.RawMessage) error { return nil }
	mockRawRepo.OnMarkProcessed = func(ctx context.Context, id string, err error) error { return nil }
	mockOfferRepo.OnSave = func(ctx context.Context, offer *entity.Offer) error { return nil }
	mockMedRepo.OnSearch = func(ctx context.Context, query string) ([]*entity.MedicationMapping, error) {
		return []*entity.MedicationMapping{
			{ArabicName: "اوجمنتين", EnglishName: "Augmentin", Embedding: []float32{0.1, 0.2}},
		}, nil
	}
	mockMedRepo.OnGetAll = func(ctx context.Context) ([]*entity.MedicationMapping, error) {
		return []*entity.MedicationMapping{
			{ArabicName: "اوجمنتين", EnglishName: "Augmentin", Embedding: []float32{0.1, 0.2}},
		}, nil
	}
	mockAI.OnParseMessages = func(ctx context.Context, messages []*entity.RawMessage, mappings []*entity.MedicationMapping) ([]*entity.AIParseResult, error) {
		// Simulate AI delay
		time.Sleep(10 * time.Millisecond)
		return []*entity.AIParseResult{
			{Items: []entity.ParsedItem{{Type: "OFFER", Medication: "Augmentin", Quantity: 5}}},
		}, nil
	}
	mockQueueRepo.OnEnqueue = func(ctx context.Context, item *entity.MatchQueueItem) error { return nil }

	// Create Parser
	parser := NewParser(
		mockRawRepo,
		mockAI,
		mockOfferRepo,
		mockRequestRepo,
		nil, // matchRepo
		mockMedRepo,
		mockQueueRepo,
		nil, // configRepo
		nil, // errorNotifier
		nil, // broadcaster
		zerolog.Nop(),
	)

	// Start Parser
	ctx := t.Context()
	parser.Start(ctx)
	defer parser.Stop()

	var wg sync.WaitGroup
	startTime := time.Now()

	// 1. Concurrent Message Processing
	workerCount := 50
	messagesPerWorker := 20
	wg.Add(workerCount)
	for i := range workerCount {
		go func(id int) {
			defer wg.Done()
			for j := 0; j < messagesPerWorker; j++ {
				msg := &entity.RawMessage{
					ID:        uuid.New().String(),
					Content:   fmt.Sprintf("Concurrent message %d-%d", id, j),
					Timestamp: time.Now(),
				}
				parser.ProcessMessage(ctx, msg)
				// Random sleep to create jitter
				time.Sleep(time.Duration(rand.Intn(5)) * time.Millisecond)
			}
		}(i)
	}

	// 2. Concurrent Embeddings Refresh
	wg.Go(func() {
		for time.Since(startTime) < 2*time.Second {
			// Simulate frequent updates to shared map
			_ = parser.embeddingCache.Refresh(ctx)
			time.Sleep(100 * time.Millisecond)
		}
	})

	// Wait for completion
	wg.Wait()
	t.Log("Stress test completed without panic")
}
