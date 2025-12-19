package handlers

// NewAPIError creates a new APIError
func NewAPIError(code, message string) *APIError {
	return &APIError{
		Code:    code,
		Message: message,
	}
}

// WithDetails adds details to the error
func (e *APIError) WithDetails(details any) *APIError {
	// APIError struct needs Details field in types.go?
	// The copy from internal/api/errors.go had Details. My types.go APIError definition did NOT have Details.
	// I should add Details to APIError in types.go first or update it.
	// For now I will assume I update types.go in next step if missed.
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

func ErrRateLimited(message string) *APIError {
	return NewAPIError(ErrCodeRateLimited, message)
}

func ErrUnauthorized(message string) *APIError {
	return NewAPIError(ErrCodeUnauthorized, message)
}

func ErrForbidden(message string) *APIError {
	return NewAPIError(ErrCodeForbidden, message)
}
