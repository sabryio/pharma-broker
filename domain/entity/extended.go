package entity

import "time"

// ParsedItem represents a single item extracted by AI from a message
type ParsedItem struct {
	Type            MessageType `json:"type" jsonschema:"enum=OFFER,enum=REQUEST,description=Type of the listing (OFFER or REQUEST)"`
	Medication      string      `json:"medication" jsonschema_description:"Normalized medication name (English preferred)"`
	MedicationRaw   string      `json:"medication_raw" jsonschema_description:"Exact text from message referring to medication"`
	MatchConfidence string      `json:"match_confidence,omitempty" jsonschema_description:"How the medication was matched: EXACT, FUZZY, VECTOR, or TRANSLITERATED"`
	AIConfidence    float64     `json:"ai_confidence,omitempty" jsonschema:"minimum=0,maximum=1,description=AI certainty about extraction quality (0.0-1.0)"`
	Quantity        float64     `json:"quantity,omitempty" jsonschema_description:"Numeric quantity (supports decimals like 0.5)"`
	Unit            *string     `json:"unit,omitempty" jsonschema_description:"Unit of measure (e.g. boxes, strips, ampoules)"`
	Price           float64     `json:"price,omitempty" jsonschema_description:"Price per unit if specified"`
	MaxPrice        float64     `json:"max_price,omitempty" jsonschema_description:"Maximum price willing to pay (for requests)"`
	Urgent          bool        `json:"urgent,omitempty" jsonschema_description:"If the user explicitly mentions urgent/emergency"`
	Notes           string      `json:"notes,omitempty" jsonschema_description:"Any other details (expiry, batch, currency, location, unknown text)"`
}

// AIParseResult represents the AI response for a message
type AIParseResult struct {
	Items   []ParsedItem `json:"items" jsonschema_description:"List of pharmaceutical items found in the message"`
	Error   string       `json:"error,omitempty" jsonschema_description:"Error message if parsing failed, otherwise empty"`
	RawJSON string       `json:"-"` // Raw JSON for debugging
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
	Embedding   []float32 `json:"embedding,omitempty"` // Vector embedding for similarity search
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

// UnmappedMedication represents a medication that couldn't be mapped
// during AI parsing, queued for human review.
type UnmappedMedication struct {
	ID            uint       `json:"id"`
	RawText       string     `json:"raw_text"`
	AIOutput      string     `json:"ai_output"`
	SourceMessage string     `json:"source_message"`
	SourceGroup   string     `json:"source_group"`
	MessageID     string     `json:"message_id"`
	Count         int        `json:"count"`
	Reviewed      bool       `json:"reviewed"`
	ApprovedName  string     `json:"approved_name,omitempty"`
	ReviewedAt    *time.Time `json:"reviewed_at,omitempty"`
	ReviewedBy    string     `json:"reviewed_by,omitempty"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
}
