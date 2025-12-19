package handlers

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
)

func TestValidator_Required(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		wantErr bool
	}{
		{"valid", "hello", false},
		{"empty", "", true},
		{"whitespace only", "   ", true},
		{"with spaces", "  hello  ", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().Required("field", tt.value)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Required() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_MinLength(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		min     int
		wantErr bool
	}{
		{"valid", "hello", 3, false},
		{"exact", "abc", 3, false},
		{"too short", "ab", 3, true},
		{"empty", "", 1, true},
		{"unicode", "مرحبا", 3, false}, // Arabic "hello" is 5 chars
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().MinLength("field", tt.value, tt.min)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("MinLength() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_MaxLength(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		max     int
		wantErr bool
	}{
		{"valid", "hello", 10, false},
		{"exact", "abc", 3, false},
		{"too long", "abcd", 3, true},
		{"empty", "", 10, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().MaxLength("field", tt.value, tt.max)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("MaxLength() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_Range(t *testing.T) {
	tests := []struct {
		name    string
		value   int
		min     int
		max     int
		wantErr bool
	}{
		{"in range", 5, 1, 10, false},
		{"at min", 1, 1, 10, false},
		{"at max", 10, 1, 10, false},
		{"below min", 0, 1, 10, true},
		{"above max", 11, 1, 10, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().Range("field", tt.value, tt.min, tt.max)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Range() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_RangeFloat(t *testing.T) {
	tests := []struct {
		name    string
		value   float64
		min     float64
		max     float64
		wantErr bool
	}{
		{"in range", 0.5, 0.0, 1.0, false},
		{"at min", 0.0, 0.0, 1.0, false},
		{"at max", 1.0, 0.0, 1.0, false},
		{"below min", -0.1, 0.0, 1.0, true},
		{"above max", 1.1, 0.0, 1.0, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().RangeFloat("field", tt.value, tt.min, tt.max)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("RangeFloat() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_OneOf(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		allowed []string
		wantErr bool
	}{
		{"valid", "CONFIRMED", []string{"CONFIRMED", "REJECTED"}, false},
		{"invalid", "PENDING", []string{"CONFIRMED", "REJECTED"}, true},
		{"empty skipped", "", []string{"CONFIRMED", "REJECTED"}, false},
		{"case sensitive", "confirmed", []string{"CONFIRMED", "REJECTED"}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().OneOf("field", tt.value, tt.allowed...)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("OneOf() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_Email(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		wantErr bool
	}{
		{"valid", "test@example.com", false},
		{"valid with subdomain", "test@mail.example.com", false},
		{"invalid no @", "testexample.com", true},
		{"invalid no domain", "test@", true},
		{"empty skipped", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().Email("field", tt.value)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Email() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_Phone(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		wantErr bool
	}{
		{"valid international", "+1234567890", false},
		{"valid with spaces", "+1 234 567 890", false},
		{"valid with dashes", "123-456-7890", false},
		{"too short", "12345", true},
		{"invalid chars", "abc123", true},
		{"empty skipped", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().Phone("field", tt.value)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Phone() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_UUID(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		wantErr bool
	}{
		{"valid", "550e8400-e29b-41d4-a716-446655440000", false},
		{"valid uppercase", "550E8400-E29B-41D4-A716-446655440000", false},
		{"invalid format", "550e8400e29b41d4a716446655440000", true},
		{"too short", "550e8400-e29b-41d4", true},
		{"empty skipped", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().UUID("field", tt.value)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("UUID() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_NoHTML(t *testing.T) {
	tests := []struct {
		name    string
		value   string
		wantErr bool
	}{
		{"plain text", "hello world", false},
		{"with script tag", "<script>alert('xss')</script>", true},
		{"with div tag", "<div>content</div>", true},
		{"with angle brackets in text", "5 < 10 and 10 > 5", true}, // This is a limitation
		{"empty skipped", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator().NoHTML("field", tt.value)
			if v.HasErrors() != tt.wantErr {
				t.Errorf("NoHTML() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestValidator_Chaining(t *testing.T) {
	v := NewValidator().
		Required("name", "John").
		MinLength("name", "John", 2).
		MaxLength("name", "John", 100)

	if v.HasErrors() {
		t.Errorf("Chained validation should pass, got errors: %v", v.Errors())
	}

	v2 := NewValidator().
		Required("name", "").
		MinLength("email", "a", 5)

	if !v2.HasErrors() {
		t.Error("Chained validation should fail")
	}
	if len(v2.Errors().Errors) != 2 {
		t.Errorf("Expected 2 errors, got %d", len(v2.Errors().Errors))
	}
}

func TestValidator_ValidateGin(t *testing.T) {
	gin.SetMode(gin.TestMode)

	t.Run("passes validation", func(t *testing.T) {
		w := httptest.NewRecorder()
		c, _ := gin.CreateTestContext(w)

		v := NewValidator().Required("name", "John")
		result := v.ValidateGin(c)

		if !result {
			t.Error("ValidateGin should return true for valid data")
		}
		if w.Code != http.StatusOK {
			t.Errorf("Status should be 200, got %d", w.Code)
		}
	})

	t.Run("fails validation", func(t *testing.T) {
		w := httptest.NewRecorder()
		c, _ := gin.CreateTestContext(w)

		v := NewValidator().Required("name", "")
		result := v.ValidateGin(c)

		if result {
			t.Error("ValidateGin should return false for invalid data")
		}
		if w.Code != http.StatusBadRequest {
			t.Errorf("Status should be 400, got %d", w.Code)
		}
	})
}

func TestFeedbackRequest_Validate(t *testing.T) {
	tests := []struct {
		name    string
		req     FeedbackRequest
		wantErr bool
	}{
		{
			name:    "valid confirmed",
			req:     FeedbackRequest{Decision: "CONFIRMED", OperatorID: "user1"},
			wantErr: false,
		},
		{
			name:    "valid rejected",
			req:     FeedbackRequest{Decision: "REJECTED", Reason: "Price too high"},
			wantErr: false,
		},
		{
			name:    "missing decision",
			req:     FeedbackRequest{Decision: ""},
			wantErr: true,
		},
		{
			name:    "invalid decision",
			req:     FeedbackRequest{Decision: "PENDING"},
			wantErr: true,
		},
		{
			name:    "html in reason",
			req:     FeedbackRequest{Decision: "REJECTED", Reason: "<script>alert('xss')</script>"},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := tt.req.Validate()
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Validate() hasErrors = %v, want %v, errors: %v", v.HasErrors(), tt.wantErr, v.Errors())
			}
		})
	}
}

func TestAnalyzeRequest_Validate(t *testing.T) {
	tests := []struct {
		name    string
		req     AnalyzeRequest
		wantErr bool
	}{
		{
			name:    "valid",
			req:     AnalyzeRequest{Text: "عرض باراسيتامول 500 مج"},
			wantErr: false,
		},
		{
			name:    "missing text",
			req:     AnalyzeRequest{Text: ""},
			wantErr: true,
		},
		{
			name:    "text too short",
			req:     AnalyzeRequest{Text: "ab"},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := tt.req.Validate()
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Validate() hasErrors = %v, want %v", v.HasErrors(), tt.wantErr)
			}
		})
	}
}

func TestManualWeightsRequestDTO_Validate(t *testing.T) {
	tests := []struct {
		name    string
		req     ManualWeightsRequestDTO
		wantErr bool
	}{
		{
			name: "valid weights",
			req: ManualWeightsRequestDTO{
				Weights: WeightsDTO{
					Medication: 0.30,
					Dosage:     0.20,
					Quantity:   0.20,
					Price:      0.15,
					Recency:    0.15,
				},
				Notes: "Adjusted for better matching",
			},
			wantErr: false,
		},
		{
			name: "weights don't sum to 1",
			req: ManualWeightsRequestDTO{
				Weights: WeightsDTO{
					Medication: 0.30,
					Dosage:     0.30,
					Quantity:   0.30,
					Price:      0.30,
					Recency:    0.30,
				},
			},
			wantErr: true,
		},
		{
			name: "weight too low",
			req: ManualWeightsRequestDTO{
				Weights: WeightsDTO{
					Medication: 0.01, // Below 0.05 minimum
					Dosage:     0.30,
					Quantity:   0.30,
					Price:      0.20,
					Recency:    0.19,
				},
			},
			wantErr: true,
		},
		{
			name: "weight too high",
			req: ManualWeightsRequestDTO{
				Weights: WeightsDTO{
					Medication: 0.80, // Above 0.70 maximum
					Dosage:     0.05,
					Quantity:   0.05,
					Price:      0.05,
					Recency:    0.05,
				},
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := tt.req.Validate()
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Validate() hasErrors = %v, want %v, errors: %v", v.HasErrors(), tt.wantErr, v.Errors())
			}
		})
	}
}

func TestLoginRequest_Validate(t *testing.T) {
	tests := []struct {
		name    string
		req     LoginRequest
		wantErr bool
	}{
		{
			name:    "valid",
			req:     LoginRequest{Username: "admin", Password: "password123"},
			wantErr: false,
		},
		{
			name:    "missing username",
			req:     LoginRequest{Username: "", Password: "password123"},
			wantErr: true,
		},
		{
			name:    "missing password",
			req:     LoginRequest{Username: "admin", Password: ""},
			wantErr: true,
		},
		{
			name:    "username too short",
			req:     LoginRequest{Username: "ab", Password: "password123"},
			wantErr: true,
		},
		{
			name:    "password too short",
			req:     LoginRequest{Username: "admin", Password: "12345"},
			wantErr: true,
		},
		{
			name:    "html in username",
			req:     LoginRequest{Username: "<script>", Password: "password123"},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := tt.req.Validate()
			if v.HasErrors() != tt.wantErr {
				t.Errorf("Validate() hasErrors = %v, want %v, errors: %v", v.HasErrors(), tt.wantErr, v.Errors())
			}
		})
	}
}
