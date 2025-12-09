package api

import (
	"context"
	"encoding/csv"
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"github.com/rs/zerolog"

	apiHandlers "pharmabroker/api/handlers"
	sse "pharmabroker/api/sse"
	"pharmabroker/internal/domain"
)

// Handlers contains all HTTP handlers
type Handlers struct {
	offerRepo       domain.OfferRepository
	requestRepo     domain.RequestRepository
	matchRepo       domain.MatchRepository
	groupRepo       domain.GroupRepository
	statsRepo       domain.StatsRepository
	configRepo      domain.ConfigRepository
	feedbackRepo    domain.FeedbackRepository
	leaderboardRepo domain.LeaderboardRepository
	auditRepo       domain.AuditRepository
	log             zerolog.Logger
	sseHub          *sse.SSEHub
	syncGroups      func() error                              // Function to sync groups from WhatsApp
	analyzeFunc     func(text string) (*AnalyzeResult, error) // Function to analyze text with AI

	// Embedded handlers from api/handlers package for delegation
	offerHandler       *apiHandlers.OfferHandler
	requestHandler     *apiHandlers.RequestHandler
	matchHandler       *apiHandlers.MatchHandler
	groupHandler       *apiHandlers.GroupHandler
	statsHandler       *apiHandlers.StatsHandler
	configHandler      *apiHandlers.ConfigHandler
	feedbackHandler    *apiHandlers.FeedbackHandler
	leaderboardHandler *apiHandlers.LeaderboardHandler
	auditHandler       *apiHandlers.AuditHandler
}

// AnalyzeResult represents AI analysis output
type AnalyzeResult struct {
	Items   []AnalyzeItem `json:"items"`
	RawJSON string        `json:"raw_json,omitempty"`
}

// AnalyzeItem represents a single parsed item
type AnalyzeItem struct {
	Type          string  `json:"type"`
	Medication    string  `json:"medication"`
	MedicationRaw string  `json:"medication_raw"`
	Quantity      int     `json:"quantity"`
	Unit          string  `json:"unit,omitempty"`
	Price         float64 `json:"price,omitempty"`
	MaxPrice      float64 `json:"max_price,omitempty"`
	Currency      string  `json:"currency,omitempty"`
	ExpiryDate    string  `json:"expiry_date,omitempty"`
	BatchNumber   string  `json:"batch_number,omitempty"`
	Urgent        bool    `json:"urgent,omitempty"`
	Notes         string  `json:"notes,omitempty"`
}

// NewHandlers creates new HTTP handlers
func NewHandlers(
	offerRepo domain.OfferRepository,
	requestRepo domain.RequestRepository,
	matchRepo domain.MatchRepository,
	groupRepo domain.GroupRepository,
	statsRepo domain.StatsRepository,
	sseHub *sse.SSEHub,
	log zerolog.Logger,
) *Handlers {
	return &Handlers{
		offerRepo:   offerRepo,
		requestRepo: requestRepo,
		matchRepo:   matchRepo,
		groupRepo:   groupRepo,
		statsRepo:   statsRepo,
		sseHub:      sseHub,
		log:         log.With().Str("component", "api").Logger(),

		// Initialize embedded handlers for delegation
		offerHandler:   apiHandlers.NewOfferHandler(offerRepo, log),
		requestHandler: apiHandlers.NewRequestHandler(requestRepo, log),
		matchHandler:   apiHandlers.NewMatchHandler(matchRepo, offerRepo, requestRepo, nil, sseHub, log),
		groupHandler:   apiHandlers.NewGroupHandler(groupRepo, log),
		statsHandler:   apiHandlers.NewStatsHandler(statsRepo, log),
	}
}

// SetGroupSyncFunc sets the function to sync groups from WhatsApp
func (h *Handlers) SetGroupSyncFunc(fn func() error) {
	h.syncGroups = fn
	// Also propagate to embedded group handler
	if h.groupHandler != nil {
		h.groupHandler.SetSyncFunc(fn)
	}
}

// SetAnalyzeFunc sets the function to analyze text with AI
func (h *Handlers) SetAnalyzeFunc(fn func(text string) (*AnalyzeResult, error)) {
	h.analyzeFunc = fn
}

// SetConfigRepo sets the config repository
func (h *Handlers) SetConfigRepo(repo domain.ConfigRepository) {
	h.configRepo = repo
	// Also create embedded config handler
	h.configHandler = apiHandlers.NewConfigHandler(repo, h.log)
}

// Response helpers
type response struct {
	Success bool      `json:"success"`
	Data    any       `json:"data,omitempty"`
	Error   *APIError `json:"error,omitempty"`
	Meta    *meta     `json:"meta,omitempty"`
}

type meta struct {
	Total  int64 `json:"total,omitempty"`
	Limit  int   `json:"limit,omitempty"`
	Offset int   `json:"offset,omitempty"`
}

func (h *Handlers) writeJSON(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

func (h *Handlers) success(w http.ResponseWriter, data interface{}) {
	h.writeJSON(w, http.StatusOK, response{Success: true, Data: data})
}

func (h *Handlers) error(w http.ResponseWriter, status int, msg string) {
	// Legacy simple error - converts to structured format
	h.errorWithCode(w, status, ErrBadRequest(msg))
}

func (h *Handlers) errorWithCode(w http.ResponseWriter, status int, apiErr *APIError) {
	h.writeJSON(w, status, response{Success: false, Error: apiErr})
}

// GetOffers returns active offers with pagination
func (h *Handlers) GetOffers(w http.ResponseWriter, r *http.Request) {
	h.offerHandler.GetOffers(w, r)
}

// GetOffer returns a single offer by ID
func (h *Handlers) GetOffer(w http.ResponseWriter, r *http.Request) {
	h.offerHandler.GetOffer(w, r)
}

// GetRequests returns active requests with pagination
func (h *Handlers) GetRequests(w http.ResponseWriter, r *http.Request) {
	h.requestHandler.GetRequests(w, r)
}

// GetRequest returns a single request by ID
func (h *Handlers) GetRequest(w http.ResponseWriter, r *http.Request) {
	h.requestHandler.GetRequest(w, r)
}

// GetMatches returns pending matches
func (h *Handlers) GetMatches(w http.ResponseWriter, r *http.Request) {
	h.matchHandler.GetMatches(w, r)
}

// ConfirmMatch confirms a pending match
func (h *Handlers) ConfirmMatch(w http.ResponseWriter, r *http.Request) {
	h.matchHandler.ConfirmMatch(w, r)
}

// RejectMatch rejects a pending match
func (h *Handlers) RejectMatch(w http.ResponseWriter, r *http.Request) {
	h.matchHandler.RejectMatch(w, r)
}

// GetStats returns dashboard statistics
func (h *Handlers) GetStats(w http.ResponseWriter, r *http.Request) {
	h.statsHandler.GetStats(w, r)
}

// GetGroups returns all groups
func (h *Handlers) GetGroups(w http.ResponseWriter, r *http.Request) {
	h.groupHandler.GetGroups(w, r)
}

// SyncGroups fetches groups from WhatsApp and syncs to database
func (h *Handlers) SyncGroups(w http.ResponseWriter, r *http.Request) {
	h.groupHandler.SyncGroups(w, r)
}

// UpdateGroupMonitoring toggles group monitoring
func (h *Handlers) UpdateGroupMonitoring(w http.ResponseWriter, r *http.Request) {
	h.groupHandler.UpdateGroupMonitoring(w, r)
}

// Analyze handles manual text analysis with AI
func (h *Handlers) Analyze(w http.ResponseWriter, r *http.Request) {
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

// GetConfig returns current configuration
func (h *Handlers) GetConfig(w http.ResponseWriter, r *http.Request) {
	if h.configHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Config not available")
		return
	}
	h.configHandler.GetConfig(w, r)
}

// UpdateConfig updates configuration
func (h *Handlers) UpdateConfig(w http.ResponseWriter, r *http.Request) {
	if h.configHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Config not available")
		return
	}
	h.configHandler.UpdateConfig(w, r)
}

// ExportMatchesCSV exports matched pairs to CSV format
func (h *Handlers) ExportMatchesCSV(w http.ResponseWriter, r *http.Request) {
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

// Helper functions for safe field access
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

// SetFeedbackRepo sets the feedback repository
func (h *Handlers) SetFeedbackRepo(repo domain.FeedbackRepository) {
	h.feedbackRepo = repo
	// Also create embedded feedback handler
	h.feedbackHandler = apiHandlers.NewFeedbackHandler(repo, h.matchRepo, h.log)
}

// SetLeaderboardRepo sets the leaderboard repository
func (h *Handlers) SetLeaderboardRepo(repo domain.LeaderboardRepository) {
	h.leaderboardRepo = repo
	// Also create embedded leaderboard handler
	h.leaderboardHandler = apiHandlers.NewLeaderboardHandler(repo, h.log)
}

// RecordFeedback records operator feedback on a match
func (h *Handlers) RecordFeedback(w http.ResponseWriter, r *http.Request) {
	if h.feedbackHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Feedback service not configured")
		return
	}
	h.feedbackHandler.RecordFeedback(w, r)
}

// GetFeedbackAnalysis returns aggregated feedback statistics
func (h *Handlers) GetFeedbackAnalysis(w http.ResponseWriter, r *http.Request) {
	if h.feedbackHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Feedback service not configured")
		return
	}
	h.feedbackHandler.GetFeedbackAnalysis(w, r)
}

// GetRecentFeedback returns recent feedback entries
func (h *Handlers) GetRecentFeedback(w http.ResponseWriter, r *http.Request) {
	if h.feedbackHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Feedback service not configured")
		return
	}
	h.feedbackHandler.GetRecentFeedback(w, r)
}

// GetDemandLeaderboard returns top medications by demand ratio
func (h *Handlers) GetDemandLeaderboard(w http.ResponseWriter, r *http.Request) {
	if h.leaderboardHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Leaderboard service not configured")
		return
	}
	h.leaderboardHandler.GetDemandLeaderboard(w, r)
}

// GetMedicationDemand returns demand stats for a specific medication
func (h *Handlers) GetMedicationDemand(w http.ResponseWriter, r *http.Request) {
	if h.leaderboardHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Leaderboard service not configured")
		return
	}
	h.leaderboardHandler.GetMedicationDemand(w, r)
}

// RefreshLeaderboard triggers a leaderboard refresh
func (h *Handlers) RefreshLeaderboard(w http.ResponseWriter, r *http.Request) {
	if h.leaderboardHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Leaderboard service not configured")
		return
	}
	h.leaderboardHandler.RefreshLeaderboard(w, r)
}

// SetAuditRepo sets the audit repository
func (h *Handlers) SetAuditRepo(repo domain.AuditRepository) {
	h.auditRepo = repo
	// Also create embedded audit handler
	h.auditHandler = apiHandlers.NewAuditHandler(repo, h.log)
}

// GetAuditLogs returns recent audit log entries
func (h *Handlers) GetAuditLogs(w http.ResponseWriter, r *http.Request) {
	if h.auditHandler == nil {
		h.error(w, http.StatusServiceUnavailable, "Audit service not configured")
		return
	}
	h.auditHandler.GetAuditLogs(w, r)
}
