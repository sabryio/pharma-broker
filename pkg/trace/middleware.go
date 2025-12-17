package trace

import (
	"context"
	"net/http"
	"strings"

	"github.com/gin-gonic/gin"
)

// GinMiddleware creates a Gin middleware that extracts or creates trace context.
func GinMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		// Try to extract trace from headers
		tc := extractFromHeaders(c.Request.Header)
		if tc == nil {
			tc = New()
		}

		// Store in context
		ctx := WithContext(c.Request.Context(), tc)
		c.Request = c.Request.WithContext(ctx)

		// Add trace headers to response
		c.Header(HeaderXRequestID, tc.RequestID)
		c.Header(HeaderXTraceID, tc.TraceID)

		c.Next()
	}
}

// HTTPMiddleware creates a standard http.Handler middleware.
func HTTPMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Try to extract trace from headers
		tc := extractFromHeaders(r.Header)
		if tc == nil {
			tc = New()
		}

		// Store in context
		ctx := WithContext(r.Context(), tc)
		r = r.WithContext(ctx)

		// Add trace headers to response
		w.Header().Set(HeaderXRequestID, tc.RequestID)
		w.Header().Set(HeaderXTraceID, tc.TraceID)

		next.ServeHTTP(w, r)
	})
}

// extractFromHeaders extracts trace context from HTTP headers.
func extractFromHeaders(h http.Header) *Context {
	// Try W3C traceparent header first
	// Format: version-traceid-spanid-flags (e.g., "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01")
	if tp := h.Get(HeaderTraceParent); tp != "" {
		if tc := parseTraceParent(tp); tc != nil {
			tc.RequestID = h.Get(HeaderXRequestID)
			if tc.RequestID == "" {
				tc.RequestID = generateID(8)
			}
			return tc
		}
	}

	// Fall back to legacy headers
	traceID := h.Get(HeaderXTraceID)
	requestID := h.Get(HeaderXRequestID)

	if traceID != "" || requestID != "" {
		tc := New()
		if traceID != "" {
			tc.TraceID = traceID
		}
		if requestID != "" {
			tc.RequestID = requestID
		}
		return tc
	}

	return nil
}

// parseTraceParent parses W3C traceparent header.
func parseTraceParent(tp string) *Context {
	parts := strings.Split(tp, "-")
	if len(parts) != 4 {
		return nil
	}

	// version-traceid-spanid-flags
	version := parts[0]
	traceID := parts[1]
	spanID := parts[2]
	flags := parts[3]

	// Validate version (only "00" supported)
	if version != "00" {
		return nil
	}

	// Validate lengths
	if len(traceID) != 32 || len(spanID) != 16 {
		return nil
	}

	sampled := flags == "01"

	return &Context{
		TraceID:  traceID,
		ParentID: spanID,
		SpanID:   generateID(8), // Generate new span ID
		Sampled:  sampled,
	}
}

// InjectHeaders adds trace context to outgoing HTTP request headers.
func InjectHeaders(ctx context.Context, h http.Header) {
	tc := FromContext(ctx)
	if tc == nil {
		return
	}

	// W3C traceparent
	flags := "00"
	if tc.Sampled {
		flags = "01"
	}
	h.Set(HeaderTraceParent, "00-"+tc.TraceID+"-"+tc.SpanID+"-"+flags)

	// Legacy headers
	h.Set(HeaderXTraceID, tc.TraceID)
	h.Set(HeaderXRequestID, tc.RequestID)
}
