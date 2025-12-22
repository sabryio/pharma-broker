package trace

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/rs/zerolog"
)

func TestNew(t *testing.T) {
	tc := New()

	if len(tc.TraceID) != 32 {
		t.Errorf("TraceID length = %d, want 32", len(tc.TraceID))
	}
	if len(tc.SpanID) != 16 {
		t.Errorf("SpanID length = %d, want 16", len(tc.SpanID))
	}
	if len(tc.RequestID) != 16 {
		t.Errorf("RequestID length = %d, want 16", len(tc.RequestID))
	}
	if !tc.Sampled {
		t.Error("Sampled should be true by default")
	}
}

func TestContext_NewSpan(t *testing.T) {
	parent := New()
	child := parent.NewSpan()

	if child.TraceID != parent.TraceID {
		t.Error("Child should inherit TraceID from parent")
	}
	if child.ParentID != parent.SpanID {
		t.Error("Child ParentID should be parent SpanID")
	}
	if child.SpanID == parent.SpanID {
		t.Error("Child should have different SpanID")
	}
	if child.RequestID != parent.RequestID {
		t.Error("Child should inherit RequestID")
	}
}

func TestWithContext_FromContext(t *testing.T) {
	tc := New()
	ctx := WithContext(context.Background(), tc)

	retrieved := FromContext(ctx)
	if retrieved != tc {
		t.Error("FromContext should return the same trace context")
	}
}

func TestFromContext_NotFound(t *testing.T) {
	ctx := context.Background()
	tc := FromContext(ctx)

	if tc != nil {
		t.Error("FromContext should return nil when not set")
	}
}

func TestFromContextOrNew(t *testing.T) {
	// Without existing context
	ctx := context.Background()
	tc, newCtx := FromContextOrNew(ctx)

	if tc == nil {
		t.Error("Should create new trace context")
	}

	// Verify it's stored in returned context
	retrieved := FromContext(newCtx)
	if retrieved != tc {
		t.Error("New context should contain trace context")
	}

	// With existing context
	existing := New()
	ctx = WithContext(context.Background(), existing)
	tc, _ = FromContextOrNew(ctx)

	if tc != existing {
		t.Error("Should return existing trace context")
	}
}

func TestLogger(t *testing.T) {
	tc := New()
	ctx := WithContext(context.Background(), tc)

	log := zerolog.Nop()
	enriched := Logger(ctx, log)

	// Just verify it doesn't panic - zerolog.Nop() discards output
	enriched.Info().Msg("test")
}

func TestParseTraceParent(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantNil bool
		traceID string
		sampled bool
	}{
		{
			name:    "valid sampled",
			input:   "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
			wantNil: false,
			traceID: "0af7651916cd43dd8448eb211c80319c",
			sampled: true,
		},
		{
			name:    "valid not sampled",
			input:   "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00",
			wantNil: false,
			traceID: "0af7651916cd43dd8448eb211c80319c",
			sampled: false,
		},
		{
			name:    "invalid version",
			input:   "01-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
			wantNil: true,
		},
		{
			name:    "invalid format",
			input:   "invalid",
			wantNil: true,
		},
		{
			name:    "short trace id",
			input:   "00-0af7651916cd43dd-b7ad6b7169203331-01",
			wantNil: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tc := parseTraceParent(tt.input)

			if tt.wantNil {
				if tc != nil {
					t.Error("Expected nil")
				}
				return
			}

			if tc == nil {
				t.Fatal("Expected non-nil")
			}
			if tc.TraceID != tt.traceID {
				t.Errorf("TraceID = %s, want %s", tc.TraceID, tt.traceID)
			}
			if tc.Sampled != tt.sampled {
				t.Errorf("Sampled = %v, want %v", tc.Sampled, tt.sampled)
			}
		})
	}
}

func TestHTTPMiddleware(t *testing.T) {
	handler := HTTPMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		tc := FromContext(r.Context())
		if tc == nil {
			t.Error("Trace context should be set")
		}
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/test", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Header().Get(HeaderXRequestID) == "" {
		t.Error("Response should have X-Request-ID header")
	}
	if rec.Header().Get(HeaderXTraceID) == "" {
		t.Error("Response should have X-Trace-ID header")
	}
}

func TestInjectHeaders(t *testing.T) {
	tc := &Context{
		TraceID:   "0af7651916cd43dd8448eb211c80319c",
		SpanID:    "b7ad6b7169203331",
		RequestID: "req123",
		Sampled:   true,
	}
	ctx := WithContext(context.Background(), tc)

	h := make(http.Header)
	InjectHeaders(ctx, h)

	expected := "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
	if got := h.Get(HeaderTraceParent); got != expected {
		t.Errorf("traceparent = %s, want %s", got, expected)
	}
	if got := h.Get(HeaderXTraceID); got != tc.TraceID {
		t.Errorf("X-Trace-ID = %s, want %s", got, tc.TraceID)
	}
	if got := h.Get(HeaderXRequestID); got != tc.RequestID {
		t.Errorf("X-Request-ID = %s, want %s", got, tc.RequestID)
	}
}
