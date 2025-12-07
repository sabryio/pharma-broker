package storage

import (
	"context"
	"time"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// FeedbackAnalysis holds aggregated feedback statistics
type FeedbackAnalysis struct {
	TotalFeedback    int            `json:"total_feedback"`
	PositiveFeedback int            `json:"positive_feedback"`
	NegativeFeedback int            `json:"negative_feedback"`
	AccuracyRate     float64        `json:"accuracy_rate"`
	TopIssues        []string       `json:"top_issues"`
	TrendByDay       map[string]int `json:"trend_by_day"`
	FeedbackByType   map[string]int `json:"feedback_by_type"`
}

// GormFeedbackRepo implements match feedback storage using GORM
type GormFeedbackRepo struct {
	db *GormDB
}

// NewGormFeedbackRepo creates a new GORM-based feedback repository
func NewGormFeedbackRepo(db *GormDB) *GormFeedbackRepo {
	return &GormFeedbackRepo{db: db}
}

// Save stores operator feedback on a match
func (r *GormFeedbackRepo) Save(ctx context.Context, feedback *domain.MatchFeedback) error {
	model := ToMatchFeedbackModel(feedback)
	return r.db.DB.WithContext(ctx).Create(model).Error
}

// GetByMatchID retrieves feedback for a specific match
func (r *GormFeedbackRepo) GetByMatchID(ctx context.Context, matchID string) ([]*domain.MatchFeedback, error) {
	var feedbacks []models.MatchFeedback
	err := r.db.DB.WithContext(ctx).
		Where("match_id = ?", matchID).
		Order("created_at DESC").
		Find(&feedbacks).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.MatchFeedback, len(feedbacks))
	for i := range feedbacks {
		result[i] = ToMatchFeedbackDomain(&feedbacks[i])
	}
	return result, nil
}

// GetRecent retrieves recent feedback for learning loop
func (r *GormFeedbackRepo) GetRecent(ctx context.Context, limit int) ([]*domain.MatchFeedback, error) {
	var feedbacks []models.MatchFeedback
	err := r.db.DB.WithContext(ctx).
		Order("created_at DESC").
		Limit(limit).
		Find(&feedbacks).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.MatchFeedback, len(feedbacks))
	for i := range feedbacks {
		result[i] = ToMatchFeedbackDomain(&feedbacks[i])
	}
	return result, nil
}

// AnalyzeFeedback generates aggregated statistics from feedback data
func (r *GormFeedbackRepo) AnalyzeFeedback(ctx context.Context, days int) (*FeedbackAnalysis, error) {
	cutoff := time.Now().AddDate(0, 0, -days)

	analysis := &FeedbackAnalysis{
		TrendByDay:     make(map[string]int),
		FeedbackByType: make(map[string]int),
	}

	// Count total and by type
	var feedbacks []models.MatchFeedback
	err := r.db.DB.WithContext(ctx).
		Where("created_at >= ?", cutoff).
		Find(&feedbacks).Error
	if err != nil {
		return nil, err
	}

	for _, fb := range feedbacks {
		analysis.TotalFeedback++
		// Decision is 'CONFIRMED' or 'REJECTED'
		if fb.Decision == "CONFIRMED" {
			analysis.PositiveFeedback++
		} else if fb.Decision == "REJECTED" {
			analysis.NegativeFeedback++
		}

		// Track by decision type
		if fb.Decision != "" {
			analysis.FeedbackByType[fb.Decision]++
		}

		// Track by day
		day := fb.CreatedAt.Format("2006-01-02")
		analysis.TrendByDay[day]++
	}

	// Calculate accuracy rate
	if analysis.TotalFeedback > 0 {
		analysis.AccuracyRate = float64(analysis.PositiveFeedback) / float64(analysis.TotalFeedback) * 100
	}

	return analysis, nil
}

// GetFeedbackByMatch is an alias for GetByMatchID (interface compatibility)
func (r *GormFeedbackRepo) GetFeedbackByMatch(ctx context.Context, matchID string) ([]*domain.MatchFeedback, error) {
	return r.GetByMatchID(ctx, matchID)
}

// GetRecentFeedback is an alias for GetRecent (interface compatibility)
func (r *GormFeedbackRepo) GetRecentFeedback(ctx context.Context, limit int) ([]*domain.MatchFeedback, error) {
	return r.GetRecent(ctx, limit)
}

// RecordFeedback is an alias for Save (interface compatibility)
func (r *GormFeedbackRepo) RecordFeedback(ctx context.Context, fb *domain.MatchFeedback) error {
	return r.Save(ctx, fb)
}
