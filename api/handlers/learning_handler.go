package handlers

import (
	"context"
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/ai"
	"pharmabroker/domain/entity"
	"pharmabroker/matching"
)

// LearningFeedbackRepository interface for learning feedback
type LearningFeedbackRepository interface {
	GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*entity.FeedbackStats, error)
	CountByAction(ctx context.Context, action entity.FeedbackAction, startDate, endDate time.Time) (int64, error)
}

// LearningWeightHistoryRepository interface for weight history
type LearningWeightHistoryRepository interface {
	GetCurrent(ctx context.Context) (*entity.WeightHistory, error)
	GetHistory(ctx context.Context, limit int) ([]*entity.WeightHistory, error)
	GetBySource(ctx context.Context, source entity.WeightSource, limit int) ([]*entity.WeightHistory, error)
}

// LearningHandler handles adaptive learning admin endpoints
type LearningHandler struct {
	scheduler         ai.LearningScheduler
	feedbackRepo      LearningFeedbackRepository
	weightHistoryRepo LearningWeightHistoryRepository
	log               zerolog.Logger
}

// NewLearningHandler creates learning admin handlers
func NewLearningHandler(
	scheduler ai.LearningScheduler,
	feedbackRepo LearningFeedbackRepository,
	weightHistoryRepo LearningWeightHistoryRepository,
	log zerolog.Logger,
) *LearningHandler {
	return &LearningHandler{
		scheduler:         scheduler,
		feedbackRepo:      feedbackRepo,
		weightHistoryRepo: weightHistoryRepo,
		log:               log.With().Str("component", "LearningHandler").Logger(),
	}
}

// LearningStatusResponse represents the learning system status
type LearningStatusResponse struct {
	Enabled        bool                       `json:"enabled"`
	Schedule       string                     `json:"schedule"`
	LastRun        *time.Time                 `json:"last_run,omitempty"`
	LastStatus     string                     `json:"last_status"`
	LastError      string                     `json:"last_error,omitempty"`
	LastMetrics    *entity.PerformanceMetrics `json:"last_metrics,omitempty"`
	PendingApply   *matching.Weights          `json:"pending_weights,omitempty"`
	PendingReason  string                     `json:"pending_reason,omitempty"`
	CurrentWeights *matching.Weights          `json:"current_weights,omitempty"`
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

// ApplyPendingRequest for applying pending weights
type ApplyPendingRequest struct {
	Confirm bool `json:"confirm"` // Must be true to apply
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
	Metrics          *entity.PerformanceMetrics `json:"metrics,omitempty"`
	Notes            string                     `json:"notes,omitempty"`
}

// FeedbackStatsResponse for feedback statistics
type FeedbackStatsResponse struct {
	Period           string  `json:"period"`
	TotalFeedbacks   int     `json:"total"`
	ConfirmedCount   int     `json:"confirmed"`
	RejectedCount    int     `json:"rejected"`
	ConfirmationRate float64 `json:"confirmation_rate"`

	ConfirmedAvgScore float64 `json:"confirmed_avg_score"`
	RejectedAvgScore  float64 `json:"rejected_avg_score"`
	Separation        float64 `json:"separation"`

	MedicationDiff float64 `json:"medication_diff"`
	DosageDiff     float64 `json:"dosage_diff"`
	QuantityDiff   float64 `json:"quantity_diff"`
	PriceDiff      float64 `json:"price_diff"`
	RecencyDiff    float64 `json:"recency_diff"`
}

// CurrentWeightsResponse for current weights
type CurrentWeightsResponse struct {
	Weights   matching.Weights `json:"weights"`
	Source    string           `json:"source"`
	AppliedAt *time.Time       `json:"applied_at,omitempty"`
	Notes     string           `json:"notes,omitempty"`
}

// ManualWeightsRequest for manual weight updates
type ManualWeightsRequest struct {
	Weights matching.Weights `json:"weights"`
	Notes   string           `json:"notes"`
}

// GetLearningStatusGin returns current learning system status
func (h *LearningHandler) GetLearningStatusGin(c *gin.Context) {
	if h.scheduler == nil {
		ErrorGin(c, http.StatusServiceUnavailable, ErrInternal("Learning scheduler not configured"))
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

	SuccessGin(c, response)
}

// TriggerLearningGin manually triggers a learning job
func (h *LearningHandler) TriggerLearningGin(c *gin.Context) {
	if h.scheduler == nil {
		ErrorGin(c, http.StatusServiceUnavailable, ErrInternal("Learning scheduler not configured"))
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

	SuccessGin(c, response)
}

// ApplyPendingWeightsGin applies pending weights manually
func (h *LearningHandler) ApplyPendingWeightsGin(c *gin.Context) {
	if h.scheduler == nil {
		ErrorGin(c, http.StatusServiceUnavailable, ErrInternal("Learning scheduler not configured"))
		return
	}

	var req ApplyPendingRequest
	if !BindJSONGin(c, &req) {
		return
	}

	if !req.Confirm {
		BadRequestGin(c, "Must set confirm=true to apply weights")
		return
	}

	ctx := c.Request.Context()
	err := h.scheduler.ApplyPending(ctx)
	if err != nil {
		InternalErrorGin(c, err.Error())
		return
	}

	SuccessGin(c, map[string]string{
		"status":  "success",
		"message": "Pending weights applied successfully",
	})
}

// RejectPendingWeightsGin rejects pending weights
func (h *LearningHandler) RejectPendingWeightsGin(c *gin.Context) {
	if h.scheduler == nil {
		ErrorGin(c, http.StatusServiceUnavailable, ErrInternal("Learning scheduler not configured"))
		return
	}

	h.scheduler.RejectPending()

	SuccessGin(c, map[string]string{
		"status":  "success",
		"message": "Pending weights rejected",
	})
}

// RollbackWeightsGin reverts to previous weights
func (h *LearningHandler) RollbackWeightsGin(c *gin.Context) {
	if h.scheduler == nil {
		ErrorGin(c, http.StatusServiceUnavailable, ErrInternal("Learning scheduler not configured"))
		return
	}

	ctx := c.Request.Context()
	currentWeights := h.scheduler.Status().PendingApply

	err := h.scheduler.Rollback(ctx)
	if err != nil {
		InternalErrorGin(c, "Rollback failed: "+err.Error())
		return
	}

	SuccessGin(c, map[string]interface{}{
		"status":           "success",
		"message":          "Weights rolled back to previous configuration",
		"rolled_back_from": currentWeights,
	})
}

// GetWeightHistoryGin returns historical weight changes
func (h *LearningHandler) GetWeightHistoryGin(c *gin.Context) {
	if h.weightHistoryRepo == nil {
		InternalErrorGin(c, "Weight history not configured")
		return
	}

	ctx := c.Request.Context()
	limit := GetQueryInt(c, "limit", 20)

	history, err := h.weightHistoryRepo.GetHistory(ctx, limit)
	if err != nil {
		DatabaseErrorGin(c, "Failed to fetch history: "+err.Error())
		return
	}

	items := make([]*WeightHistoryItem, 0, len(history))
	for _, wh := range history {
		items = append(items, &WeightHistoryItem{
			ID:               wh.ID,
			Timestamp:        wh.AppliedAt,
			Source:           string(wh.Source),
			MedicationWeight: wh.MedicationWeight,
			DosageWeight:     wh.DosageWeight,
			QuantityWeight:   wh.QuantityWeight,
			PriceWeight:      wh.PriceWeight,
			RecencyWeight:    wh.RecencyWeight,
			Notes:            wh.Notes,
		})
	}

	SuccessGin(c, WeightHistoryResponse{
		History: items,
		Total:   len(items),
	})
}

// GetFeedbackStatsGin returns feedback statistics for learning
func (h *LearningHandler) GetFeedbackStatsGin(c *gin.Context) {
	if h.feedbackRepo == nil {
		InternalErrorGin(c, "Feedback repository not configured")
		return
	}

	ctx := c.Request.Context()
	days := GetQueryInt(c, "days", 30)

	endDate := time.Now()
	startDate := endDate.AddDate(0, 0, -days)

	stats, err := h.feedbackRepo.GetFeedbackStats(ctx, startDate, endDate)
	if err != nil {
		DatabaseErrorGin(c, "Failed to fetch stats: "+err.Error())
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

	SuccessGin(c, response)
}

// GetCurrentWeightsGin returns current scoring weights
func (h *LearningHandler) GetCurrentWeightsGin(c *gin.Context) {
	if h.weightHistoryRepo == nil {
		InternalErrorGin(c, "Weight history not configured")
		return
	}

	ctx := c.Request.Context()

	current, err := h.weightHistoryRepo.GetCurrent(ctx)
	if err != nil || current == nil {
		defaultWeights := matching.DefaultWeights()
		SuccessGin(c, CurrentWeightsResponse{
			Weights: defaultWeights,
			Source:  "default",
		})
		return
	}

	response := CurrentWeightsResponse{
		Weights: matching.Weights{
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

	SuccessGin(c, response)
}

// UpdateWeightsManuallyGin allows admin to set weights manually
func (h *LearningHandler) UpdateWeightsManuallyGin(c *gin.Context) {
	if h.scheduler == nil {
		ErrorGin(c, http.StatusServiceUnavailable, ErrInternal("Learning scheduler not configured"))
		return
	}

	var req ManualWeightsRequest
	if !BindJSONGin(c, &req) {
		return
	}

	// Validate weights sum to 1.0
	sum := req.Weights.Medication + req.Weights.Dosage + req.Weights.Quantity +
		req.Weights.Price + req.Weights.Recency
	if sum < 0.99 || sum > 1.01 {
		BadRequestGin(c, "Weights must sum to 1.0")
		return
	}

	// Validate individual weights
	if req.Weights.Medication < 0.05 || req.Weights.Medication > 0.70 ||
		req.Weights.Dosage < 0.05 || req.Weights.Dosage > 0.70 ||
		req.Weights.Quantity < 0.05 || req.Weights.Quantity > 0.70 ||
		req.Weights.Price < 0.05 || req.Weights.Price > 0.70 ||
		req.Weights.Recency < 0.05 || req.Weights.Recency > 0.70 {
		BadRequestGin(c, "Each weight must be between 0.05 and 0.70")
		return
	}

	ctx := c.Request.Context()

	err := h.scheduler.ApplyWeightsManual(ctx, req.Weights, req.Notes)
	if err != nil {
		InternalErrorGin(c, "Failed to apply weights: "+err.Error())
		return
	}

	SuccessGin(c, map[string]interface{}{
		"status":  "success",
		"message": "Weights updated and persisted",
		"weights": req.Weights,
	})
}
