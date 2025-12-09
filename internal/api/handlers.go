// Package api contains legacy HTTP handlers.
// Most handlers have been migrated to pharmabroker/api/handlers.
// This file contains handlers not yet migrated, kept for future refactoring.
package api

import (
	"context"
	"encoding/csv"
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/api/handlers"
	"pharmabroker/internal/domain"
)

// LegacyHandlers contains handlers not yet migrated to api/handlers package.
// These are kept for reference and future refactoring.
type LegacyHandlers struct {
	matchRepo   domain.MatchRepository
	log         zerolog.Logger
	analyzeFunc func(text string) (*handlers.AnalyzeResult, error)
}

// NewLegacyHandlers creates legacy handlers for un-migrated endpoints
func NewLegacyHandlers(matchRepo domain.MatchRepository, log zerolog.Logger) *LegacyHandlers {
	return &LegacyHandlers{
		matchRepo: matchRepo,
		log:       log.With().Str("component", "legacy-api").Logger(),
	}
}

// SetAnalyzeFunc sets the function to analyze text with AI
func (h *LegacyHandlers) SetAnalyzeFunc(fn func(text string) (*handlers.AnalyzeResult, error)) {
	h.analyzeFunc = fn
}

// Response helpers
type response struct {
	Success bool      `json:"success"`
	Data    any       `json:"data,omitempty"`
	Error   *APIError `json:"error,omitempty"`
}

func (h *LegacyHandlers) writeJSON(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

func (h *LegacyHandlers) success(w http.ResponseWriter, data interface{}) {
	h.writeJSON(w, http.StatusOK, response{Success: true, Data: data})
}

func (h *LegacyHandlers) error(w http.ResponseWriter, status int, msg string) {
	h.writeJSON(w, status, response{Success: false, Error: ErrBadRequest(msg)})
}

func (h *LegacyHandlers) errorWithCode(w http.ResponseWriter, status int, apiErr *APIError) {
	h.writeJSON(w, status, response{Success: false, Error: apiErr})
}

// ============================================================================
// UN-MIGRATED HANDLERS - Keep for future refactoring
// ============================================================================

// Analyze handles manual text analysis with AI
// TODO: Migrate to api/handlers/analysis_handler.go
func (h *LegacyHandlers) Analyze(w http.ResponseWriter, r *http.Request) {
	if h.analyzeFunc == nil {
		h.error(w, http.StatusServiceUnavailable, "Analyze function not configured")
		return
	}

	var req struct {
		Text       string `json:"text"`
		SourceName string `json:"source_name,omitempty"`
		GroupName  string `json:"group_name,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.error(w, http.StatusBadRequest, "Invalid request body")
		return
	}

	if req.Text == "" {
		h.error(w, http.StatusBadRequest, "Text is required")
		return
	}

	result, err := h.analyzeFunc(req.Text)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to analyze text")
		h.error(w, http.StatusInternalServerError, "Analysis failed: "+err.Error())
		return
	}

	h.success(w, result)
}

// ExportMatchesCSV exports matched pairs to CSV format
// TODO: Migrate to api/handlers/match_handler.go
func (h *LegacyHandlers) ExportMatchesCSV(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 30*time.Second)
	defer cancel()

	// Get status filter (default: all matches)
	statusFilter := r.URL.Query().Get("status")

	// Get all matches with details
	matches, err := h.matchRepo.GetPending(ctx, 1000, 0) // Get all pending first
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get matches for export")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch matches"))
		return
	}

	// Set CSV headers
	w.Header().Set("Content-Type", "text/csv; charset=utf-8")
	w.Header().Set("Content-Disposition", "attachment; filename=matches_export.csv")

	// Write BOM for Excel Arabic support
	w.Write([]byte{0xEF, 0xBB, 0xBF})

	// Create CSV writer
	writer := csv.NewWriter(w)
	defer writer.Flush()

	// Write header row
	headers := []string{
		"Match ID",
		"Status",
		"Score",
		"Offer Medication",
		"Offer Quantity",
		"Offer Price",
		"Offer Expiry",
		"Offer Source",
		"Offer Group",
		"Request Medication",
		"Request Quantity",
		"Request Max Price",
		"Request Source",
		"Request Group",
		"Reasoning",
		"Matched By",
		"Created At",
		"Confirmed At",
	}
	writer.Write(headers)

	// Write data rows
	for _, m := range matches {
		// Apply status filter if specified
		if statusFilter != "" && string(m.Status) != statusFilter {
			continue
		}

		// Format confirmed at
		confirmedAt := ""
		if m.ConfirmedAt != nil {
			confirmedAt = m.ConfirmedAt.Format("2006-01-02 15:04")
		}

		// Build row
		row := []string{
			m.ID,
			string(m.Status),
			formatFloat(m.Score),
			safeString(m.Offer, func(o *domain.Offer) string { return o.Medication }),
			safeFloat(m.Offer, func(o *domain.Offer) float64 { return o.Quantity }),
			safeFloat(m.Offer, func(o *domain.Offer) float64 { return o.Price }),
			safeExpiry(m.Offer),
			safeString(m.Offer, func(o *domain.Offer) string { return o.SourcePhone }),
			safeString(m.Offer, func(o *domain.Offer) string { return o.GroupName }),
			safeString(m.Request, func(r *domain.Request) string { return r.Medication }),
			safeFloatReq(m.Request, func(r *domain.Request) float64 { return r.Quantity }),
			safeFloatReq(m.Request, func(r *domain.Request) float64 { return r.MaxPrice }),
			safeString(m.Request, func(r *domain.Request) string { return r.SourcePhone }),
			safeString(m.Request, func(r *domain.Request) string { return r.GroupName }),
			m.Reasoning,
			m.MatchedBy,
			m.CreatedAt.Format("2006-01-02 15:04"),
			confirmedAt,
		}
		writer.Write(row)
	}

	h.log.Info().Int("count", len(matches)).Msg("Exported matches to CSV")
}

// ============================================================================
// Helper functions for safe field access
// ============================================================================

func formatFloat(f float64) string {
	return strconv.FormatFloat(f, 'f', 2, 64)
}

func safeString[T any](ptr *T, getter func(*T) string) string {
	if ptr == nil {
		return ""
	}
	return getter(ptr)
}

func safeFloat(offer *domain.Offer, getter func(*domain.Offer) float64) string {
	if offer == nil {
		return ""
	}
	return formatFloat(getter(offer))
}

func safeExpiry(offer *domain.Offer) string {
	if offer == nil || offer.ExpiryDate == nil {
		return ""
	}
	return offer.ExpiryDate.Format("2006-01")
}

func safeFloatReq(req *domain.Request, getter func(*domain.Request) float64) string {
	if req == nil {
		return ""
	}
	return formatFloat(getter(req))
}
