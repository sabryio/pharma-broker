package domain

import "time"

// UnmappedMedication represents a medication that couldn't be mapped
// during AI parsing. These are queued for human review to improve
// the medication database over time.
type UnmappedMedication struct {
	ID            uint       `gorm:"primaryKey" json:"id"`
	RawText       string     `gorm:"index;not null" json:"raw_text"`      // Original Arabic text
	AIOutput      string     `json:"ai_output"`                           // What the AI generated
	SourceMessage string     `gorm:"type:text" json:"source_message"`     // Full message content
	SourceGroup   string     `gorm:"index" json:"source_group"`           // WhatsApp group name
	MessageID     string     `gorm:"index" json:"message_id"`             // Reference to raw message
	Count         int        `gorm:"default:1" json:"count"`              // Times this term appeared
	Reviewed      bool       `gorm:"default:false;index" json:"reviewed"` // Has been reviewed
	ApprovedName  string     `json:"approved_name,omitempty"`             // Human-approved English name
	ReviewedAt    *time.Time `json:"reviewed_at,omitempty"`
	ReviewedBy    string     `json:"reviewed_by,omitempty"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
}

// UnmappedMedicationRepo defines the interface for unmapped medication storage
type UnmappedMedicationRepo interface {
	// Save creates or updates an unmapped medication record
	// If the same RawText already exists, it increments the count
	Save(rawText, aiOutput, sourceMessage, sourceGroup, messageID string) error

	// GetPending returns unmapped medications that haven't been reviewed
	GetPending(limit, offset int) ([]*UnmappedMedication, error)

	// GetByRawText finds an unmapped medication by raw text
	GetByRawText(rawText string) (*UnmappedMedication, error)

	// MarkReviewed marks a medication as reviewed with the approved English name
	MarkReviewed(id uint, approvedName, reviewedBy string) error

	// Count returns the total number of unmapped medications
	Count() (int64, error)

	// CountPending returns number of pending reviews
	CountPending() (int64, error)
}
