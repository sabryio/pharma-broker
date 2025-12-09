// Package ai provides adapters for the internal AI implementation.
package ai

import (
	"context"

	internalAI "pharmabroker/internal/ai"
)

// schedulerAdapter wraps internal/ai.LearningScheduler to implement ai.LearningScheduler
type schedulerAdapter struct {
	internal *internalAI.LearningScheduler
}

// WrapLearningScheduler creates an ai.LearningScheduler from internal implementation
func WrapLearningScheduler(internal *internalAI.LearningScheduler) LearningScheduler {
	if internal == nil {
		return nil
	}
	return &schedulerAdapter{internal: internal}
}

func (a *schedulerAdapter) Start() error {
	return a.internal.Start()
}

func (a *schedulerAdapter) Stop() {
	a.internal.Stop()
}

func (a *schedulerAdapter) RunNow() error {
	return a.internal.RunNow()
}

func (a *schedulerAdapter) Status() SchedulerStatus {
	internalStatus := a.internal.Status()

	// Convert internal types to public types
	var pendingWeights *ScoringWeights
	if internalStatus.PendingApply != nil {
		pendingWeights = &ScoringWeights{
			Medication: internalStatus.PendingApply.Medication,
			Dosage:     internalStatus.PendingApply.Dosage,
			Quantity:   internalStatus.PendingApply.Quantity,
			Price:      internalStatus.PendingApply.Price,
			Recency:    internalStatus.PendingApply.Recency,
		}
	}

	return SchedulerStatus{
		Enabled:       internalStatus.Enabled,
		Schedule:      internalStatus.Schedule,
		LastRun:       internalStatus.LastRun,
		LastStatus:    JobStatus(internalStatus.LastStatus),
		LastError:     internalStatus.LastError,
		LastMetrics:   internalStatus.LastMetrics,
		PendingApply:  pendingWeights,
		PendingReason: internalStatus.PendingReason,
	}
}

func (a *schedulerAdapter) ApplyPending(ctx context.Context) error {
	return a.internal.ApplyPending(ctx)
}

func (a *schedulerAdapter) RejectPending() {
	a.internal.RejectPending()
}

func (a *schedulerAdapter) Rollback(ctx context.Context) error {
	return a.internal.Rollback(ctx)
}

func (a *schedulerAdapter) ApplyWeightsManual(ctx context.Context, weights ScoringWeights, notes string) error {
	// Convert public type to internal type
	internalWeights := internalAI.ScoringWeights{
		Medication: weights.Medication,
		Dosage:     weights.Dosage,
		Quantity:   weights.Quantity,
		Price:      weights.Price,
		Recency:    weights.Recency,
	}
	return a.internal.ApplyWeightsManual(ctx, internalWeights, notes)
}

// Verify interface implementation
var _ LearningScheduler = (*schedulerAdapter)(nil)
