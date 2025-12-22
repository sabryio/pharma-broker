// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
)

// FeedbackRepo implements match feedback storage
type FeedbackRepo struct {
	db *DB
}

// NewFeedbackRepo creates a new feedback repository
func NewFeedbackRepo(db *DB) *FeedbackRepo {
	return &FeedbackRepo{db: db}
}

// Save stores operator feedback on a match
func (r *FeedbackRepo) Save(ctx context.Context, feedback *entity.MatchFeedback) error {
	model := ToMatchFeedbackModel(feedback)
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// GetByMatchID retrieves feedback for a specific match
func (r *FeedbackRepo) GetByMatchID(ctx context.Context, matchID string) ([]*entity.MatchFeedback, error) {
	var feedbacks []MatchFeedback
	err := r.db.Conn.WithContext(ctx).
		Where("match_id = ?", matchID).
		Order("created_at DESC").
		Find(&feedbacks).Error
	if err != nil {
		return nil, err
	}

	result := make([]*entity.MatchFeedback, len(feedbacks))
	for i := range feedbacks {
		result[i] = ToMatchFeedbackEntity(&feedbacks[i])
	}
	return result, nil
}

// GetRecent retrieves recent feedback for learning loop
func (r *FeedbackRepo) GetRecent(ctx context.Context, limit int) ([]*entity.MatchFeedback, error) {
	var feedbacks []MatchFeedback
	err := r.db.Conn.WithContext(ctx).
		Order("created_at DESC").
		Limit(limit).
		Find(&feedbacks).Error
	if err != nil {
		return nil, err
	}

	result := make([]*entity.MatchFeedback, len(feedbacks))
	for i := range feedbacks {
		result[i] = ToMatchFeedbackEntity(&feedbacks[i])
	}
	return result, nil
}

// AnalyzeFeedback generates aggregated statistics from feedback data
func (r *FeedbackRepo) AnalyzeFeedback(ctx context.Context, days int) (*entity.FeedbackAnalysis, error) {
	cutoff := time.Now().AddDate(0, 0, -days)

	analysis := &entity.FeedbackAnalysis{
		TrendByDay:     make(map[string]int),
		FeedbackByType: make(map[string]int),
	}

	var feedbacks []MatchFeedback
	err := r.db.Conn.WithContext(ctx).
		Where("created_at >= ?", cutoff).
		Find(&feedbacks).Error
	if err != nil {
		return nil, err
	}

	for _, fb := range feedbacks {
		analysis.TotalFeedback++
		switch fb.Decision {
		case "CONFIRMED":
			analysis.PositiveFeedback++
		case "REJECTED":
			analysis.NegativeFeedback++
		}

		if fb.Decision != "" {
			analysis.FeedbackByType[fb.Decision]++
		}

		day := fb.CreatedAt.Format("2006-01-02")
		analysis.TrendByDay[day]++
	}

	if analysis.TotalFeedback > 0 {
		analysis.AccuracyRate = float64(analysis.PositiveFeedback) / float64(analysis.TotalFeedback) * 100
	}

	return analysis, nil
}

// GetFeedbackByMatch is an alias for GetByMatchID
func (r *FeedbackRepo) GetFeedbackByMatch(ctx context.Context, matchID string) ([]*entity.MatchFeedback, error) {
	return r.GetByMatchID(ctx, matchID)
}

// GetRecentFeedback is an alias for GetRecent
func (r *FeedbackRepo) GetRecentFeedback(ctx context.Context, limit int) ([]*entity.MatchFeedback, error) {
	return r.GetRecent(ctx, limit)
}

// RecordFeedback is an alias for Save
func (r *FeedbackRepo) RecordFeedback(ctx context.Context, fb *entity.MatchFeedback) error {
	return r.Save(ctx, fb)
}
