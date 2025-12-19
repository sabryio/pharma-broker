package config

import "errors"

var (
	// ErrMissingAPIKey is returned when GEMINI_API_KEY is not set
	ErrMissingAPIKey = errors.New("GEMINI_API_KEY environment variable is required")
)
