// Package ai defines the AI provider interface for message parsing.
// This package contains the provider interface and factory function.
package ai

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/matching"
)

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
	PendingApply  *matching.Weights
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
	ApplyWeightsManual(ctx context.Context, weights matching.Weights, notes string) error
}
