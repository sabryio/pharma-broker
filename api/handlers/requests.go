// Package handlers provides HTTP request handlers.
package handlers

import (
	"github.com/gin-gonic/gin"
)

// ConfirmMatchRequest represents a match confirmation request
type ConfirmMatchRequest struct {
	MatchedBy string `json:"matched_by"`
	Notes     string `json:"notes"`
}

// Validate validates the request
func (r *ConfirmMatchRequest) Validate() *Validator {
	return NewValidator().
		MaxLength("matched_by", r.MatchedBy, 100).
		MaxLength("notes", r.Notes, 1000).
		NoHTML("matched_by", r.MatchedBy).
		NoHTML("notes", r.Notes)
}

// RejectMatchRequest represents a match rejection request
type RejectMatchRequest struct {
	MatchedBy string `json:"matched_by"`
	Reason    string `json:"reason"`
}

// Validate validates the request
func (r *RejectMatchRequest) Validate() *Validator {
	return NewValidator().
		MaxLength("matched_by", r.MatchedBy, 100).
		MaxLength("reason", r.Reason, 1000).
		NoHTML("matched_by", r.MatchedBy).
		NoHTML("reason", r.Reason)
}

// FeedbackRequest represents a feedback submission request
type FeedbackRequest struct {
	Decision   string `json:"decision"`
	Reason     string `json:"reason,omitempty"`
	OperatorID string `json:"operator_id,omitempty"`
}

// Validate validates the request
func (r *FeedbackRequest) Validate() *Validator {
	return NewValidator().
		Required("decision", r.Decision).
		OneOf("decision", r.Decision, "CONFIRMED", "REJECTED").
		MaxLength("reason", r.Reason, 1000).
		MaxLength("operator_id", r.OperatorID, 100).
		NoHTML("reason", r.Reason).
		NoHTML("operator_id", r.OperatorID)
}

// AnalyzeRequest represents a text analysis request
type AnalyzeRequest struct {
	Text       string `json:"text"`
	SourceName string `json:"source_name,omitempty"`
	GroupName  string `json:"group_name,omitempty"`
}

// Validate validates the request
func (r *AnalyzeRequest) Validate() *Validator {
	return NewValidator().
		Required("text", r.Text).
		MinLength("text", r.Text, 3).
		MaxLength("text", r.Text, 10000).
		MaxLength("source_name", r.SourceName, 200).
		MaxLength("group_name", r.GroupName, 200)
}

// UpdateGroupRequest represents a group monitoring update request
type UpdateGroupRequest struct {
	Monitored bool `json:"monitored"`
}

// Validate validates the request (no validation needed for bool)
func (r *UpdateGroupRequest) Validate() *Validator {
	return NewValidator()
}

// UpdateConfigRequest represents a config update request
type UpdateConfigRequest map[string]any

// Validate validates the request
func (r UpdateConfigRequest) Validate() *Validator {
	v := NewValidator()

	// Validate known config keys
	for key, value := range r {
		switch key {
		case "match_threshold":
			if f, ok := value.(float64); ok {
				v.RangeFloat("match_threshold", f, 0.0, 1.0)
			}
		case "admin_phone":
			if s, ok := value.(string); ok {
				v.Phone("admin_phone", s)
			}
		case "batch_interval_seconds":
			if i, ok := value.(float64); ok {
				v.Range("batch_interval_seconds", int(i), 1, 3600)
			}
		case "rate_limit_per_hour":
			if i, ok := value.(float64); ok {
				v.Range("rate_limit_per_hour", int(i), 1, 10000)
			}
		}
		// Allow unknown keys to pass through for flexibility
	}

	return v
}

// ManualWeightsRequestDTO represents a manual weight update request
type ManualWeightsRequestDTO struct {
	Weights WeightsDTO `json:"weights"`
	Notes   string     `json:"notes"`
}

// WeightsDTO represents scoring weights
type WeightsDTO struct {
	Medication float64 `json:"medication"`
	Dosage     float64 `json:"dosage"`
	Quantity   float64 `json:"quantity"`
	Price      float64 `json:"price"`
	Recency    float64 `json:"recency"`
}

// Validate validates the request
func (r *ManualWeightsRequestDTO) Validate() *Validator {
	v := NewValidator()

	// Validate individual weights are in range
	v.RangeFloat("weights.medication", r.Weights.Medication, 0.05, 0.70)
	v.RangeFloat("weights.dosage", r.Weights.Dosage, 0.05, 0.70)
	v.RangeFloat("weights.quantity", r.Weights.Quantity, 0.05, 0.70)
	v.RangeFloat("weights.price", r.Weights.Price, 0.05, 0.70)
	v.RangeFloat("weights.recency", r.Weights.Recency, 0.05, 0.70)

	// Validate weights sum to 1.0
	sum := r.Weights.Medication + r.Weights.Dosage + r.Weights.Quantity +
		r.Weights.Price + r.Weights.Recency
	v.Custom("weights", sum >= 0.99 && sum <= 1.01, "weights must sum to 1.0")

	v.MaxLength("notes", r.Notes, 500)
	v.NoHTML("notes", r.Notes)

	return v
}

// ApplyPendingWeightsRequest represents a request to apply pending weights
type ApplyPendingWeightsRequest struct {
	Confirm bool `json:"confirm"`
}

// Validate validates the request
func (r *ApplyPendingWeightsRequest) Validate() *Validator {
	return NewValidator().
		Custom("confirm", r.Confirm, "must set confirm=true to apply weights")
}

// RefreshTokenRequest represents a token refresh request
type RefreshTokenRequest struct {
	RefreshToken string `json:"refresh_token"`
}

// Validate validates the request
func (r *RefreshTokenRequest) Validate() *Validator {
	return NewValidator().
		Required("refresh_token", r.RefreshToken).
		MinLength("refresh_token", r.RefreshToken, 10)
}

// LoginRequest represents a login request
type LoginRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
}

// Validate validates the request
func (r *LoginRequest) Validate() *Validator {
	return NewValidator().
		Required("username", r.Username).
		Required("password", r.Password).
		MinLength("username", r.Username, 3).
		MaxLength("username", r.Username, 100).
		MinLength("password", r.Password, 6).
		MaxLength("password", r.Password, 100).
		NoHTML("username", r.Username)
}

// PaginationParams represents pagination query parameters
type PaginationParams struct {
	Limit  int
	Offset int
}

// ValidatePagination extracts and validates pagination parameters
func ValidatePagination(c *gin.Context) (PaginationParams, bool) {
	limit, offset := GetPaginationGin(c)

	v := NewValidator().
		Range("limit", limit, 1, 100).
		NonNegative("offset", offset)

	if !v.ValidateGin(c) {
		return PaginationParams{}, false
	}

	return PaginationParams{Limit: limit, Offset: offset}, true
}

// IDParam represents a path ID parameter
type IDParam struct {
	ID string
}

// ValidateID extracts and validates an ID path parameter
func ValidateID(c *gin.Context, param string) (string, bool) {
	id := c.Param(param)

	v := NewValidator().
		Required(param, id).
		MaxLength(param, id, 100).
		SafeString(param, id)

	if !v.ValidateGin(c) {
		return "", false
	}

	return id, true
}

// ValidateUUID extracts and validates a UUID path parameter
func ValidateUUID(c *gin.Context, param string) (string, bool) {
	id := c.Param(param)

	v := NewValidator().
		Required(param, id).
		UUID(param, id)

	if !v.ValidateGin(c) {
		return "", false
	}

	return id, true
}

// BindAndValidate binds JSON and validates the request
// Returns true if successful, false if validation failed (response already sent)
func BindAndValidate[T interface{ Validate() *Validator }](c *gin.Context, req T) bool {
	if err := c.ShouldBindJSON(req); err != nil {
		ErrorGin(c, 400, ErrValidation("Invalid request body: "+err.Error()))
		return false
	}

	return req.Validate().ValidateGin(c)
}

// ExportRequest represents an export request with filters
type ExportRequest struct {
	Status string `json:"status"`
	Format string `json:"format"`
}

// Validate validates the request
func (r *ExportRequest) Validate() *Validator {
	return NewValidator().
		OneOf("status", r.Status, "pending", "confirmed", "rejected", "").
		OneOf("format", r.Format, "csv", "json", "")
}
