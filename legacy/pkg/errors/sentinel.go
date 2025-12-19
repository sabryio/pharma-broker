package errors

import "errors"

// Sentinel errors for common cases - use with errors.Is()
var (
	// General
	ErrNotFound      = New(CodeNotFound, "resource not found")
	ErrInvalidInput  = New(CodeInvalidInput, "invalid input")
	ErrAlreadyExists = New(CodeAlreadyExists, "resource already exists")
	ErrUnauthorized  = New(CodeUnauthorized, "unauthorized")
	ErrForbidden     = New(CodeForbidden, "forbidden")

	// Matching
	ErrMatchNotFound    = New(CodeMatchNotFound, "match not found")
	ErrMatchFailed      = New(CodeMatchFailed, "matching failed")
	ErrInsufficientData = New(CodeInsufficientData, "insufficient data")

	// AI/Parsing
	ErrAIProviderFailed = New(CodeAIProviderFailed, "AI provider failed")
	ErrParsingFailed    = New(CodeParsingFailed, "parsing failed")
	ErrTokenLimitExceed = New(CodeTokenLimitExceed, "token limit exceeded")

	// Messaging
	ErrMessageFailed   = New(CodeMessageFailed, "message delivery failed")
	ErrConnectionLost  = New(CodeConnectionLost, "connection lost")
	ErrRateLimited     = New(CodeRateLimited, "rate limited")
	ErrMessageTooLarge = New(CodeMessageTooLarge, "message too large")

	// Storage
	ErrStorageFailed = New(CodeStorageFailed, "storage operation failed")
	ErrDBError       = New(CodeDBError, "database error")
)

// Is wraps errors.Is for convenience.
func Is(err, target error) bool {
	return errors.Is(err, target)
}

// As wraps errors.As for convenience.
func As(err error, target any) bool {
	return errors.As(err, target)
}

// GetCode extracts the error code from a DomainError.
// Returns CodeUnknown if not a DomainError.
func GetCode(err error) Code {
	var de *DomainError
	if errors.As(err, &de) {
		return de.Code
	}
	return CodeUnknown
}

// GetCorrelationID extracts the correlation ID from a DomainError.
// Returns empty string if not found.
func GetCorrelationID(err error) string {
	var de *DomainError
	if errors.As(err, &de) {
		return de.CorrelationID
	}
	return ""
}

// GetContext extracts context from a DomainError.
// Returns nil if not a DomainError.
func GetContext(err error) map[string]any {
	var de *DomainError
	if errors.As(err, &de) {
		return de.Context
	}
	return nil
}
