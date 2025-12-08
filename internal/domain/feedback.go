package domain

import "time"

// FeedbackAction represents the type of user feedback on a match
type FeedbackAction string

const (
	MatchFeedbackConfirmed FeedbackAction = "CONFIRMED"
	MatchFeedbackRejected  FeedbackAction = "REJECTED"
)

// WeightSource indicates how weights were determined
type WeightSource string

const (
	WeightSourceDefault     WeightSource = "DEFAULT"
	WeightSourceManual      WeightSource = "MANUAL"
	WeightSourceAutoLearned WeightSource = "AUTO_LEARNED"
)

// MatchFeedback stores user feedback on a specific match
type FeedbackRecord struct {
	ID        string         `gorm:"primaryKey;type:varchar(36)"`
	MatchID   string         `gorm:"type:varchar(36);index"` // Optional: for tracking
	OfferID   string         `gorm:"type:varchar(36);not null;index"`
	RequestID string         `gorm:"type:varchar(36);not null;index"`
	Action    FeedbackAction `gorm:"type:varchar(20);not null;index"`

	// Scores at the time of match (for learning)
	MedicationScore float64 `gorm:"type:real"`
	DosageScore     float64 `gorm:"type:real"`
	QuantityScore   float64 `gorm:"type:real"`
	PriceScore      float64 `gorm:"type:real"`
	RecencyScore    float64 `gorm:"type:real"`
	TotalScore      float64 `gorm:"type:real;index"`

	// Metadata
	FeedbackAt time.Time `gorm:"not null;index"`
	UserID     string    `gorm:"type:varchar(100)"` // Who provided feedback (optional)
	CreatedAt  time.Time `gorm:"autoCreateTime"`
}

// TableName specifies the table name for MatchFeedback
func (FeedbackRecord) TableName() string {
	return "match_feedback"
}

// WeightHistory tracks changes to scoring weights over time
type WeightHistory struct {
	ID               string       `gorm:"primaryKey;type:varchar(36)"`
	MedicationWeight float64      `gorm:"type:real;not null"`
	DosageWeight     float64      `gorm:"type:real;not null"`
	QuantityWeight   float64      `gorm:"type:real;not null"`
	PriceWeight      float64      `gorm:"type:real;not null"`
	RecencyWeight    float64      `gorm:"type:real;not null"`
	Source           WeightSource `gorm:"type:varchar(20);not null"`

	// Performance metrics (JSON)
	PerformanceMetrics string `gorm:"type:text"` // JSON serialized

	// Metadata
	AppliedAt time.Time `gorm:"not null;index"`
	CreatedAt time.Time `gorm:"autoCreateTime"`
	Notes     string    `gorm:"type:text"` // Optional description
}

// TableName specifies the table name for WeightHistory
func (WeightHistory) TableName() string {
	return "weight_history"
}

// FeedbackStats represents aggregated statistics for learning
type FeedbackStats struct {
	TotalFeedbacks   int
	ConfirmedCount   int
	RejectedCount    int
	ConfirmationRate float64

	// Average scores for confirmed matches
	ConfirmedAvgMedication float64
	ConfirmedAvgDosage     float64
	ConfirmedAvgQuantity   float64
	ConfirmedAvgPrice      float64
	ConfirmedAvgRecency    float64
	ConfirmedAvgTotal      float64

	// Average scores for rejected matches
	RejectedAvgMedication float64
	RejectedAvgDosage     float64
	RejectedAvgQuantity   float64
	RejectedAvgPrice      float64
	RejectedAvgRecency    float64
	RejectedAvgTotal      float64

	// Difference (indicator of importance)
	MedicationDiff float64
	DosageDiff     float64
	QuantityDiff   float64
	PriceDiff      float64
	RecencyDiff    float64
}

// PerformanceMetrics holds evaluation metrics for weight configurations
type PerformanceMetrics struct {
	Precision         float64   `json:"precision"`         // Confirmed / (Confirmed + False Positives)
	Recall            float64   `json:"recall"`            // Confirmed / (Confirmed + Missed)
	F1Score           float64   `json:"f1_score"`          // Harmonic mean of precision & recall
	ConfirmationRate  float64   `json:"confirmation_rate"` // % of matches confirmed
	AvgScoreConfirmed float64   `json:"avg_score_confirmed"`
	AvgScoreRejected  float64   `json:"avg_score_rejected"`
	SampleSize        int       `json:"sample_size"`
	EvaluatedAt       time.Time `json:"evaluated_at"`
}
