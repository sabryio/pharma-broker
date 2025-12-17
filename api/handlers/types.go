package handlers

// Response represents a standard API response
type Response struct {
	Success bool      `json:"success"`
	Data    any       `json:"data,omitempty"`
	Error   *APIError `json:"error,omitempty"`
	Meta    *Meta     `json:"meta,omitempty"`
}

// Meta contains pagination metadata
type Meta struct {
	Total  int64 `json:"total,omitempty"`
	Limit  int   `json:"limit,omitempty"`
	Offset int   `json:"offset,omitempty"`
}

// APIError represents a structured API error
type APIError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

// Error codes
const (
	// General errors (1xxx)
	ErrCodeInternal         = "ERR_INTERNAL"
	ErrCodeBadRequest       = "ERR_BAD_REQUEST"
	ErrCodeNotFound         = "ERR_NOT_FOUND"
	ErrCodeMethodNotAllowed = "ERR_METHOD_NOT_ALLOWED"
	ErrCodeValidation       = "ERR_VALIDATION"
	ErrCodeUnauthorized     = "ERR_UNAUTHORIZED"
	ErrCodeForbidden        = "ERR_FORBIDDEN"
	ErrCodeRateLimited      = "ERR_RATE_LIMITED"

	// Resource-specific errors (2xxx)
	ErrCodeOfferNotFound   = "ERR_OFFER_NOT_FOUND"
	ErrCodeRequestNotFound = "ERR_REQUEST_NOT_FOUND"
	ErrCodeMatchNotFound   = "ERR_MATCH_NOT_FOUND"
	ErrCodeGroupNotFound   = "ERR_GROUP_NOT_FOUND"

	// Operation errors (3xxx)
	ErrCodeDatabaseError = "ERR_DATABASE"
	ErrCodeAIParseError  = "ERR_AI_PARSE"
	ErrCodeWhatsAppError = "ERR_WHATSAPP"
	ErrCodeConfigError   = "ERR_CONFIG"

	// Match-specific errors (4xxx)
	ErrCodeMatchAlreadyConfirmed = "ERR_MATCH_ALREADY_CONFIRMED"
	ErrCodeMatchAlreadyRejected  = "ERR_MATCH_ALREADY_REJECTED"
)

// AnalyzeResult represents AI analysis output
type AnalyzeResult struct {
	Items   []AnalyzeItem `json:"items"`
	RawJSON string        `json:"raw_json,omitempty"`
}

// AnalyzeItem represents a single parsed item
type AnalyzeItem struct {
	Type          string  `json:"type"`
	Medication    string  `json:"medication"`
	MedicationRaw string  `json:"medication_raw"`
	Quantity      int     `json:"quantity"`
	Unit          string  `json:"unit,omitempty"`
	Price         float64 `json:"price,omitempty"`
	MaxPrice      float64 `json:"max_price,omitempty"`
	Currency      string  `json:"currency,omitempty"`
	ExpiryDate    string  `json:"expiry_date,omitempty"`
	BatchNumber   string  `json:"batch_number,omitempty"`
	Urgent        bool    `json:"urgent,omitempty"`
	Notes         string  `json:"notes,omitempty"`
}
