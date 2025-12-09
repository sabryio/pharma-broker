// Package ai provides adapters for the internal AI implementation.
package ai

import (
	"context"

	"pharmabroker/matching"
)

// schedulerAdapter wraps internal/ai.LearningScheduler to implement ai.LearningScheduler
type schedulerAdapter struct {
	internal *matching.LearningScheduler
}

// WrapLearningScheduler creates an ai.LearningScheduler from internal implementation
func WrapLearningScheduler(internal *matching.LearningScheduler) LearningScheduler {
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
	var pendingWeights *matching.Weights
	if internalStatus.PendingApply != nil {
		pendingWeights = &matching.Weights{
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

func (a *schedulerAdapter) ApplyWeightsManual(ctx context.Context, weights matching.Weights, notes string) error {
	// Convert public type to internal type
	internalWeights := matching.Weights{
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
