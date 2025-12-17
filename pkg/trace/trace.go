// Package trace provides trace context propagation for observability.
package trace

import (
	"context"
	"crypto/rand"
	"encoding/hex"

	"github.com/rs/zerolog"
)

// contextKey is a private type for context keys.
type contextKey string

const (
	traceContextKey contextKey = "trace_context"

	// HTTP headers for trace propagation (W3C Trace Context)
	HeaderTraceParent = "traceparent"
	HeaderTraceState  = "tracestate"

	// Legacy headers
	HeaderXRequestID = "X-Request-ID"
	HeaderXTraceID   = "X-Trace-ID"
)

// Context holds trace information for request correlation.
type Context struct {
	TraceID   string // 32-char hex (128-bit)
	SpanID    string // 16-char hex (64-bit)
	ParentID  string // Parent span ID (optional)
	Sampled   bool   // Whether to sample this trace
	RequestID string // Application-level request ID
}

// New creates a new trace context with generated IDs.
func New() *Context {
	return &Context{
		TraceID:   generateID(16), // 128-bit
		SpanID:    generateID(8),  // 64-bit
		Sampled:   true,
		RequestID: generateID(8),
	}
}

// NewSpan creates a child span from the current context.
func (tc *Context) NewSpan() *Context {
	return &Context{
		TraceID:   tc.TraceID,
		SpanID:    generateID(8),
		ParentID:  tc.SpanID,
		Sampled:   tc.Sampled,
		RequestID: tc.RequestID,
	}
}

// WithContext stores the trace context in a context.Context.
func WithContext(ctx context.Context, tc *Context) context.Context {
	return context.WithValue(ctx, traceContextKey, tc)
}

// FromContext retrieves the trace context from a context.Context.
// Returns nil if not found.
func FromContext(ctx context.Context) *Context {
	if tc, ok := ctx.Value(traceContextKey).(*Context); ok {
		return tc
	}
	return nil
}

// FromContextOrNew retrieves trace context or creates a new one.
func FromContextOrNew(ctx context.Context) (*Context, context.Context) {
	tc := FromContext(ctx)
	if tc == nil {
		tc = New()
		ctx = WithContext(ctx, tc)
	}
	return tc, ctx
}

// Logger returns a zerolog logger enriched with trace context.
func Logger(ctx context.Context, log zerolog.Logger) zerolog.Logger {
	tc := FromContext(ctx)
	if tc == nil {
		return log
	}
	return log.With().
		Str("trace_id", tc.TraceID).
		Str("span_id", tc.SpanID).
		Str("request_id", tc.RequestID).
		Logger()
}

// LogEvent adds trace context to a zerolog event.
func LogEvent(ctx context.Context, event *zerolog.Event) *zerolog.Event {
	tc := FromContext(ctx)
	if tc == nil {
		return event
	}
	return event.
		Str("trace_id", tc.TraceID).
		Str("span_id", tc.SpanID).
		Str("request_id", tc.RequestID)
}

// generateID generates a random hex ID of the specified byte length.
func generateID(byteLen int) string {
	b := make([]byte, byteLen)
	_, _ = rand.Read(b)
	return hex.EncodeToString(b)
}
