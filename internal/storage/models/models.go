package models

import (
	"time"
)

// RawMessage represents an incoming WhatsApp message before AI processing
type RawMessage struct {
	ID          string     `gorm:"column:id;primaryKey"`
	ExternalID  *string    `gorm:"column:external_id;uniqueIndex"`
	GroupJID    string     `gorm:"column:group_jid;not null"`
	GroupName   string     `gorm:"column:group_name;not null"`
	SenderJID   string     `gorm:"column:sender_jid;not null"`
	SenderPhone string     `gorm:"column:sender_phone;not null"`
	SenderName  *string    `gorm:"column:sender_name"`
	Content     string     `gorm:"column:content;not null"`
	Timestamp   time.Time  `gorm:"column:timestamp;not null"`
	ProcessedAt *time.Time `gorm:"column:processed_at"`
	Error       *string    `gorm:"column:error"`
	CreatedAt   time.Time  `gorm:"column:created_at;autoCreateTime"`

	// Relationships
	Offers   []Offer   `gorm:"foreignKey:RawMessageID"`
	Requests []Request `gorm:"foreignKey:RawMessageID"`
}

func (RawMessage) TableName() string {
	return "raw_messages"
}

// Offer represents a medication supply offer
type Offer struct {
	ID            string     `gorm:"column:id;primaryKey"`
	RawMessageID  *string    `gorm:"column:raw_message_id;index"`
	SourcePhone   string     `gorm:"column:source_phone;not null"`
	SourceName    *string    `gorm:"column:source_name"`
	SourceGroup   string     `gorm:"column:source_group;not null"`
	GroupName     *string    `gorm:"column:group_name"`
	Medication    string     `gorm:"column:medication;not null;index"`
	MedicationRaw string     `gorm:"column:medication_raw;not null"`
	Quantity      float64    `gorm:"column:quantity;default:0"`
	Unit          *string    `gorm:"column:unit"`
	Price         *float64   `gorm:"column:price"`
	Currency      string     `gorm:"column:currency;default:'EGP'"`
	ExpiryDate    *time.Time `gorm:"column:expiry_date"`
	BatchNumber   *string    `gorm:"column:batch_number"`
	Notes         *string    `gorm:"column:notes"`
	RawMessage    string     `gorm:"column:raw_message;not null"`
	Status        string     `gorm:"column:status;not null;default:'ACTIVE';index:idx_offers_status_created"`
	CreatedAt     time.Time  `gorm:"column:created_at;autoCreateTime;index:idx_offers_status_created,sort:desc"`
	UpdatedAt     time.Time  `gorm:"column:updated_at;autoUpdateTime"`

	// Relationships
	RawMessageRef *RawMessage `gorm:"foreignKey:RawMessageID"`
	Matches       []Match     `gorm:"foreignKey:OfferID"`
}

func (Offer) TableName() string {
	return "offers"
}

// Request represents a medication demand request
type Request struct {
	ID            string    `gorm:"column:id;primaryKey"`
	RawMessageID  *string   `gorm:"column:raw_message_id;index"`
	SourcePhone   string    `gorm:"column:source_phone;not null"`
	SourceName    *string   `gorm:"column:source_name"`
	SourceGroup   string    `gorm:"column:source_group;not null"`
	GroupName     *string   `gorm:"column:group_name"`
	Medication    string    `gorm:"column:medication;not null;index"`
	MedicationRaw string    `gorm:"column:medication_raw;not null"`
	Quantity      float64   `gorm:"column:quantity;default:0"`
	Unit          *string   `gorm:"column:unit"`
	MaxPrice      *float64  `gorm:"column:max_price"`
	Currency      string    `gorm:"column:currency;default:'EGP'"`
	Urgent        bool      `gorm:"column:urgent;default:false"`
	Notes         *string   `gorm:"column:notes"`
	RawMessage    string    `gorm:"column:raw_message;not null"`
	Status        string    `gorm:"column:status;not null;default:'ACTIVE';index:idx_requests_status_created"`
	CreatedAt     time.Time `gorm:"column:created_at;autoCreateTime;index:idx_requests_status_created,sort:desc"`
	UpdatedAt     time.Time `gorm:"column:updated_at;autoUpdateTime"`

	// Relationships
	RawMessageRef *RawMessage `gorm:"foreignKey:RawMessageID"`
	Matches       []Match     `gorm:"foreignKey:RequestID"`
}

func (Request) TableName() string {
	return "requests"
}

// Match represents a potential or confirmed match between offer and request
type Match struct {
	ID          string     `gorm:"column:id;primaryKey"`
	OfferID     string     `gorm:"column:offer_id;not null;uniqueIndex:idx_match_offer_request"`
	RequestID   string     `gorm:"column:request_id;not null;uniqueIndex:idx_match_offer_request"`
	Score       float64    `gorm:"column:score;not null"`
	Reasoning   *string    `gorm:"column:reasoning"`
	MatchedBy   *string    `gorm:"column:matched_by"`
	Status      string     `gorm:"column:status;not null;default:'PENDING';index:idx_matches_status_created"`
	CreatedAt   time.Time  `gorm:"column:created_at;autoCreateTime;index:idx_matches_status_created,sort:desc"`
	ConfirmedAt *time.Time `gorm:"column:confirmed_at"`
	Notes       *string    `gorm:"column:notes"`

	// Relationships
	Offer   *Offer   `gorm:"foreignKey:OfferID"`
	Request *Request `gorm:"foreignKey:RequestID"`
}

func (Match) TableName() string {
	return "matches"
}

// MatchQueue represents a job in the persistent matching queue
type MatchQueue struct {
	ID         string    `gorm:"column:id;primaryKey"`
	SourceType string    `gorm:"column:source_type;not null"` // 'OFFER' or 'REQUEST'
	SourceID   string    `gorm:"column:source_id;not null"`
	CreatedAt  time.Time `gorm:"column:created_at;autoCreateTime;index"`
}

func (MatchQueue) TableName() string {
	return "match_queue"
}

// Config represents application configuration key-value pairs
type Config struct {
	Key       string    `gorm:"column:key;primaryKey"`
	Value     string    `gorm:"column:value;not null"`
	UpdatedAt time.Time `gorm:"column:updated_at;autoUpdateTime"`
}

func (Config) TableName() string {
	return "config"
}

// Group represents a monitored WhatsApp group
type Group struct {
	JID          string     `gorm:"column:jid;primaryKey"`
	Name         string     `gorm:"column:name;not null"`
	Description  *string    `gorm:"column:description"`
	Monitored    bool       `gorm:"column:monitored;default:false;index"`
	AddedAt      time.Time  `gorm:"column:added_at;autoCreateTime"`
	LastMessage  *time.Time `gorm:"column:last_message"`
	MessageCount int64      `gorm:"column:message_count;default:0"`
}

func (Group) TableName() string {
	return "groups"
}

// MedicationMapping represents medication name translation/mapping
type MedicationMapping struct {
	ID          string    `gorm:"column:id;primaryKey"`
	ArabicName  string    `gorm:"column:arabic_name;not null;uniqueIndex"`
	EnglishName string    `gorm:"column:english_name;not null"`
	Synonyms    string    `gorm:"column:synonyms;default:'[]'"`
	Embedding   []byte    `gorm:"column:embedding"` // Vector embedding (768-dim float32)
	CreatedAt   time.Time `gorm:"column:created_at;autoCreateTime"`
	UpdatedAt   time.Time `gorm:"column:updated_at;autoUpdateTime"`
}

func (MedicationMapping) TableName() string {
	return "medication_mappings"
}

// FailedMessage represents a message that failed AI processing (dead-letter queue)
type FailedMessage struct {
	ID            string     `gorm:"column:id;primaryKey"`
	RawMessageID  string     `gorm:"column:raw_message_id;uniqueIndex"`
	FailureReason string     `gorm:"column:failure_reason;not null"`
	RetryCount    int        `gorm:"column:retry_count;default:0"`
	FailedAt      time.Time  `gorm:"column:failed_at;autoCreateTime"`
	ResolvedAt    *time.Time `gorm:"column:resolved_at"`

	// Relationships
	RawMessage *RawMessage `gorm:"foreignKey:RawMessageID"`
}

func (FailedMessage) TableName() string {
	return "failed_messages"
}

// MatchFeedback represents operator feedback on a match for learning loop
type MatchFeedback struct {
	ID                 string    `gorm:"column:id;primaryKey"`
	MatchID            string    `gorm:"column:match_id;not null;index"`
	OperatorID         *string   `gorm:"column:operator_id"`
	Decision           string    `gorm:"column:decision;not null;index"` // 'CONFIRMED', 'REJECTED'
	Reason             *string   `gorm:"column:reason"`
	OriginalScore      float64   `gorm:"column:original_score;not null"`
	OriginalConfidence *string   `gorm:"column:original_confidence"`
	CreatedAt          time.Time `gorm:"column:created_at;autoCreateTime;index"`

	// Relationships
	Match *Match `gorm:"foreignKey:MatchID"`
}

func (MatchFeedback) TableName() string {
	return "match_feedback"
}

// DemandLeaderboard represents medication demand statistics (materialized view pattern)
type DemandLeaderboard struct {
	Medication   string    `gorm:"column:medication;primaryKey"`
	RequestCount int       `gorm:"column:request_count;not null;default:0"`
	OfferCount   int       `gorm:"column:offer_count;not null;default:0"`
	DemandRatio  float64   `gorm:"column:demand_ratio;not null;default:0;index:,sort:desc"`
	LastUpdated  time.Time `gorm:"column:last_updated;autoUpdateTime"`
}

func (DemandLeaderboard) TableName() string {
	return "demand_leaderboard"
}

// AuditLog represents system audit entries
type AuditLog struct {
	ID        string    `gorm:"column:id;primaryKey"`
	Action    string    `gorm:"column:action;not null;index"`
	EntityID  *string   `gorm:"column:entity_id;index"`
	OldValue  *string   `gorm:"column:old_value"`
	NewValue  *string   `gorm:"column:new_value"`
	Details   *string   `gorm:"column:details"`
	IPAddress *string   `gorm:"column:ip_address"`
	CreatedAt time.Time `gorm:"column:created_at;autoCreateTime;index:,sort:desc"`
}

func (AuditLog) TableName() string {
	return "audit_logs"
}

// UnmappedMedication represents a medication that couldn't be mapped for review
type UnmappedMedication struct {
	ID            uint       `gorm:"column:id;primaryKey;autoIncrement"`
	RawText       string     `gorm:"column:raw_text;uniqueIndex;not null"`
	AIOutput      string     `gorm:"column:ai_output"`
	SourceMessage string     `gorm:"column:source_message;type:text"`
	SourceGroup   string     `gorm:"column:source_group;index"`
	MessageID     string     `gorm:"column:message_id;index"`
	Count         int        `gorm:"column:count;default:1"`
	Reviewed      bool       `gorm:"column:reviewed;default:false;index"`
	ApprovedName  string     `gorm:"column:approved_name"`
	ReviewedAt    *time.Time `gorm:"column:reviewed_at"`
	ReviewedBy    string     `gorm:"column:reviewed_by"`
	CreatedAt     time.Time  `gorm:"column:created_at;autoCreateTime"`
	UpdatedAt     time.Time  `gorm:"column:updated_at;autoUpdateTime"`
}

func (UnmappedMedication) TableName() string {
	return "unmapped_medications"
}
