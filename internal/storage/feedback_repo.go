package storage

import (
	"context"
	"time"

	"pharmabroker/internal/domain"

	"github.com/google/uuid"
)

// FeedbackRepo implements storage for match feedback (learning loop)
type FeedbackRepo struct {
	db *DB
}

// NewFeedbackRepo creates a new FeedbackRepo
func NewFeedbackRepo(db *DB) *FeedbackRepo {
	return &FeedbackRepo{db: db}
}

// RecordFeedback stores an operator's decision on a match
func (r *FeedbackRepo) RecordFeedback(ctx context.Context, fb *domain.MatchFeedback) error {
	if fb.ID == "" {
		fb.ID = uuid.New().String()
	}
	if fb.CreatedAt.IsZero() {
		fb.CreatedAt = time.Now()
	}

	query := `
		INSERT INTO match_feedback (id, match_id, operator_id, decision, reason, original_score, original_confidence, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
	`
	_, err := r.db.Conn().ExecContext(ctx, query,
		fb.ID,
		fb.MatchID,
		fb.OperatorID,
		string(fb.Decision),
		fb.Reason,
		fb.OriginalScore,
		fb.OriginalConfidence,
		fb.CreatedAt,
	)
	return err
}

// GetFeedbackByMatch returns all feedback for a specific match
func (r *FeedbackRepo) GetFeedbackByMatch(ctx context.Context, matchID string) ([]*domain.MatchFeedback, error) {
	query := `
		SELECT id, match_id, operator_id, decision, reason, original_score, original_confidence, created_at
		FROM match_feedback
		WHERE match_id = ?
		ORDER BY created_at DESC
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, matchID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []*domain.MatchFeedback
	for rows.Next() {
		fb := &domain.MatchFeedback{}
		var decision string
		if err := rows.Scan(&fb.ID, &fb.MatchID, &fb.OperatorID, &decision, &fb.Reason, &fb.OriginalScore, &fb.OriginalConfidence, &fb.CreatedAt); err != nil {
			return nil, err
		}
		fb.Decision = domain.FeedbackDecision(decision)
		result = append(result, fb)
	}
	return result, rows.Err()
}

// FeedbackAnalysis represents aggregated feedback statistics
type FeedbackAnalysis struct {
	TotalFeedback      int     `json:"total_feedback"`
	ConfirmedCount     int     `json:"confirmed_count"`
	RejectedCount      int     `json:"rejected_count"`
	ConfirmRate        float64 `json:"confirm_rate"` // 0-1
	AvgConfirmedScore  float64 `json:"avg_confirmed_score"`
	AvgRejectedScore   float64 `json:"avg_rejected_score"`
	SuggestedThreshold float64 `json:"suggested_threshold"` // Optimal threshold based on data
}

// AnalyzeFeedback aggregates feedback data for tuning
func (r *FeedbackRepo) AnalyzeFeedback(ctx context.Context, days int) (*FeedbackAnalysis, error) {
	query := `
		SELECT 
			decision,
			COUNT(*) as count,
			AVG(original_score) as avg_score
		FROM match_feedback
		WHERE created_at > datetime('now', '-' || ? || ' days')
		GROUP BY decision
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, days)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	analysis := &FeedbackAnalysis{}
	var confirmedSum, rejectedSum float64

	for rows.Next() {
		var decision string
		var count int
		var avgScore float64
		if err := rows.Scan(&decision, &count, &avgScore); err != nil {
			return nil, err
		}

		switch domain.FeedbackDecision(decision) {
		case domain.FeedbackConfirmed:
			analysis.ConfirmedCount = count
			analysis.AvgConfirmedScore = avgScore
			confirmedSum = avgScore * float64(count)
		case domain.FeedbackRejected:
			analysis.RejectedCount = count
			analysis.AvgRejectedScore = avgScore
			rejectedSum = avgScore * float64(count)
		}
	}

	analysis.TotalFeedback = analysis.ConfirmedCount + analysis.RejectedCount
	if analysis.TotalFeedback > 0 {
		analysis.ConfirmRate = float64(analysis.ConfirmedCount) / float64(analysis.TotalFeedback)
	}

	// Calculate suggested threshold: midpoint between avg confirmed and rejected scores
	if analysis.ConfirmedCount > 0 && analysis.RejectedCount > 0 {
		avgConfirmed := confirmedSum / float64(analysis.ConfirmedCount)
		avgRejected := rejectedSum / float64(analysis.RejectedCount)
		analysis.SuggestedThreshold = (avgConfirmed + avgRejected) / 2
	} else if analysis.ConfirmedCount > 0 {
		// Only confirmed feedback, use a lower threshold
		analysis.SuggestedThreshold = analysis.AvgConfirmedScore * 0.8
	} else {
		// Default threshold
		analysis.SuggestedThreshold = 0.5
	}

	return analysis, rows.Err()
}

// GetRecentFeedback returns recent feedback entries for review
func (r *FeedbackRepo) GetRecentFeedback(ctx context.Context, limit int) ([]*domain.MatchFeedback, error) {
	query := `
		SELECT id, match_id, operator_id, decision, reason, original_score, original_confidence, created_at
		FROM match_feedback
		ORDER BY created_at DESC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []*domain.MatchFeedback
	for rows.Next() {
		fb := &domain.MatchFeedback{}
		var decision string
		if err := rows.Scan(&fb.ID, &fb.MatchID, &fb.OperatorID, &decision, &fb.Reason, &fb.OriginalScore, &fb.OriginalConfidence, &fb.CreatedAt); err != nil {
			return nil, err
		}
		fb.Decision = domain.FeedbackDecision(decision)
		result = append(result, fb)
	}
	return result, rows.Err()
}
