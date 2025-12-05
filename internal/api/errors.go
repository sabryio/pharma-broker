package api

// Error codes for machine-readable API responses
const (
	// General errors (1xxx)
	ErrCodeInternal         = "ERR_INTERNAL"
	ErrCodeBadRequest       = "ERR_BAD_REQUEST"
	ErrCodeNotFound         = "ERR_NOT_FOUND"
	ErrCodeMethodNotAllowed = "ERR_METHOD_NOT_ALLOWED"
	ErrCodeValidation       = "ERR_VALIDATION"

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

// APIError represents a structured error response
type APIError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
	Details any    `json:"details,omitempty"`
}

// NewAPIError creates a new APIError
func NewAPIError(code, message string) *APIError {
	return &APIError{
		Code:    code,
		Message: message,
	}
}

// WithDetails adds details to the error
func (e *APIError) WithDetails(details any) *APIError {
	e.Details = details
	return e
}

// Common error constructors for convenience
func ErrInternal(message string) *APIError {
	return NewAPIError(ErrCodeInternal, message)
}

func ErrBadRequest(message string) *APIError {
	return NewAPIError(ErrCodeBadRequest, message)
}

func ErrNotFound(message string) *APIError {
	return NewAPIError(ErrCodeNotFound, message)
}

func ErrValidation(message string) *APIError {
	return NewAPIError(ErrCodeValidation, message)
}

func ErrDatabase(message string) *APIError {
	return NewAPIError(ErrCodeDatabaseError, message)
}

func ErrOfferNotFound() *APIError {
	return NewAPIError(ErrCodeOfferNotFound, "Offer not found")
}

func ErrRequestNotFound() *APIError {
	return NewAPIError(ErrCodeRequestNotFound, "Request not found")
}

func ErrMatchNotFound() *APIError {
	return NewAPIError(ErrCodeMatchNotFound, "Match not found")
}

func ErrGroupNotFound() *APIError {
	return NewAPIError(ErrCodeGroupNotFound, "Group not found")
}

func ErrAIParse(message string) *APIError {
	return NewAPIError(ErrCodeAIParseError, message)
}

func ErrWhatsApp(message string) *APIError {
	return NewAPIError(ErrCodeWhatsAppError, message)
}

func ErrConfig(message string) *APIError {
	return NewAPIError(ErrCodeConfigError, message)
}
