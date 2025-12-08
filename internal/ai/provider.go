package ai

import (
	"context"
	"fmt"

	"github.com/rs/zerolog"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

// AIProvider defines the interface for AI-based message parsing.
// This abstraction allows switching between different AI backends
// like Google Gemini or Docker Model Runner (local LLMs).
type AIProvider interface {
	// ParseMessages parses raw WhatsApp messages and extracts
	// pharmaceutical offers and requests.
	ParseMessages(ctx context.Context, messages []*domain.RawMessage, mappings []*domain.MedicationMapping) ([]*domain.AIParseResult, error)

	// Embed generates a vector embedding for the given text.
	Embed(ctx context.Context, text string) ([]float32, error)

	// EmbedBatch generates vector embeddings for a batch of texts.
	EmbedBatch(ctx context.Context, texts []string) ([][]float32, error)

	// SetMappings configures the full medication mappings for hybrid RAG filtering.
	// This enables keyword + vector similarity search to reduce prompt size.
	SetMappings(mappings []*domain.MedicationMapping)

	// SetUnmappedRepo configures the repository for saving unmapped medications.
	// This enables active learning - unmapped terms are saved for human review.
	SetUnmappedRepo(repo domain.UnmappedMedicationRepo)
}

// NewAIProvider creates an AI provider based on configuration.
// Returns either a GeminiClient or DockerModelClient depending on cfg.AI.Provider.
func NewAIProvider(ctx context.Context, cfg *config.Config, log zerolog.Logger) (AIProvider, error) {
	switch cfg.AI.Provider {
	case "gemini":
		log.Info().Str("provider", "gemini").Str("model", cfg.Gemini.Model).Msg("Using Gemini AI provider")
		return NewGeminiClient(ctx, &cfg.Gemini, log)

	case "docker":
		log.Info().
			Str("provider", "docker").
			Str("model", cfg.DockerModel.Model).
			Str("base_url", cfg.DockerModel.BaseURL).
			Msg("Using Docker Model Runner AI provider")
		return NewDockerModelClient(&cfg.DockerModel, log)

	default:
		return nil, fmt.Errorf("unknown AI provider: %s (use 'gemini' or 'docker')", cfg.AI.Provider)
	}
}
