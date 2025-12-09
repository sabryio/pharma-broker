// Package ai defines the AI provider interface for message parsing.
// This package contains the provider interface and factory function.
package ai

import (
	"context"
	"time"

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

// =============================================================================
// Learning System Types
// =============================================================================

// ScoringWeights holds configurable weights for each scoring field
type ScoringWeights struct {
	Medication float64 `json:"medication"` // Default: 0.45
	Dosage     float64 `json:"dosage"`     // Default: 0.10
	Quantity   float64 `json:"quantity"`   // Default: 0.20
	Price      float64 `json:"price"`      // Default: 0.15
	Recency    float64 `json:"recency"`    // Default: 0.10
}

// DefaultWeights returns the default scoring weights
func DefaultWeights() ScoringWeights {
	return ScoringWeights{
		Medication: 0.45,
		Dosage:     0.10,
		Quantity:   0.20,
		Price:      0.15,
		Recency:    0.10,
	}
}

// JobStatus represents the status of a learning job
type JobStatus string

const (
	JobStatusPending     JobStatus = "pending"
	JobStatusRunning     JobStatus = "running"
	JobStatusSuccess     JobStatus = "success"
	JobStatusFailed      JobStatus = "failed"
	JobStatusSkipped     JobStatus = "skipped"
	JobStatusRecommended JobStatus = "recommended" // Weights calculated but not applied
)

// SchedulerStatus provides current scheduler state
type SchedulerStatus struct {
	Enabled       bool
	Schedule      string
	LastRun       time.Time
	LastStatus    JobStatus
	LastError     error
	LastMetrics   *entity.PerformanceMetrics
	PendingApply  *ScoringWeights
	PendingReason string
}

// LearningScheduler defines the interface for adaptive weight learning system
type LearningScheduler interface {
	// Start begins the scheduled learning jobs
	Start() error

	// Stop gracefully stops the scheduler
	Stop()

	// RunNow triggers an immediate learning job
	RunNow() error

	// Status returns the current scheduler status
	Status() SchedulerStatus

	// ApplyPending manually applies pending weights
	ApplyPending(ctx context.Context) error

	// RejectPending clears pending weights without applying
	RejectPending()

	// Rollback reverts to the previous weight configuration
	Rollback(ctx context.Context) error

	// ApplyWeightsManual applies weights directly with manual source
	ApplyWeightsManual(ctx context.Context, weights ScoringWeights, notes string) error
}
