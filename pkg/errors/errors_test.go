package errors

import (
	"errors"
	"testing"
)

func TestDomainError_Error(t *testing.T) {
	tests := []struct {
		name     string
		err      *DomainError
		expected string
	}{
		{
			name:     "without cause",
			err:      New(CodeNotFound, "user not found"),
			expected: "[NOT_FOUND] user not found",
		},
		{
			name:     "with cause",
			err:      Wrap(errors.New("db error"), CodeDBError, "failed to query"),
			expected: "[DB_ERROR] failed to query: db error",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.Error(); got != tt.expected {
				t.Errorf("Error() = %q, want %q", got, tt.expected)
			}
		})
	}
}

func TestDomainError_Unwrap(t *testing.T) {
	cause := errors.New("underlying error")
	err := Wrap(cause, CodeDBError, "wrapped")

	if unwrapped := err.Unwrap(); unwrapped != cause {
		t.Errorf("Unwrap() = %v, want %v", unwrapped, cause)
	}
}

func TestDomainError_Is(t *testing.T) {
	err := New(CodeNotFound, "specific message")

	// Should match sentinel by code
	if !errors.Is(err, ErrNotFound) {
		t.Error("expected err to match ErrNotFound")
	}

	// Should not match different code
	if errors.Is(err, ErrDBError) {
		t.Error("expected err to NOT match ErrDBError")
	}
}

func TestDomainError_WithContext(t *testing.T) {
	err := New(CodeNotFound, "user not found").
		WithContext("user_id", "123").
		WithContext("action", "lookup")

	if err.Context["user_id"] != "123" {
		t.Errorf("Context[user_id] = %v, want 123", err.Context["user_id"])
	}
	if err.Context["action"] != "lookup" {
		t.Errorf("Context[action] = %v, want lookup", err.Context["action"])
	}
}

func TestDomainError_WithCorrelationID(t *testing.T) {
	err := New(CodeNotFound, "not found").WithCorrelationID("req-123")

	if err.CorrelationID != "req-123" {
		t.Errorf("CorrelationID = %q, want %q", err.CorrelationID, "req-123")
	}
}

func TestWrapf(t *testing.T) {
	cause := errors.New("connection refused")
	err := Wrapf(cause, CodeConnectionLost, "failed to connect to %s:%d", "localhost", 5432)

	expected := "[CONNECTION_LOST] failed to connect to localhost:5432: connection refused"
	if err.Error() != expected {
		t.Errorf("Error() = %q, want %q", err.Error(), expected)
	}
}

func TestGetCode(t *testing.T) {
	tests := []struct {
		name     string
		err      error
		expected Code
	}{
		{
			name:     "domain error",
			err:      New(CodeNotFound, "not found"),
			expected: CodeNotFound,
		},
		{
			name:     "wrapped domain error",
			err:      Wrap(New(CodeDBError, "db"), CodeStorageFailed, "storage"),
			expected: CodeStorageFailed,
		},
		{
			name:     "standard error",
			err:      errors.New("standard"),
			expected: CodeUnknown,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := GetCode(tt.err); got != tt.expected {
				t.Errorf("GetCode() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestGetCorrelationID(t *testing.T) {
	err := New(CodeNotFound, "not found").WithCorrelationID("trace-456")

	if got := GetCorrelationID(err); got != "trace-456" {
		t.Errorf("GetCorrelationID() = %q, want %q", got, "trace-456")
	}

	// Standard error returns empty
	if got := GetCorrelationID(errors.New("standard")); got != "" {
		t.Errorf("GetCorrelationID(standard) = %q, want empty", got)
	}
}

func TestGetContext(t *testing.T) {
	err := New(CodeNotFound, "not found").WithContext("key", "value")

	ctx := GetContext(err)
	if ctx == nil || ctx["key"] != "value" {
		t.Errorf("GetContext() = %v, want map with key=value", ctx)
	}

	// Standard error returns nil
	if got := GetContext(errors.New("standard")); got != nil {
		t.Errorf("GetContext(standard) = %v, want nil", got)
	}
}
