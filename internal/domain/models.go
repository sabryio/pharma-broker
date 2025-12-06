package domain

import (
	"time"
)

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

// RawMessage represents an incoming WhatsApp message before AI processing
type RawMessage struct {
	ID          string     `json:"id"`
	GroupJID    string     `json:"group_jid"`
	GroupName   string     `json:"group_name"`
	SenderJID   string     `json:"sender_jid"`
	SenderPhone string     `json:"sender_phone"`
	SenderName  string     `json:"sender_name"`
	Content     string     `json:"content"`
	Timestamp   time.Time  `json:"timestamp"`
	ProcessedAt *time.Time `json:"processed_at,omitempty"`
	Error       string     `json:"error,omitempty"`
}

// Offer represents a medication supply offer
type Offer struct {
	ID            string     `json:"id"`
	RawMessageID  string     `json:"raw_message_id"`
	SourcePhone   string     `json:"source_phone"`
	SourceName    string     `json:"source_name"`
	SourceGroup   string     `json:"source_group"`
	GroupName     string     `json:"group_name"`
	Medication    string     `json:"medication"`     // Normalized name
	MedicationRaw string     `json:"medication_raw"` // Original text
	Quantity      float64    `json:"quantity"`
	Unit          *string    `json:"unit,omitempty"` // e.g., "boxes", "strips"
	Price         float64    `json:"price,omitempty"`
	Currency      string     `json:"currency,omitempty"` // Default EGP
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
	Medication    string     `json:"medication"`     // Normalized name
	MedicationRaw string     `json:"medication_raw"` // Original text
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
	Score       float64     `json:"score"`      // AI-computed similarity (0-1)
	Reasoning   string      `json:"reasoning"`  // AI explanation for match
	MatchedBy   string      `json:"matched_by"` // Operator who confirmed
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

// ParsedItem represents a single item extracted by AI from a message
type ParsedItem struct {
	Type          MessageType `json:"type" jsonschema:"enum=OFFER,enum=REQUEST,description=Type of the listing (OFFER or REQUEST)"`
	Medication    string      `json:"medication" jsonschema_description:"Normalized medication name (English preferred)"`
	MedicationRaw string      `json:"medication_raw" jsonschema_description:"Exact text from message referring to medication"`
	Quantity      float64     `json:"quantity,omitempty" jsonschema_description:"Numeric quantity (supports decimals like 0.5)"`
	Unit          *string     `json:"unit,omitempty" jsonschema_description:"Unit of measure (e.g. boxes, strips, ampoules)"`
	Price         float64     `json:"price,omitempty" jsonschema_description:"Price per unit if specified"`
	MaxPrice      float64     `json:"max_price,omitempty" jsonschema_description:"Maximum price willing to pay (for requests)"`
	// Removed: Currency, ExpiryDate, BatchNumber (moved to Notes)
	Urgent bool   `json:"urgent,omitempty" jsonschema_description:"If the user explicitly mentions urgent/emergency"`
	Notes  string `json:"notes,omitempty" jsonschema_description:"Any other details (expiry, batch, currency, location, unknown text)"`
}

// AIParseResult represents the AI response for a message
type AIParseResult struct {
	Items   []ParsedItem `json:"items" jsonschema_description:"List of pharmaceutical items found in the message"`
	Error   string       `json:"error,omitempty" jsonschema_description:"Error message if parsing failed, otherwise empty"`
	RawJSON string       `json:"-"` // Raw JSON for debugging
}
