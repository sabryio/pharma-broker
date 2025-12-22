// Package errors provides domain-specific error types with context and correlation support.
package errors

import (
	"fmt"
)

// Code represents a domain error code.
type Code string

// Domain error codes
const (
	// General errors
	CodeUnknown       Code = "UNKNOWN"
	CodeInvalidInput  Code = "INVALID_INPUT"
	CodeNotFound      Code = "NOT_FOUND"
	CodeAlreadyExists Code = "ALREADY_EXISTS"
	CodeUnauthorized  Code = "UNAUTHORIZED"
	CodeForbidden     Code = "FORBIDDEN"

	// Matching errors
	CodeMatchNotFound    Code = "MATCH_NOT_FOUND"
	CodeMatchFailed      Code = "MATCH_FAILED"
	CodeInsufficientData Code = "INSUFFICIENT_DATA"

	// AI/Parsing errors
	CodeAIProviderFailed Code = "AI_PROVIDER_FAILED"
	CodeParsingFailed    Code = "PARSING_FAILED"
	CodeTokenLimitExceed Code = "TOKEN_LIMIT_EXCEEDED"

	// Messaging errors
	CodeMessageFailed   Code = "MESSAGE_FAILED"
	CodeConnectionLost  Code = "CONNECTION_LOST"
	CodeRateLimited     Code = "RATE_LIMITED"
	CodeMessageTooLarge Code = "MESSAGE_TOO_LARGE"

	// Storage errors
	CodeStorageFailed Code = "STORAGE_FAILED"
	CodeDBError       Code = "DB_ERROR"
)

// DomainError represents a domain-specific error with context.
type DomainError struct {
	Code          Code           // Machine-readable error code
	Message       string         // Human-readable message
	Cause         error          // Underlying error (for wrapping)
	Context       map[string]any // Additional context
	CorrelationID string         // Request/trace correlation ID
}

// Error implements the error interface.
func (e *DomainError) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("[%s] %s: %v", e.Code, e.Message, e.Cause)
	}
	return fmt.Sprintf("[%s] %s", e.Code, e.Message)
}

// Unwrap returns the underlying error for errors.Is/As support.
func (e *DomainError) Unwrap() error {
	return e.Cause
}

// Is checks if the error matches a target error by code.
func (e *DomainError) Is(target error) bool {
	if t, ok := target.(*DomainError); ok {
		return e.Code == t.Code
	}
	return false
}

// WithContext adds context to the error.
func (e *DomainError) WithContext(key string, value any) *DomainError {
	if e.Context == nil {
		e.Context = make(map[string]any)
	}
	e.Context[key] = value
	return e
}

// WithCorrelationID sets the correlation ID.
func (e *DomainError) WithCorrelationID(id string) *DomainError {
	e.CorrelationID = id
	return e
}

// New creates a new DomainError.
func New(code Code, message string) *DomainError {
	return &DomainError{
		Code:    code,
		Message: message,
	}
}

// Wrap wraps an existing error with domain context.
func Wrap(err error, code Code, message string) *DomainError {
	return &DomainError{
		Code:    code,
		Message: message,
		Cause:   err,
	}
}

// Wrapf wraps an error with formatted message.
func Wrapf(err error, code Code, format string, args ...any) *DomainError {
	return &DomainError{
		Code:    code,
		Message: fmt.Sprintf(format, args...),
		Cause:   err,
	}
}
