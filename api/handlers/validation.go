// Package handlers provides HTTP request handlers.
package handlers

import (
	"fmt"
	"net/http"
	"regexp"
	"strings"
	"unicode/utf8"

	"github.com/gin-gonic/gin"
)

// ValidationError represents a field validation error
type ValidationError struct {
	Field   string `json:"field"`
	Message string `json:"message"`
	Value   any    `json:"value,omitempty"`
}

// ValidationErrors holds multiple validation errors
type ValidationErrors struct {
	Errors []ValidationError `json:"errors"`
}

// Error implements the error interface
func (v ValidationErrors) Error() string {
	if len(v.Errors) == 0 {
		return "validation failed"
	}
	msgs := make([]string, len(v.Errors))
	for i, e := range v.Errors {
		msgs[i] = fmt.Sprintf("%s: %s", e.Field, e.Message)
	}
	return strings.Join(msgs, "; ")
}

// HasErrors returns true if there are validation errors
func (v ValidationErrors) HasErrors() bool {
	return len(v.Errors) > 0
}

// Add adds a validation error
func (v *ValidationErrors) Add(field, message string) {
	v.Errors = append(v.Errors, ValidationError{Field: field, Message: message})
}

// AddWithValue adds a validation error with the invalid value
func (v *ValidationErrors) AddWithValue(field, message string, value any) {
	v.Errors = append(v.Errors, ValidationError{Field: field, Message: message, Value: value})
}

// Validator provides fluent validation for request data
type Validator struct {
	errors ValidationErrors
}

// NewValidator creates a new validator
func NewValidator() *Validator {
	return &Validator{}
}

// Errors returns the validation errors
func (v *Validator) Errors() ValidationErrors {
	return v.errors
}

// HasErrors returns true if validation failed
func (v *Validator) HasErrors() bool {
	return v.errors.HasErrors()
}

// Required validates that a string field is not empty
func (v *Validator) Required(field, value string) *Validator {
	if strings.TrimSpace(value) == "" {
		v.errors.Add(field, "is required")
	}
	return v
}

// RequiredInt validates that an int field is not zero
func (v *Validator) RequiredInt(field string, value int) *Validator {
	if value == 0 {
		v.errors.Add(field, "is required")
	}
	return v
}

// MinLength validates minimum string length
func (v *Validator) MinLength(field, value string, min int) *Validator {
	if utf8.RuneCountInString(value) < min {
		v.errors.Add(field, fmt.Sprintf("must be at least %d characters", min))
	}
	return v
}

// MaxLength validates maximum string length
func (v *Validator) MaxLength(field, value string, max int) *Validator {
	if utf8.RuneCountInString(value) > max {
		v.errors.Add(field, fmt.Sprintf("must be at most %d characters", max))
	}
	return v
}

// Range validates an integer is within a range
func (v *Validator) Range(field string, value, min, max int) *Validator {
	if value < min || value > max {
		v.errors.Add(field, fmt.Sprintf("must be between %d and %d", min, max))
	}
	return v
}

// RangeFloat validates a float is within a range
func (v *Validator) RangeFloat(field string, value, min, max float64) *Validator {
	if value < min || value > max {
		v.errors.Add(field, fmt.Sprintf("must be between %.2f and %.2f", min, max))
	}
	return v
}

// Positive validates that a number is positive
func (v *Validator) Positive(field string, value int) *Validator {
	if value <= 0 {
		v.errors.Add(field, "must be positive")
	}
	return v
}

// PositiveFloat validates that a float is positive
func (v *Validator) PositiveFloat(field string, value float64) *Validator {
	if value <= 0 {
		v.errors.Add(field, "must be positive")
	}
	return v
}

// NonNegative validates that a number is non-negative
func (v *Validator) NonNegative(field string, value int) *Validator {
	if value < 0 {
		v.errors.Add(field, "must be non-negative")
	}
	return v
}

// OneOf validates that a value is one of the allowed values
func (v *Validator) OneOf(field, value string, allowed ...string) *Validator {
	if value == "" {
		return v // Skip if empty (use Required for mandatory fields)
	}
	for _, a := range allowed {
		if value == a {
			return v
		}
	}
	v.errors.AddWithValue(field, fmt.Sprintf("must be one of: %s", strings.Join(allowed, ", ")), value)
	return v
}

// Pattern validates a string matches a regex pattern
func (v *Validator) Pattern(field, value, pattern, description string) *Validator {
	if value == "" {
		return v // Skip if empty
	}
	matched, err := regexp.MatchString(pattern, value)
	if err != nil || !matched {
		v.errors.Add(field, description)
	}
	return v
}

// Email validates an email format
func (v *Validator) Email(field, value string) *Validator {
	if value == "" {
		return v
	}
	// Simple email regex - not RFC 5322 compliant but practical
	pattern := `^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$`
	if matched, _ := regexp.MatchString(pattern, value); !matched {
		v.errors.Add(field, "must be a valid email address")
	}
	return v
}

// Phone validates a phone number format (international)
func (v *Validator) Phone(field, value string) *Validator {
	if value == "" {
		return v
	}
	// Allow +, digits, spaces, dashes, parentheses
	pattern := `^[\+]?[(]?[0-9]{1,4}[)]?[-\s\./0-9]*$`
	if matched, _ := regexp.MatchString(pattern, value); !matched || len(value) < 7 {
		v.errors.Add(field, "must be a valid phone number")
	}
	return v
}

// UUID validates a UUID format
func (v *Validator) UUID(field, value string) *Validator {
	if value == "" {
		return v
	}
	pattern := `^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`
	if matched, _ := regexp.MatchString(pattern, value); !matched {
		v.errors.Add(field, "must be a valid UUID")
	}
	return v
}

// URL validates a URL format
func (v *Validator) URL(field, value string) *Validator {
	if value == "" {
		return v
	}
	pattern := `^https?://[^\s/$.?#].[^\s]*$`
	if matched, _ := regexp.MatchString(pattern, value); !matched {
		v.errors.Add(field, "must be a valid URL")
	}
	return v
}

// NoHTML validates that a string contains no HTML tags (XSS prevention)
func (v *Validator) NoHTML(field, value string) *Validator {
	if value == "" {
		return v
	}
	pattern := `<[^>]*>`
	if matched, _ := regexp.MatchString(pattern, value); matched {
		v.errors.Add(field, "must not contain HTML tags")
	}
	return v
}

// SafeString validates a string contains only safe characters
func (v *Validator) SafeString(field, value string) *Validator {
	if value == "" {
		return v
	}
	// Allow alphanumeric, spaces, common punctuation, Arabic characters
	pattern := `^[\p{L}\p{N}\s\-_.,!?@#$%&*()+=:;'"\/\[\]{}]+$`
	if matched, _ := regexp.MatchString(pattern, value); !matched {
		v.errors.Add(field, "contains invalid characters")
	}
	return v
}

// Custom adds a custom validation
func (v *Validator) Custom(field string, valid bool, message string) *Validator {
	if !valid {
		v.errors.Add(field, message)
	}
	return v
}

// ValidateGin validates and sends error response if validation fails
// Returns true if validation passed, false if it failed (and response was sent)
func (v *Validator) ValidateGin(c *gin.Context) bool {
	if !v.HasErrors() {
		return true
	}

	c.JSON(http.StatusBadRequest, Response{
		Success: false,
		Error: &APIError{
			Code:    ErrCodeValidation,
			Message: v.errors.Error(),
		},
		Data: v.errors.Errors,
	})
	return false
}
