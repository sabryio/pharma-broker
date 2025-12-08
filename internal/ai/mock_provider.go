package ai

import (
	"context"
	"pharmabroker/internal/domain"
)

// MockAIProvider is a manual mock for AIProvider
type MockAIProvider struct {
	OnParseMessages func(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error)
	OnEmbed         func(ctx context.Context, text string) ([]float32, error)
	OnEmbedBatch    func(ctx context.Context, texts []string) ([][]float32, error)
}

func (m *MockAIProvider) ParseMessages(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	if m.OnParseMessages != nil {
		return m.OnParseMessages(ctx, messages, mappings)
	}
	return nil, nil // Default: return nothing
}

func (m *MockAIProvider) Embed(ctx context.Context, text string) ([]float32, error) {
	if m.OnEmbed != nil {
		return m.OnEmbed(ctx, text)
	}
	return []float32{}, nil // Default: return nothing
}

func (m *MockAIProvider) EmbedBatch(ctx context.Context, texts []string) ([][]float32, error) {
	if m.OnEmbedBatch != nil {
		return m.OnEmbedBatch(ctx, texts)
	}
	// Default: return dummy vectors for each text
	results := make([][]float32, len(texts))
	for i := range texts {
		results[i] = make([]float32, 768) // Assuming a default embedding dimension
	}
	return results, nil
}

func (m *MockAIProvider) SetMappings(mappings []*domain.MedicationMapping) {
	// No-op for mock
}
