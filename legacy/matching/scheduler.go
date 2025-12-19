package matching

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/config"

	"github.com/robfig/cron/v3"
)

// LearningScheduler manages scheduled weight learning jobs
type LearningScheduler struct {
	learner *WeightLearner
	config  config.AdaptiveLearningConfig
	cron    *cron.Cron
	logger  *slog.Logger

	// State tracking
	mu            sync.RWMutex
	lastRun       time.Time
	lastStatus    JobStatus
	lastError     error
	lastMetrics   *entity.PerformanceMetrics
	pendingApply  *Weights // Calculated but not applied weights
	pendingReason string   // Why pending (e.g., "manual review required")
}

// JobStatus represents the status of the last learning job
type JobStatus string

const (
	JobStatusPending     JobStatus = "pending"
	JobStatusRunning     JobStatus = "running"
	JobStatusSuccess     JobStatus = "success"
	JobStatusFailed      JobStatus = "failed"
	JobStatusSkipped     JobStatus = "skipped"
	JobStatusRecommended JobStatus = "recommended" // Weights calculated but not applied
)

// NewLearningScheduler creates a new learning scheduler
func NewLearningScheduler(
	learner *WeightLearner,
	cfg config.AdaptiveLearningConfig,
	logger *slog.Logger,
) *LearningScheduler {
	if logger == nil {
		logger = slog.Default()
	}

	return &LearningScheduler{
		learner:    learner,
		config:     cfg,
		logger:     logger,
		lastStatus: JobStatusPending,
	}
}

// Start begins the scheduled learning jobs
func (ls *LearningScheduler) Start() error {
	if !ls.config.Enabled {
		ls.logger.Info("Adaptive learning is disabled")
		return nil
	}

	ls.cron = cron.New()

	_, err := ls.cron.AddFunc(ls.config.Schedule, ls.runJob)
	if err != nil {
		return fmt.Errorf("invalid cron schedule '%s': %w", ls.config.Schedule, err)
	}

	ls.cron.Start()
	ls.logger.Info("Learning scheduler started",
		"schedule", ls.config.Schedule,
		"auto_apply", ls.config.AutoApply.Enabled,
	)

	return nil
}

// Stop gracefully stops the scheduler
func (ls *LearningScheduler) Stop() {
	if ls.cron != nil {
		ctx := ls.cron.Stop()
		<-ctx.Done()
		ls.logger.Info("Learning scheduler stopped")
	}
}

// RunNow triggers an immediate learning job (for manual/testing)
func (ls *LearningScheduler) RunNow() error {
	ls.runJob()

	ls.mu.RLock()
	defer ls.mu.RUnlock()

	if ls.lastError != nil {
		return ls.lastError
	}
	return nil
}

// runJob is the main job execution function
func (ls *LearningScheduler) runJob() {
	ls.mu.Lock()
	ls.lastRun = time.Now()
	ls.lastStatus = JobStatusRunning
	ls.lastError = nil
	ls.mu.Unlock()

	ctx := context.Background()
	logger := ls.logger.With("job", "learning", "started_at", ls.lastRun)

	logger.Info("Learning job started")

	// Calculate date range from config
	endDate := time.Now()
	startDate := endDate.Add(-time.Duration(ls.config.Algorithm.AnalysisWindowDays) * 24 * time.Hour)

	// Calculate optimal weights
	newWeights, newMetrics, err := ls.learner.CalculateOptimalWeights(ctx, startDate, endDate)
	if err != nil {
		ls.handleError(err, logger)
		return
	}

	logger.Info("Weights calculated",
		"sample_size", newMetrics.SampleSize,
		"confirmation_rate", fmt.Sprintf("%.2f%%", newMetrics.ConfirmationRate*100),
	)

	// Get current weights for comparison
	currentWeights := ls.learner.scorer.GetWeights()
	currentMetrics := ls.getCurrentMetrics(ctx)

	// Decide whether to apply
	shouldApply := ls.shouldApply(currentMetrics, *newMetrics)

	if shouldApply && ls.config.AutoApply.Enabled {
		// Auto-apply the weights
		err = ls.learner.ApplyWeights(
			ctx,
			*newWeights,
			entity.WeightSourceAutoLearned,
			newMetrics,
			ls.buildNote(currentWeights, *newWeights, *newMetrics),
		)
		if err != nil {
			ls.handleError(err, logger)
			return
		}

		ls.mu.Lock()
		ls.lastStatus = JobStatusSuccess
		ls.lastMetrics = newMetrics
		ls.pendingApply = nil
		ls.pendingReason = ""
		ls.mu.Unlock()

		logger.Info("Weights applied successfully",
			"old_weights", currentWeights,
			"new_weights", *newWeights,
		)

		if ls.config.Notifications.OnSuccess {
			ls.notifySuccess(*newWeights, *newMetrics)
		}
	} else {
		// Store for manual review
		reason := ls.getSkipReason(shouldApply)

		ls.mu.Lock()
		ls.lastStatus = JobStatusRecommended
		ls.lastMetrics = newMetrics
		ls.pendingApply = newWeights
		ls.pendingReason = reason
		ls.mu.Unlock()

		logger.Info("Weights calculated but not applied",
			"reason", reason,
			"recommended_weights", *newWeights,
		)

		if ls.config.Notifications.OnRecommendation {
			ls.notifyRecommendation(*newWeights, *newMetrics, reason)
		}
	}
}

// shouldApply determines if weights should be auto-applied
func (ls *LearningScheduler) shouldApply(current, new entity.PerformanceMetrics) bool {
	if !ls.config.AutoApply.RequireImprovement {
		return true
	}

	// Calculate separation (confirmed - rejected avg scores)
	currentSep := current.AvgScoreConfirmed - current.AvgScoreRejected
	newSep := new.AvgScoreConfirmed - new.AvgScoreRejected

	// Check separation gain
	sepGain := newSep - currentSep
	if sepGain < ls.config.AutoApply.MinSeparationGain {
		return false
	}

	// Check confirmation rate drop
	rateDrop := current.ConfirmationRate - new.ConfirmationRate
	return rateDrop <= ls.config.AutoApply.MaxConfirmationRateDrop
}

// getSkipReason explains why weights weren't applied
func (ls *LearningScheduler) getSkipReason(shouldApply bool) string {
	if !ls.config.AutoApply.Enabled {
		return "auto-apply disabled, manual review required"
	}
	if !shouldApply {
		return "performance improvement threshold not met"
	}
	return "unknown"
}

// handleError processes errors and updates state
func (ls *LearningScheduler) handleError(err error, logger *slog.Logger) {
	ls.mu.Lock()
	ls.lastStatus = JobStatusFailed
	ls.lastError = err
	ls.mu.Unlock()

	logger.Error("Learning job failed", "error", err)

	if ls.config.Notifications.OnFailure {
		ls.notifyFailure(err)
	}
}

// getCurrentMetrics fetches or estimates current performance metrics
func (ls *LearningScheduler) getCurrentMetrics(ctx context.Context) entity.PerformanceMetrics {
	// Try to get from last run or estimate from current data
	ls.mu.RLock()
	if ls.lastMetrics != nil {
		metrics := *ls.lastMetrics
		ls.mu.RUnlock()
		return metrics
	}
	ls.mu.RUnlock()

	// Return neutral metrics if no history
	return entity.PerformanceMetrics{
		ConfirmationRate:  0.5,
		AvgScoreConfirmed: 0.7,
		AvgScoreRejected:  0.3,
	}
}

// buildNote creates a descriptive note for the weight change
func (ls *LearningScheduler) buildNote(old, new Weights, metrics entity.PerformanceMetrics) string {
	return fmt.Sprintf(
		"Auto-learned from %d samples. Separation: %.3f. "+
			"Weights changed: med %.2f→%.2f, dosage %.2f→%.2f, qty %.2f→%.2f, price %.2f→%.2f, recency %.2f→%.2f",
		metrics.SampleSize,
		metrics.AvgScoreConfirmed-metrics.AvgScoreRejected,
		old.Medication, new.Medication,
		old.Dosage, new.Dosage,
		old.Quantity, new.Quantity,
		old.Price, new.Price,
		old.Recency, new.Recency,
	)
}

// Notification stubs (can be expanded with actual notification implementation)
func (ls *LearningScheduler) notifySuccess(weights Weights, metrics entity.PerformanceMetrics) {
	ls.logger.Info("NOTIFICATION: Weights applied",
		"new_weights", weights,
		"metrics", metrics,
	)
}

func (ls *LearningScheduler) notifyFailure(err error) {
	ls.logger.Error("NOTIFICATION: Learning failed",
		"error", err,
	)
}

func (ls *LearningScheduler) notifyRecommendation(weights Weights, metrics entity.PerformanceMetrics, reason string) {
	ls.logger.Info("NOTIFICATION: New weights recommended",
		"recommended_weights", weights,
		"metrics", metrics,
		"reason", reason,
	)
}

// Status returns the current scheduler status
func (ls *LearningScheduler) Status() SchedulerStatus {
	ls.mu.RLock()
	defer ls.mu.RUnlock()

	return SchedulerStatus{
		Enabled:       ls.config.Enabled,
		Schedule:      ls.config.Schedule,
		LastRun:       ls.lastRun,
		LastStatus:    ls.lastStatus,
		LastError:     ls.lastError,
		LastMetrics:   ls.lastMetrics,
		PendingApply:  ls.pendingApply,
		PendingReason: ls.pendingReason,
	}
}

// SchedulerStatus provides current scheduler state
type SchedulerStatus struct {
	Enabled       bool
	Schedule      string
	LastRun       time.Time
	LastStatus    JobStatus
	LastError     error
	LastMetrics   *entity.PerformanceMetrics
	PendingApply  *Weights
	PendingReason string
}

// ApplyPending manually applies pending weights
func (ls *LearningScheduler) ApplyPending(ctx context.Context) error {
	ls.mu.Lock()
	pending := ls.pendingApply
	ls.mu.Unlock()

	if pending == nil {
		return fmt.Errorf("no pending weights to apply")
	}

	metrics := ls.getCurrentMetrics(ctx)

	err := ls.learner.ApplyWeights(
		ctx,
		*pending,
		entity.WeightSourceManual, // Since manually triggered
		&metrics,
		"Manually approved from scheduler recommendation",
	)
	if err != nil {
		return err
	}

	ls.mu.Lock()
	ls.pendingApply = nil
	ls.pendingReason = ""
	ls.lastStatus = JobStatusSuccess
	ls.mu.Unlock()

	ls.logger.Info("Pending weights applied manually", "weights", pending)

	return nil
}

// RejectPending clears pending weights without applying
func (ls *LearningScheduler) RejectPending() {
	ls.mu.Lock()
	defer ls.mu.Unlock()

	ls.pendingApply = nil
	ls.pendingReason = "rejected by user"
	ls.lastStatus = JobStatusSkipped

	ls.logger.Info("Pending weights rejected")
}

// UpdateConfig updates the scheduler configuration
func (ls *LearningScheduler) UpdateConfig(cfg config.AdaptiveLearningConfig) error {
	ls.mu.Lock()
	defer ls.mu.Unlock()

	ls.config = cfg

	// Also update learner config
	ls.learner.SetConfig(LearningConfig{
		LearningRate:   cfg.Algorithm.LearningRate,
		MinWeight:      cfg.Algorithm.MinWeight,
		MaxWeight:      cfg.Algorithm.MaxWeight,
		MinChange:      cfg.Algorithm.MinChange,
		MinSamples:     cfg.Algorithm.MinSamples,
		AnalysisWindow: cfg.Algorithm.AnalysisWindowDays,
	})

	ls.logger.Info("Scheduler config updated", "config", cfg)

	return nil
}

// Rollback reverts to the previous weight configuration
func (ls *LearningScheduler) Rollback(ctx context.Context) error {
	if ls.learner == nil {
		return fmt.Errorf("learner not configured")
	}

	err := ls.learner.Rollback(ctx)
	if err != nil {
		ls.logger.Error("Rollback failed", "error", err)
		return err
	}

	ls.mu.Lock()
	ls.lastStatus = JobStatusSuccess
	ls.pendingApply = nil
	ls.pendingReason = "rolled back to previous weights"
	ls.mu.Unlock()

	ls.logger.Info("Weights rolled back to previous configuration")

	return nil
}

// ApplyWeightsManual applies weights directly with manual source
func (ls *LearningScheduler) ApplyWeightsManual(ctx context.Context, weights Weights, notes string) error {
	if ls.learner == nil {
		return fmt.Errorf("learner not configured")
	}

	// Get current metrics for context
	metrics := ls.getCurrentMetrics(ctx)

	err := ls.learner.ApplyWeights(
		ctx,
		weights,
		entity.WeightSourceManual,
		&metrics,
		notes,
	)
	if err != nil {
		return err
	}

	ls.mu.Lock()
	ls.lastStatus = JobStatusSuccess
	ls.pendingApply = nil
	ls.pendingReason = ""
	ls.mu.Unlock()

	ls.logger.Info("Manual weights applied", "weights", weights, "notes", notes)

	return nil
}

// GetLearner returns the underlying weight learner for direct operations
func (ls *LearningScheduler) GetLearner() *WeightLearner {
	return ls.learner
}
