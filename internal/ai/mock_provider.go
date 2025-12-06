package ai

import (
	"context"
	"pharmabroker/internal/domain"
)

// MockAIProvider is a manual mock for AIProvider
type MockAIProvider struct {
	OnParseMessages func(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error)
}

func (m *MockAIProvider) ParseMessages(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	if m.OnParseMessages != nil {
		return m.OnParseMessages(ctx, messages, mappings)
	}
	return nil, nil // Default: return nothing
}
