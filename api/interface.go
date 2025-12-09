// Package api provides HTTP API server components.
package api

import (
	"context"
	"net/http"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Server defines the HTTP server interface
type Server interface {
	Start(ctx context.Context) error
	Stop(ctx context.Context) error
	Handler() http.Handler
}

// Config holds API server configuration
type Config struct {
	Host           string
	Port           int
	AllowedOrigins []string
	RateLimit      int // requests per second
	ReadTimeout    int // seconds
	WriteTimeout   int // seconds
}

// Repositories bundles all repository dependencies for handlers
type Repositories struct {
	Offers   repository.OfferRepository
	Requests repository.RequestRepository
	Matches  repository.MatchRepository
	Groups   repository.GroupRepository
	Stats    repository.StatsRepository
	Messages repository.RawMessageRepository
	Mappings repository.MedicationMappingRepository
	Queue    repository.MatchQueueRepository
	Review   repository.ReviewQueueRepository
}

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
	ErrCodeBadRequest   = "BAD_REQUEST"
	ErrCodeNotFound     = "NOT_FOUND"
	ErrCodeInternal     = "INTERNAL_ERROR"
	ErrCodeUnauthorized = "UNAUTHORIZED"
	ErrCodeRateLimited  = "RATE_LIMITED"
)

// SSEEvent represents a server-sent event
type SSEEvent struct {
	Type string `json:"type"`
	Data any    `json:"data"`
}

// SSEHub defines interface for SSE broadcasting
type SSEHub interface {
	Broadcast(event SSEEvent)
	ServeHTTP(w http.ResponseWriter, r *http.Request)
	ClientCount() int
}

// HealthStatus represents system health
type HealthStatus struct {
	Status     string            `json:"status"` // "healthy", "degraded", "unhealthy"
	Components map[string]string `json:"components"`
	Timestamp  string            `json:"timestamp"`
}

// Compile-time check that entity types are used
var _ = entity.Offer{}
