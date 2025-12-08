package models

import (
	"time"
)

// ReviewQueue represents a message that needs manual review in the database
type ReviewQueue struct {
	ID           string  `gorm:"primaryKey;type:varchar(36)"`
	RawMessageID string  `gorm:"type:varchar(36);index;not null"`
	GroupName    string  `gorm:"type:varchar(255)"`
	SenderName   string  `gorm:"type:varchar(255)"`
	Content      string  `gorm:"type:text;not null"`
	ReplyContext *string `gorm:"type:text"` // Quoted message if reply

	// Partial extraction results (JSON)
	PartialItems  string  `gorm:"type:text"` // JSON array of ParsedItem
	ParsePass     int     `gorm:"type:integer;not null;default:3"`
	AvgConfidence float64 `gorm:"type:real"`
	FailureReason *string `gorm:"type:text"`

	// Review tracking
	Status     string     `gorm:"type:varchar(20);not null;default:'PENDING';index"`
	ReviewedBy *string    `gorm:"type:varchar(255)"`
	ReviewedAt *time.Time `gorm:"type:datetime"`
	ReviewNote *string    `gorm:"type:text"`

	// Corrected items (JSON)
	CorrectedItems *string `gorm:"type:text"` // JSON array

	CreatedAt time.Time `gorm:"autoCreateTime"`
	UpdatedAt time.Time `gorm:"autoUpdateTime"`
}

// TableName returns the table name for GORM
func (ReviewQueue) TableName() string {
	return "review_queue"
}
