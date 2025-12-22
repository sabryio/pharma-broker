package handlers

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
)

// TestHelper wraps common Gin testing functionality
type TestHelper struct {
	t *testing.T
}

// NewTestHelper creates a new TestHelper
func NewTestHelper(t *testing.T) *TestHelper {
	gin.SetMode(gin.TestMode)
	return &TestHelper{t: t}
}

// CreateContext creates a gin.Context and httptest.ResponseRecorder
func (h *TestHelper) CreateContext(method, url string, body interface{}) (*gin.Context, *httptest.ResponseRecorder) {
	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)

	var reqBody io.Reader
	if body != nil {
		jsonBytes, err := json.Marshal(body)
		if err != nil {
			h.t.Fatalf("Failed to marshal request body: %v", err)
		}
		reqBody = bytes.NewBuffer(jsonBytes)
	}

	c.Request = httptest.NewRequest(method, url, reqBody)
	if body != nil {
		c.Request.Header.Set("Content-Type", "application/json")
	}

	return c, w
}

// AssertStatus checks if the response code matches expected
func (h *TestHelper) AssertStatus(w *httptest.ResponseRecorder, expected int) {
	if w.Code != expected {
		h.t.Errorf("Expected status %d, got %d", expected, w.Code)
	}
}

// AssertJSONResponse checks if response body decodes to target and optionally checks specific fields
func (h *TestHelper) AssertJSONResponse(w *httptest.ResponseRecorder, target interface{}) {
	if err := json.NewDecoder(w.Body).Decode(target); err != nil {
		h.t.Fatalf("Failed to decode response: %v", err)
	}
}
