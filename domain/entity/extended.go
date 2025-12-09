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

// ========================================
// Audit Log Types
// ========================================

// AuditAction represents the type of audited action
type AuditAction string

const (
	AuditMatchConfirmed  AuditAction = "MATCH_CONFIRMED"
	AuditMatchRejected   AuditAction = "MATCH_REJECTED"
	AuditConfigChanged   AuditAction = "CONFIG_CHANGED"
	AuditGroupEnabled    AuditAction = "GROUP_ENABLED"
	AuditGroupDisabled   AuditAction = "GROUP_DISABLED"
	AuditReportGenerated AuditAction = "REPORT_GENERATED"
)

// AuditLog represents a system audit log entry
type AuditLog struct {
	ID        string      `json:"id"`
	Action    AuditAction `json:"action"`
	EntityID  string      `json:"entity_id,omitempty"`
	OldValue  string      `json:"old_value,omitempty"`
	NewValue  string      `json:"new_value,omitempty"`
	Details   string      `json:"details,omitempty"`
	IPAddress string      `json:"ip_address,omitempty"`
	CreatedAt time.Time   `json:"created_at"`
}

// ========================================
// Weight History Types
// ========================================

// WeightSource indicates how weights were determined
type WeightSource string

const (
	WeightSourceDefault     WeightSource = "DEFAULT"
	WeightSourceManual      WeightSource = "MANUAL"
	WeightSourceAutoLearned WeightSource = "AUTO_LEARNED"
)

// WeightHistory tracks changes to scoring weights over time
type WeightHistory struct {
	ID                 string       `json:"id"`
	MedicationWeight   float64      `json:"medication_weight"`
	DosageWeight       float64      `json:"dosage_weight"`
	QuantityWeight     float64      `json:"quantity_weight"`
	PriceWeight        float64      `json:"price_weight"`
	RecencyWeight      float64      `json:"recency_weight"`
	Source             WeightSource `json:"source"`
	PerformanceMetrics string       `json:"performance_metrics,omitempty"` // JSON serialized
	AppliedAt          time.Time    `json:"applied_at"`
	CreatedAt          time.Time    `json:"created_at"`
	Notes              string       `json:"notes,omitempty"`
}

// ========================================
// Feedback Types
// ========================================

// FeedbackAction represents the type of user feedback on a match
type FeedbackAction string

const (
	FeedbackActionConfirmed FeedbackAction = "CONFIRMED"
	FeedbackActionRejected  FeedbackAction = "REJECTED"
)

// FeedbackRecord stores structured user feedback on a specific match
type FeedbackRecord struct {
	ID              string         `json:"id"`
	MatchID         string         `json:"match_id"`
	OfferID         string         `json:"offer_id"`
	RequestID       string         `json:"request_id"`
	Action          FeedbackAction `json:"action"`
	MedicationScore float64        `json:"medication_score"`
	DosageScore     float64        `json:"dosage_score"`
	QuantityScore   float64        `json:"quantity_score"`
	PriceScore      float64        `json:"price_score"`
	RecencyScore    float64        `json:"recency_score"`
	TotalScore      float64        `json:"total_score"`
	FeedbackAt      time.Time      `json:"feedback_at"`
	UserID          string         `json:"user_id,omitempty"`
	CreatedAt       time.Time      `json:"created_at"`
}

// FeedbackStats represents aggregated statistics for learning
type FeedbackStats struct {
	TotalFeedbacks   int     `json:"total_feedbacks"`
	ConfirmedCount   int     `json:"confirmed_count"`
	RejectedCount    int     `json:"rejected_count"`
	ConfirmationRate float64 `json:"confirmation_rate"`

	// Average scores for confirmed matches
	ConfirmedAvgMedication float64 `json:"confirmed_avg_medication"`
	ConfirmedAvgDosage     float64 `json:"confirmed_avg_dosage"`
	ConfirmedAvgQuantity   float64 `json:"confirmed_avg_quantity"`
	ConfirmedAvgPrice      float64 `json:"confirmed_avg_price"`
	ConfirmedAvgRecency    float64 `json:"confirmed_avg_recency"`
	ConfirmedAvgTotal      float64 `json:"confirmed_avg_total"`

	// Average scores for rejected matches
	RejectedAvgMedication float64 `json:"rejected_avg_medication"`
	RejectedAvgDosage     float64 `json:"rejected_avg_dosage"`
	RejectedAvgQuantity   float64 `json:"rejected_avg_quantity"`
	RejectedAvgPrice      float64 `json:"rejected_avg_price"`
	RejectedAvgRecency    float64 `json:"rejected_avg_recency"`
	RejectedAvgTotal      float64 `json:"rejected_avg_total"`

	// Difference (indicator of importance)
	MedicationDiff float64 `json:"medication_diff"`
	DosageDiff     float64 `json:"dosage_diff"`
	QuantityDiff   float64 `json:"quantity_diff"`
	PriceDiff      float64 `json:"price_diff"`
	RecencyDiff    float64 `json:"recency_diff"`
}

// PerformanceMetrics holds evaluation metrics for weight configurations
type PerformanceMetrics struct {
	Precision         float64   `json:"precision"`
	Recall            float64   `json:"recall"`
	F1Score           float64   `json:"f1_score"`
	ConfirmationRate  float64   `json:"confirmation_rate"`
	AvgScoreConfirmed float64   `json:"avg_score_confirmed"`
	AvgScoreRejected  float64   `json:"avg_score_rejected"`
	SampleSize        int       `json:"sample_size"`
	EvaluatedAt       time.Time `json:"evaluated_at"`
}

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

// AppConfig holds aggregated application configuration
type AppConfig struct {
	AutoParseEnabled bool   `json:"auto_parse_enabled"`
	SkipOwnMessages  bool   `json:"skip_own_messages"`
	AdminPhone       string `json:"admin_phone"`
}
