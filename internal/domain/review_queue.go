package domain

import "time"

// ReviewStatus represents the status of a review queue item
type ReviewStatus string

const (
	ReviewStatusPending  ReviewStatus = "PENDING"
	ReviewStatusApproved ReviewStatus = "APPROVED"
	ReviewStatusRejected ReviewStatus = "REJECTED"
)

// ReviewQueueItem represents a message that needs manual review
type ReviewQueueItem struct {
	ID           string `json:"id"`
	RawMessageID string `json:"raw_message_id"`
	GroupName    string `json:"group_name"`
	SenderName   string `json:"sender_name"`
	Content      string `json:"content"`
	ReplyContext string `json:"reply_context,omitempty"` // Quoted message if reply

	// Partial extraction results from AI
	PartialItems  []ParsedItem `json:"partial_items,omitempty"`
	ParsePass     int          `json:"parse_pass"` // Which pass queued this (usually 3)
	AvgConfidence float64      `json:"avg_confidence"`
	FailureReason string       `json:"failure_reason,omitempty"`

	// Review tracking
	Status     ReviewStatus `json:"status"`
	ReviewedBy string       `json:"reviewed_by,omitempty"`
	ReviewedAt *time.Time   `json:"reviewed_at,omitempty"`
	ReviewNote string       `json:"review_note,omitempty"`

	// Corrected items after review (if approved with changes)
	CorrectedItems []ParsedItem `json:"corrected_items,omitempty"`

	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

// ReviewQueueRepository defines the interface for review queue persistence
type ReviewQueueRepository interface {
	// Save creates or updates a review queue item
	Save(ctx interface{}, item *ReviewQueueItem) error

	// GetByID retrieves a review queue item by ID
	GetByID(ctx interface{}, id string) (*ReviewQueueItem, error)

	// GetPending retrieves pending review items with pagination
	GetPending(ctx interface{}, limit, offset int) ([]*ReviewQueueItem, error)

	// CountPending returns the number of pending reviews
	CountPending(ctx interface{}) (int64, error)

	// Approve marks an item as approved with optional corrections
	Approve(ctx interface{}, id string, reviewedBy string, correctedItems []ParsedItem, note string) error

	// Reject marks an item as rejected
	Reject(ctx interface{}, id string, reviewedBy string, reason string) error

	// GetByRawMessageID finds review items for a specific message
	GetByRawMessageID(ctx interface{}, rawMessageID string) (*ReviewQueueItem, error)
}
