// Package entity contains pure domain entities with no external dependencies.
// These are the core business objects of PharmaBroker.
package entity

import "time"

// MessageType categorizes incoming WhatsApp messages
type MessageType string

const (
	MessageTypeOffer   MessageType = "OFFER"
	MessageTypeRequest MessageType = "REQUEST"
	MessageTypeBoth    MessageType = "BOTH"
	MessageTypeUnknown MessageType = "UNKNOWN"
)

// ItemStatus tracks lifecycle of offers/requests
type ItemStatus string

const (
	StatusActive   ItemStatus = "ACTIVE"
	StatusMatched  ItemStatus = "MATCHED"
	StatusExpired  ItemStatus = "EXPIRED"
	StatusArchived ItemStatus = "ARCHIVED"
)

// MatchStatus tracks lifecycle of matches
type MatchStatus string

const (
	MatchStatusPending   MatchStatus = "PENDING"
	MatchStatusConfirmed MatchStatus = "CONFIRMED"
	MatchStatusRejected  MatchStatus = "REJECTED"
)

// ReviewStatus represents the status of a review queue item
type ReviewStatus string

const (
	ReviewStatusPending  ReviewStatus = "PENDING"
	ReviewStatusApproved ReviewStatus = "APPROVED"
	ReviewStatusRejected ReviewStatus = "REJECTED"
)

// FeedbackDecision represents the operator's decision on a match
type FeedbackDecision string

const (
	FeedbackConfirmed FeedbackDecision = "CONFIRMED"
	FeedbackRejected  FeedbackDecision = "REJECTED"
)

// RawMessage represents an incoming WhatsApp message before AI processing
type RawMessage struct {
	ID          string     `json:"id"`
	ExternalID  string     `json:"external_id"` // WhatsApp Message ID
	GroupJID    string     `json:"group_jid"`
	GroupName   string     `json:"group_name"`
	SenderJID   string     `json:"sender_jid"`
	SenderPhone string     `json:"sender_phone"`
	SenderName  string     `json:"sender_name"`
	Content     string     `json:"content"`
	Timestamp   time.Time  `json:"timestamp"`
	ProcessedAt *time.Time `json:"processed_at,omitempty"`
	Error       string     `json:"error,omitempty"`

	// Reply context (for messages replying to other messages)
	ReplyToID      string `json:"reply_to_id,omitempty"`
	ReplyToContent string `json:"reply_to_content,omitempty"`
	ReplyToSender  string `json:"reply_to_sender,omitempty"`
}

func (rm *RawMessage) GetTimestamp() time.Time {
	return rm.Timestamp
}

func (rm *RawMessage) GetContent() string {
	return rm.Content
}

// Offer represents a medication supply offer
type Offer struct {
	ID            string     `json:"id"`
	RawMessageID  string     `json:"raw_message_id"`
	SourcePhone   string     `json:"source_phone"`
	SourceName    string     `json:"source_name"`
	SourceGroup   string     `json:"source_group"`
	GroupName     string     `json:"group_name"`
	Medication    string     `json:"medication"`
	MedicationRaw string     `json:"medication_raw"`
	Quantity      float64    `json:"quantity"`
	Unit          *string    `json:"unit,omitempty"`
	Price         float64    `json:"price,omitempty"`
	Currency      string     `json:"currency,omitempty"`
	ExpiryDate    *time.Time `json:"expiry_date,omitempty"`
	BatchNumber   string     `json:"batch_number,omitempty"`
	Notes         string     `json:"notes,omitempty"`
	RawMessage    string     `json:"raw_message"`
	Status        ItemStatus `json:"status"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
}

// Request represents a medication demand request
type Request struct {
	ID            string     `json:"id"`
	RawMessageID  string     `json:"raw_message_id"`
	SourcePhone   string     `json:"source_phone"`
	SourceName    string     `json:"source_name"`
	SourceGroup   string     `json:"source_group"`
	GroupName     string     `json:"group_name"`
	Medication    string     `json:"medication"`
	MedicationRaw string     `json:"medication_raw"`
	Quantity      float64    `json:"quantity"`
	Unit          *string    `json:"unit,omitempty"`
	MaxPrice      float64    `json:"max_price,omitempty"`
	Currency      string     `json:"currency,omitempty"`
	Urgent        bool       `json:"urgent"`
	Notes         string     `json:"notes,omitempty"`
	RawMessage    string     `json:"raw_message"`
	Status        ItemStatus `json:"status"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
}

// Match represents a potential or confirmed match between offer and request
type Match struct {
	ID          string      `json:"id"`
	OfferID     string      `json:"offer_id"`
	RequestID   string      `json:"request_id"`
	Score       float64     `json:"score"`
	Reasoning   string      `json:"reasoning"`
	MatchedBy   string      `json:"matched_by"`
	Status      MatchStatus `json:"status"`
	CreatedAt   time.Time   `json:"created_at"`
	ConfirmedAt *time.Time  `json:"confirmed_at,omitempty"`
	Notes       string      `json:"notes,omitempty"`
}

// MatchWithDetails includes full offer and request data for display
type MatchWithDetails struct {
	Match
	Offer   *Offer   `json:"offer"`
	Request *Request `json:"request"`
}

// Group represents a monitored WhatsApp group
type Group struct {
	JID          string     `json:"jid"`
	Name         string     `json:"name"`
	Description  string     `json:"description,omitempty"`
	Monitored    bool       `json:"monitored"`
	AddedAt      time.Time  `json:"added_at"`
	LastMessage  *time.Time `json:"last_message,omitempty"`
	MessageCount int64      `json:"message_count"`
}

// Stats represents dashboard statistics
type Stats struct {
	ActiveOffers     int64   `json:"active_offers"`
	ActiveRequests   int64   `json:"active_requests"`
	PendingMatches   int64   `json:"pending_matches"`
	ConfirmedToday   int64   `json:"confirmed_today"`
	ProcessedToday   int64   `json:"processed_today"`
	AvgMatchScore    float64 `json:"avg_match_score"`
	MonitoredGroups  int     `json:"monitored_groups"`
	ConnectedClients int     `json:"connected_clients"`
}
