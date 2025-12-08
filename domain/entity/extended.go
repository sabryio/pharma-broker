package entity

import "time"

// ParsedItem represents a single item extracted by AI from a message
type ParsedItem struct {
	Type            MessageType `json:"type"`
	Medication      string      `json:"medication"`
	MedicationRaw   string      `json:"medication_raw"`
	MatchConfidence string      `json:"match_confidence,omitempty"`
	AIConfidence    float64     `json:"ai_confidence,omitempty"`
	Quantity        float64     `json:"quantity,omitempty"`
	Unit            *string     `json:"unit,omitempty"`
	Price           float64     `json:"price,omitempty"`
	MaxPrice        float64     `json:"max_price,omitempty"`
	Urgent          bool        `json:"urgent,omitempty"`
	Notes           string      `json:"notes,omitempty"`
}

// AIParseResult represents the AI response for a message
type AIParseResult struct {
	Items   []ParsedItem `json:"items"`
	Error   string       `json:"error,omitempty"`
	RawJSON string       `json:"-"`
}

// FailedMessage represents a message that failed AI processing
type FailedMessage struct {
	ID            string     `json:"id"`
	RawMessageID  string     `json:"raw_message_id"`
	FailureReason string     `json:"failure_reason"`
	RetryCount    int        `json:"retry_count"`
	FailedAt      time.Time  `json:"failed_at"`
	ResolvedAt    *time.Time `json:"resolved_at,omitempty"`
}

// MatchFeedback represents operator feedback on a match
type MatchFeedback struct {
	ID                 string           `json:"id"`
	MatchID            string           `json:"match_id"`
	OperatorID         string           `json:"operator_id,omitempty"`
	Decision           FeedbackDecision `json:"decision"`
	Reason             string           `json:"reason,omitempty"`
	OriginalScore      float64          `json:"original_score"`
	OriginalConfidence string           `json:"original_confidence"`
	CreatedAt          time.Time        `json:"created_at"`
}

// DemandStats represents medication demand statistics
type DemandStats struct {
	Medication   string  `json:"medication"`
	RequestCount int     `json:"request_count"`
	OfferCount   int     `json:"offer_count"`
	DemandRatio  float64 `json:"demand_ratio"`
	Trend        string  `json:"trend"`
}

// MedicationMapping represents Arabic to English medication mapping
type MedicationMapping struct {
	ID          string    `json:"id"`
	ArabicName  string    `json:"arabic_name"`
	EnglishName string    `json:"english_name"`
	Synonyms    []string  `json:"synonyms,omitempty"`
	CreatedAt   time.Time `json:"created_at"`
	UpdatedAt   time.Time `json:"updated_at"`
}

// MatchQueueItem represents a job in the persistent queue
type MatchQueueItem struct {
	ID         string
	SourceType string // "OFFER" or "REQUEST"
	SourceID   string
	CreatedAt  time.Time
}

// ReviewQueueItem represents a message that needs manual review
type ReviewQueueItem struct {
	ID             string       `json:"id"`
	RawMessageID   string       `json:"raw_message_id"`
	GroupName      string       `json:"group_name"`
	SenderName     string       `json:"sender_name"`
	Content        string       `json:"content"`
	ReplyContext   string       `json:"reply_context,omitempty"`
	PartialItems   []ParsedItem `json:"partial_items,omitempty"`
	ParsePass      int          `json:"parse_pass"`
	AvgConfidence  float64      `json:"avg_confidence"`
	FailureReason  string       `json:"failure_reason,omitempty"`
	Status         ReviewStatus `json:"status"`
	ReviewedBy     string       `json:"reviewed_by,omitempty"`
	ReviewedAt     *time.Time   `json:"reviewed_at,omitempty"`
	ReviewNote     string       `json:"review_note,omitempty"`
	CorrectedItems []ParsedItem `json:"corrected_items,omitempty"`
	CreatedAt      time.Time    `json:"created_at"`
	UpdatedAt      time.Time    `json:"updated_at"`
}
