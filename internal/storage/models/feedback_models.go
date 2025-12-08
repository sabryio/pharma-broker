package models

import "time"

// FeedbackRecord stores user feedback on matches for learning
type FeedbackRecord struct {
	ID        string `gorm:"primaryKey;type:varchar(36)"`
	MatchID   string `gorm:"type:varchar(36);index"`
	OfferID   string `gorm:"type:varchar(36);not null;index"`
	RequestID string `gorm:"type:varchar(36);not null;index"`
	Action    string `gorm:"type:varchar(20);not null;index"`

	// Score components at time of match
	MedicationScore float64 `gorm:"type:real"`
	DosageScore     float64 `gorm:"type:real"`
	QuantityScore   float64 `gorm:"type:real"`
	PriceScore      float64 `gorm:"type:real"`
	RecencyScore    float64 `gorm:"type:real"`
	TotalScore      float64 `gorm:"type:real;index"`

	// Metadata
	FeedbackAt time.Time `gorm:"not null;index"`
	UserID     string    `gorm:"type:varchar(100)"`
	CreatedAt  time.Time `gorm:"autoCreateTime"`
}

// TableName specifies the table name
func (FeedbackRecord) TableName() string {
	return "match_feedback_records"
}

// WeightHistory tracks changes to scoring weights
type WeightHistory struct {
	ID               string  `gorm:"primaryKey;type:varchar(36)"`
	MedicationWeight float64 `gorm:"type:real;not null"`
	DosageWeight     float64 `gorm:"type:real;not null"`
	QuantityWeight   float64 `gorm:"type:real;not null"`
	PriceWeight      float64 `gorm:"type:real;not null"`
	RecencyWeight    float64 `gorm:"type:real;not null"`
	Source           string  `gorm:"type:varchar(20);not null"`

	PerformanceMetrics string    `gorm:"type:text"`
	AppliedAt          time.Time `gorm:"not null;index"`
	CreatedAt          time.Time `gorm:"autoCreateTime"`
	Notes              string    `gorm:"type:text"`
}

// TableName specifies the table name
func (WeightHistory) TableName() string {
	return "weight_history"
}
