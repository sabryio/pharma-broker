// Package ai defines the AI provider interface for message parsing.
// This package contains the provider interface and factory function.
package ai

import (
	"context"

	"pharmabroker/domain/entity"
)

// Provider defines the interface for AI-based message parsing.
// This abstraction allows switching between different AI backends
// like Google Gemini or Docker Model Runner (local LLMs).
type Provider interface {
	// ParseMessages parses raw WhatsApp messages and extracts
	// pharmaceutical offers and requests.
	ParseMessages(ctx context.Context, messages []*entity.RawMessage, mappings []*entity.MedicationMapping) ([]*entity.AIParseResult, error)

	// Embed generates a vector embedding for the given text.
	Embed(ctx context.Context, text string) ([]float32, error)

	// EmbedBatch generates vector embeddings for a batch of texts.
	EmbedBatch(ctx context.Context, texts []string) ([][]float32, error)

	// SetMappings configures the full medication mappings for hybrid RAG filtering.
	SetMappings(mappings []*entity.MedicationMapping)

	// SetUnmappedRepo configures the repository for saving unmapped medications.
	SetUnmappedRepo(repo UnmappedMedicationRepo)
}

// UnmappedMedicationRepo handles storage of medications not found in mappings
type UnmappedMedicationRepo interface {
	Save(ctx context.Context, medication *entity.UnmappedMedication) error
	GetPending(ctx context.Context, limit, offset int) ([]*entity.UnmappedMedication, error)
}

// MatchConfidence indicates how a medication match was made
type MatchConfidence string

const (
	ConfidenceExact          MatchConfidence = "EXACT"
	ConfidenceFuzzy          MatchConfidence = "FUZZY"
	ConfidenceVector         MatchConfidence = "VECTOR"
	ConfidenceTransliterated MatchConfidence = "TRANSLITERATED"
)
