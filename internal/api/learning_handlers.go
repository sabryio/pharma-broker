package api

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/domain"
)

// LearningHandlers handles adaptive learning admin endpoints
type LearningHandlers struct {
	scheduler         *ai.LearningScheduler
	feedbackRepo      LearningFeedbackRepository
	weightHistoryRepo LearningWeightHistoryRepository
}

// LearningFeedbackRepository interface for learning feedback
type LearningFeedbackRepository interface {
	GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*domain.FeedbackStats, error)
	CountByAction(ctx context.Context, action domain.FeedbackAction, startDate, endDate time.Time) (int, error)
}

// LearningWeightHistoryRepository interface for weight history
type LearningWeightHistoryRepository interface {
	GetCurrent(ctx context.Context) (*domain.WeightHistory, error)
	GetHistory(ctx context.Context, limit int) ([]*domain.WeightHistory, error)
	GetBySource(ctx context.Context, source domain.WeightSource, limit int) ([]*domain.WeightHistory, error)
}

// NewLearningHandlers creates learning admin handlers
func NewLearningHandlers(
	scheduler *ai.LearningScheduler,
	feedbackRepo LearningFeedbackRepository,
	weightHistoryRepo LearningWeightHistoryRepository,
) *LearningHandlers {
	return &LearningHandlers{
		scheduler:         scheduler,
		feedbackRepo:      feedbackRepo,
		weightHistoryRepo: weightHistoryRepo,
	}
}

// LearningStatusResponse represents the learning system status
type LearningStatusResponse struct {
	Enabled        bool                       `json:"enabled"`
	Schedule       string                     `json:"schedule"`
	LastRun        *time.Time                 `json:"last_run,omitempty"`
	LastStatus     string                     `json:"last_status"`
	LastError      string                     `json:"last_error,omitempty"`
	LastMetrics    *domain.PerformanceMetrics `json:"last_metrics,omitempty"`
	PendingApply   *ai.ScoringWeights         `json:"pending_weights,omitempty"`
	PendingReason  string                     `json:"pending_reason,omitempty"`
	CurrentWeights *ai.ScoringWeights         `json:"current_weights,omitempty"`
}

// GetLearningStatus returns current learning system status
// GET /api/admin/learning/status
func (h *LearningHandlers) GetLearningStatus(w http.ResponseWriter, r *http.Request) {
	if h.scheduler == nil {
		http.Error(w, "Learning scheduler not configured", http.StatusServiceUnavailable)
		return
	}

	status := h.scheduler.Status()

	response := LearningStatusResponse{
		Enabled:       status.Enabled,
		Schedule:      status.Schedule,
		LastStatus:    string(status.LastStatus),
		PendingApply:  status.PendingApply,
		PendingReason: status.PendingReason,
		LastMetrics:   status.LastMetrics,
	}

	if !status.LastRun.IsZero() {
		response.LastRun = &status.LastRun
	}

	if status.LastError != nil {
		response.LastError = status.LastError.Error()
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// TriggerLearningRequest for manual trigger
type TriggerLearningRequest struct {
	Force bool `json:"force"` // Force run even if recently ran
}

// TriggerLearningResponse after manual trigger
type TriggerLearningResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message"`
	Status  string `json:"status"`
}

// TriggerLearning manually triggers a learning job
// POST /api/admin/learning/trigger
func (h *LearningHandlers) TriggerLearning(w http.ResponseWriter, r *http.Request) {
	if h.scheduler == nil {
		http.Error(w, "Learning scheduler not configured", http.StatusServiceUnavailable)
		return
	}

	err := h.scheduler.RunNow()

	response := TriggerLearningResponse{
		Success: err == nil,
		Status:  string(h.scheduler.Status().LastStatus),
	}

	if err != nil {
		response.Message = err.Error()
	} else {
		response.Message = "Learning job completed successfully"
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// ApplyPendingRequest for applying pending weights
type ApplyPendingRequest struct {
	Confirm bool `json:"confirm"` // Must be true to apply
}

// ApplyPendingWeights applies pending weights manually
// POST /api/admin/learning/apply
func (h *LearningHandlers) ApplyPendingWeights(w http.ResponseWriter, r *http.Request) {
	if h.scheduler == nil {
		http.Error(w, "Learning scheduler not configured", http.StatusServiceUnavailable)
		return
	}

	var req ApplyPendingRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Invalid request body", http.StatusBadRequest)
		return
	}

	if !req.Confirm {
		http.Error(w, "Must set confirm=true to apply weights", http.StatusBadRequest)
		return
	}

	ctx := r.Context()
	err := h.scheduler.ApplyPending(ctx)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "success",
		"message": "Pending weights applied successfully",
	})
}

// RejectPendingWeights rejects pending weights
// POST /api/admin/learning/reject
func (h *LearningHandlers) RejectPendingWeights(w http.ResponseWriter, r *http.Request) {
	if h.scheduler == nil {
		http.Error(w, "Learning scheduler not configured", http.StatusServiceUnavailable)
		return
	}

	h.scheduler.RejectPending()

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "success",
		"message": "Pending weights rejected",
	})
}

// RollbackWeights reverts to previous weights
// POST /api/admin/learning/rollback
func (h *LearningHandlers) RollbackWeights(w http.ResponseWriter, r *http.Request) {
	if h.scheduler == nil {
		http.Error(w, "Learning scheduler not configured", http.StatusServiceUnavailable)
		return
	}

	ctx := r.Context()

	// Get current weights before rollback for response
	currentWeights := h.scheduler.Status().PendingApply

	// Perform actual rollback via scheduler
	err := h.scheduler.Rollback(ctx)
	if err != nil {
		http.Error(w, "Rollback failed: "+err.Error(), http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":           "success",
		"message":          "Weights rolled back to previous configuration",
		"rolled_back_from": currentWeights,
	})
}

// WeightHistoryResponse for weight history endpoint
type WeightHistoryResponse struct {
	History []*WeightHistoryItem `json:"history"`
	Total   int                  `json:"total"`
}

// WeightHistoryItem represents a weight change
type WeightHistoryItem struct {
	ID               string                     `json:"id"`
	Timestamp        time.Time                  `json:"timestamp"`
	Source           string                     `json:"source"`
	MedicationWeight float64                    `json:"medication_weight"`
	DosageWeight     float64                    `json:"dosage_weight"`
	QuantityWeight   float64                    `json:"quantity_weight"`
	PriceWeight      float64                    `json:"price_weight"`
	RecencyWeight    float64                    `json:"recency_weight"`
	Metrics          *domain.PerformanceMetrics `json:"metrics,omitempty"`
	Notes            string                     `json:"notes,omitempty"`
}

// GetWeightHistory returns historical weight changes
// GET /api/admin/learning/history
func (h *LearningHandlers) GetWeightHistory(w http.ResponseWriter, r *http.Request) {
	if h.weightHistoryRepo == nil {
		http.Error(w, "Weight history not configured", http.StatusServiceUnavailable)
		return
	}

	ctx := r.Context()
	limit := 20 // Default limit

	history, err := h.weightHistoryRepo.GetHistory(ctx, limit)
	if err != nil {
		http.Error(w, "Failed to fetch history: "+err.Error(), http.StatusInternalServerError)
		return
	}

	items := make([]*WeightHistoryItem, 0, len(history))
	for _, h := range history {
		items = append(items, &WeightHistoryItem{
			ID:               h.ID,
			Timestamp:        h.AppliedAt,
			Source:           string(h.Source),
			MedicationWeight: h.MedicationWeight,
			DosageWeight:     h.DosageWeight,
			QuantityWeight:   h.QuantityWeight,
			PriceWeight:      h.PriceWeight,
			RecencyWeight:    h.RecencyWeight,
			Notes:            h.Notes,
		})
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(WeightHistoryResponse{
		History: items,
		Total:   len(items),
	})
}

// FeedbackStatsResponse for feedback statistics
type FeedbackStatsResponse struct {
	Period           string  `json:"period"`
	TotalFeedbacks   int     `json:"total"`
	ConfirmedCount   int     `json:"confirmed"`
	RejectedCount    int     `json:"rejected"`
	ConfirmationRate float64 `json:"confirmation_rate"`

	// Average scores for confirmed vs rejected
	ConfirmedAvgScore float64 `json:"confirmed_avg_score"`
	RejectedAvgScore  float64 `json:"rejected_avg_score"`
	Separation        float64 `json:"separation"`

	// Component averages
	MedicationDiff float64 `json:"medication_diff"`
	DosageDiff     float64 `json:"dosage_diff"`
	QuantityDiff   float64 `json:"quantity_diff"`
	PriceDiff      float64 `json:"price_diff"`
	RecencyDiff    float64 `json:"recency_diff"`
}

// GetFeedbackStats returns feedback statistics for learning
// GET /api/admin/learning/feedback-stats
func (h *LearningHandlers) GetFeedbackStats(w http.ResponseWriter, r *http.Request) {
	if h.feedbackRepo == nil {
		http.Error(w, "Feedback repository not configured", http.StatusServiceUnavailable)
		return
	}

	ctx := r.Context()

	// Last 30 days by default
	endDate := time.Now()
	startDate := endDate.Add(-30 * 24 * time.Hour)

	stats, err := h.feedbackRepo.GetFeedbackStats(ctx, startDate, endDate)
	if err != nil {
		http.Error(w, "Failed to fetch stats: "+err.Error(), http.StatusInternalServerError)
		return
	}

	response := FeedbackStatsResponse{
		Period:           "30 days",
		TotalFeedbacks:   stats.TotalFeedbacks,
		ConfirmedCount:   stats.ConfirmedCount,
		RejectedCount:    stats.RejectedCount,
		ConfirmationRate: stats.ConfirmationRate,

		ConfirmedAvgScore: stats.ConfirmedAvgTotal,
		RejectedAvgScore:  stats.RejectedAvgTotal,
		Separation:        stats.ConfirmedAvgTotal - stats.RejectedAvgTotal,

		MedicationDiff: stats.MedicationDiff,
		DosageDiff:     stats.DosageDiff,
		QuantityDiff:   stats.QuantityDiff,
		PriceDiff:      stats.PriceDiff,
		RecencyDiff:    stats.RecencyDiff,
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// CurrentWeightsResponse for current weights
type CurrentWeightsResponse struct {
	Weights   ai.ScoringWeights `json:"weights"`
	Source    string            `json:"source"`
	AppliedAt *time.Time        `json:"applied_at,omitempty"`
	Notes     string            `json:"notes,omitempty"`
}

// GetCurrentWeights returns current scoring weights
// GET /api/admin/learning/weights
func (h *LearningHandlers) GetCurrentWeights(w http.ResponseWriter, r *http.Request) {
	if h.weightHistoryRepo == nil {
		http.Error(w, "Weight history not configured", http.StatusServiceUnavailable)
		return
	}

	ctx := r.Context()

	current, err := h.weightHistoryRepo.GetCurrent(ctx)
	if err != nil || current == nil {
		// Return default weights if no history
		defaultWeights := ai.DefaultWeights()
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(CurrentWeightsResponse{
			Weights: defaultWeights,
			Source:  "default",
		})
		return
	}

	response := CurrentWeightsResponse{
		Weights: ai.ScoringWeights{
			Medication: current.MedicationWeight,
			Dosage:     current.DosageWeight,
			Quantity:   current.QuantityWeight,
			Price:      current.PriceWeight,
			Recency:    current.RecencyWeight,
		},
		Source:    string(current.Source),
		AppliedAt: &current.AppliedAt,
		Notes:     current.Notes,
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// ManualWeightsRequest for manual weight updates
type ManualWeightsRequest struct {
	Weights ai.ScoringWeights `json:"weights"`
	Notes   string            `json:"notes"`
}

// UpdateWeightsManually allows admin to set weights manually
// PUT /api/admin/learning/weights
func (h *LearningHandlers) UpdateWeightsManually(w http.ResponseWriter, r *http.Request) {
	if h.scheduler == nil {
		http.Error(w, "Learning scheduler not configured", http.StatusServiceUnavailable)
		return
	}

	var req ManualWeightsRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Invalid request body", http.StatusBadRequest)
		return
	}

	// Validate weights sum to 1.0 (with small tolerance)
	sum := req.Weights.Medication + req.Weights.Dosage + req.Weights.Quantity +
		req.Weights.Price + req.Weights.Recency
	if sum < 0.99 || sum > 1.01 {
		http.Error(w, "Weights must sum to 1.0", http.StatusBadRequest)
		return
	}

	// Validate individual weights are within bounds
	if req.Weights.Medication < 0.05 || req.Weights.Medication > 0.70 ||
		req.Weights.Dosage < 0.05 || req.Weights.Dosage > 0.70 ||
		req.Weights.Quantity < 0.05 || req.Weights.Quantity > 0.70 ||
		req.Weights.Price < 0.05 || req.Weights.Price > 0.70 ||
		req.Weights.Recency < 0.05 || req.Weights.Recency > 0.70 {
		http.Error(w, "Each weight must be between 0.05 and 0.70", http.StatusBadRequest)
		return
	}

	ctx := r.Context()

	// Apply weights via scheduler (persists to DB and updates Scorer)
	err := h.scheduler.ApplyWeightsManual(ctx, req.Weights, req.Notes)
	if err != nil {
		http.Error(w, "Failed to apply weights: "+err.Error(), http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":  "success",
		"message": "Weights updated and persisted",
		"weights": req.Weights,
	})
}
