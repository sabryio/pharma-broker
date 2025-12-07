package api

import (
	"context"
	"encoding/csv"
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage"
)

// Handlers contains all HTTP handlers
type Handlers struct {
	offerRepo       domain.OfferRepository
	requestRepo     domain.RequestRepository
	matchRepo       domain.MatchRepository
	groupRepo       domain.GroupRepository
	statsRepo       domain.StatsRepository
	configRepo      ConfigRepository
	feedbackRepo    FeedbackRepository
	leaderboardRepo LeaderboardRepository
	auditRepo       AuditRepository
	log             zerolog.Logger
	sseHub          *SSEHub
	syncGroups      func() error                              // Function to sync groups from WhatsApp
	analyzeFunc     func(text string) (*AnalyzeResult, error) // Function to analyze text with AI
}

// ConfigRepository interface for config storage
type ConfigRepository interface {
	GetAll(ctx context.Context) (*storage.AppConfig, error)
	UpdateFromMap(ctx context.Context, updates map[string]interface{}) error
}

// FeedbackRepository interface for feedback storage
type FeedbackRepository interface {
	RecordFeedback(ctx context.Context, fb *domain.MatchFeedback) error
	GetFeedbackByMatch(ctx context.Context, matchID string) ([]*domain.MatchFeedback, error)
	AnalyzeFeedback(ctx context.Context, days int) (*storage.FeedbackAnalysis, error)
	GetRecentFeedback(ctx context.Context, limit int) ([]*domain.MatchFeedback, error)
}

// LeaderboardRepository interface for leaderboard storage
type LeaderboardRepository interface {
	GetTopDemand(ctx context.Context, limit int) ([]*domain.DemandStats, error)
	GetDemandForMedication(ctx context.Context, medication string) (*domain.DemandStats, error)
	RefreshLeaderboard(ctx context.Context) error
}

// AuditRepository interface for audit logging
type AuditRepository interface {
	Log(ctx context.Context, action storage.AuditAction, entityID, details string) error
	LogWithValues(ctx context.Context, action storage.AuditAction, entityID, oldVal, newVal, details string) error
	GetRecent(ctx context.Context, limit int) ([]*storage.AuditLog, error)
	GetByAction(ctx context.Context, action storage.AuditAction, limit int) ([]*storage.AuditLog, error)
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
	sseHub *SSEHub,
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
	}
}

// SetGroupSyncFunc sets the function to sync groups from WhatsApp
func (h *Handlers) SetGroupSyncFunc(fn func() error) {
	h.syncGroups = fn
}

// SetAnalyzeFunc sets the function to analyze text with AI
func (h *Handlers) SetAnalyzeFunc(fn func(text string) (*AnalyzeResult, error)) {
	h.analyzeFunc = fn
}

// SetConfigRepo sets the config repository
func (h *Handlers) SetConfigRepo(repo ConfigRepository) {
	h.configRepo = repo
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

func (h *Handlers) successWithMeta(w http.ResponseWriter, data interface{}, m *meta) {
	h.writeJSON(w, http.StatusOK, response{Success: true, Data: data, Meta: m})
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
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, offset := h.getPagination(r)
	query := r.URL.Query().Get("q")

	var offers []*domain.Offer
	var err error

	if query != "" {
		offers, err = h.offerRepo.Search(ctx, query, limit, offset)
	} else {
		offers, err = h.offerRepo.GetActive(ctx, limit, offset)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get offers")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch offers"))
		return
	}

	total, _ := h.offerRepo.CountActive(ctx)
	h.successWithMeta(w, offers, &meta{Total: total, Limit: limit, Offset: offset})
}

// GetOffer returns a single offer by ID
func (h *Handlers) GetOffer(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		h.error(w, http.StatusBadRequest, "Missing offer ID")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	offer, err := h.offerRepo.GetByID(ctx, id)
	if err != nil {
		h.errorWithCode(w, http.StatusNotFound, ErrOfferNotFound())
		return
	}

	h.success(w, offer)
}

// GetRequests returns active requests with pagination
func (h *Handlers) GetRequests(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, offset := h.getPagination(r)
	query := r.URL.Query().Get("q")

	var requests []*domain.Request
	var err error

	if query != "" {
		requests, err = h.requestRepo.Search(ctx, query, limit, offset)
	} else {
		requests, err = h.requestRepo.GetActive(ctx, limit, offset)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get requests")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch requests"))
		return
	}

	total, _ := h.requestRepo.CountActive(ctx)
	h.successWithMeta(w, requests, &meta{Total: total, Limit: limit, Offset: offset})
}

// GetRequest returns a single request by ID
func (h *Handlers) GetRequest(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		h.error(w, http.StatusBadRequest, "Missing request ID")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	request, err := h.requestRepo.GetByID(ctx, id)
	if err != nil {
		h.errorWithCode(w, http.StatusNotFound, ErrRequestNotFound())
		return
	}

	h.success(w, request)
}

// GetMatches returns pending matches
func (h *Handlers) GetMatches(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, offset := h.getPagination(r)

	matches, err := h.matchRepo.GetPending(ctx, limit, offset)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get matches")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch matches"))
		return
	}

	total, _ := h.matchRepo.CountPending(ctx)
	h.successWithMeta(w, matches, &meta{Total: total, Limit: limit, Offset: offset})
}

// ConfirmMatch confirms a pending match
func (h *Handlers) ConfirmMatch(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		h.error(w, http.StatusBadRequest, "Missing match ID")
		return
	}

	var req struct {
		MatchedBy string `json:"matched_by"`
		Notes     string `json:"notes"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.error(w, http.StatusBadRequest, "Invalid request body")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	// Get the match first
	match, err := h.matchRepo.GetByID(ctx, id)
	if err != nil {
		h.error(w, http.StatusNotFound, "Match not found")
		return
	}

	// Update match status
	if err := h.matchRepo.UpdateStatus(ctx, id, domain.MatchStatusConfirmed, req.MatchedBy); err != nil {
		h.log.Error().Err(err).Msg("Failed to confirm match")
		h.error(w, http.StatusInternalServerError, "Failed to confirm match")
		return
	}

	// Update offer and request status
	h.offerRepo.UpdateStatus(ctx, match.OfferID, domain.StatusMatched)
	h.requestRepo.UpdateStatus(ctx, match.RequestID, domain.StatusMatched)

	// Broadcast update
	h.sseHub.Broadcast(SSEEvent{
		Type: "match_confirmed",
		Data: map[string]string{"match_id": id, "offer_id": match.OfferID, "request_id": match.RequestID},
	})

	// Audit log
	h.logAudit(ctx, storage.AuditMatchConfirmed, id, "Offer: "+match.OfferID+", Request: "+match.RequestID)

	h.success(w, map[string]string{"status": "confirmed"})
}

// RejectMatch rejects a pending match
func (h *Handlers) RejectMatch(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		h.error(w, http.StatusBadRequest, "Missing match ID")
		return
	}

	var req struct {
		MatchedBy string `json:"matched_by"`
	}
	json.NewDecoder(r.Body).Decode(&req)

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	if err := h.matchRepo.UpdateStatus(ctx, id, domain.MatchStatusRejected, req.MatchedBy); err != nil {
		h.log.Error().Err(err).Msg("Failed to reject match")
		h.error(w, http.StatusInternalServerError, "Failed to reject match")
		return
	}

	h.sseHub.Broadcast(SSEEvent{
		Type: "match_rejected",
		Data: map[string]string{"match_id": id},
	})

	// Audit log
	h.logAudit(ctx, storage.AuditMatchRejected, id, "")

	h.success(w, map[string]string{"status": "rejected"})
}

// GetStats returns dashboard statistics
func (h *Handlers) GetStats(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	stats, err := h.statsRepo.GetStats(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get stats")
		h.error(w, http.StatusInternalServerError, "Failed to fetch stats")
		return
	}

	h.success(w, stats)
}

// GetGroups returns all groups
func (h *Handlers) GetGroups(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	groups, err := h.groupRepo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get groups")
		h.error(w, http.StatusInternalServerError, "Failed to fetch groups")
		return
	}

	h.success(w, groups)
}

// SyncGroups fetches groups from WhatsApp and syncs to database
func (h *Handlers) SyncGroups(w http.ResponseWriter, r *http.Request) {
	if h.syncGroups == nil {
		h.error(w, http.StatusServiceUnavailable, "WhatsApp not connected")
		return
	}

	if err := h.syncGroups(); err != nil {
		h.log.Error().Err(err).Msg("Failed to sync groups")
		h.error(w, http.StatusInternalServerError, "Failed to sync groups: "+err.Error())
		return
	}

	// Return updated groups list
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	groups, err := h.groupRepo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get groups after sync")
		h.error(w, http.StatusInternalServerError, "Synced but failed to fetch groups")
		return
	}

	h.success(w, groups)
}

// UpdateGroupMonitoring toggles group monitoring
func (h *Handlers) UpdateGroupMonitoring(w http.ResponseWriter, r *http.Request) {
	jid := r.PathValue("jid")
	if jid == "" {
		h.error(w, http.StatusBadRequest, "Missing group JID")
		return
	}

	var req struct {
		Monitored bool `json:"monitored"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.error(w, http.StatusBadRequest, "Invalid request body")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	if err := h.groupRepo.SetMonitored(ctx, jid, req.Monitored); err != nil {
		h.log.Error().Err(err).Msg("Failed to update group")
		h.error(w, http.StatusInternalServerError, "Failed to update group")
		return
	}

	h.success(w, map[string]bool{"monitored": req.Monitored})
}

func (h *Handlers) getPagination(r *http.Request) (limit, offset int) {
	limit = 50
	offset = 0

	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 && parsed <= 100 {
			limit = parsed
		}
	}

	if o := r.URL.Query().Get("offset"); o != "" {
		if parsed, err := strconv.Atoi(o); err == nil && parsed >= 0 {
			offset = parsed
		}
	}

	return
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
	if h.configRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Config not available")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	config, err := h.configRepo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get config")
		h.error(w, http.StatusInternalServerError, "Failed to get config")
		return
	}

	h.success(w, config)
}

// UpdateConfig updates configuration
func (h *Handlers) UpdateConfig(w http.ResponseWriter, r *http.Request) {
	if h.configRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Config not available")
		return
	}

	var updates map[string]interface{}
	if err := json.NewDecoder(r.Body).Decode(&updates); err != nil {
		h.error(w, http.StatusBadRequest, "Invalid request body")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	if err := h.configRepo.UpdateFromMap(ctx, updates); err != nil {
		h.log.Error().Err(err).Msg("Failed to update config")
		h.error(w, http.StatusInternalServerError, "Failed to update config")
		return
	}

	// Return updated config
	config, _ := h.configRepo.GetAll(ctx)
	h.success(w, config)
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
func (h *Handlers) SetFeedbackRepo(repo FeedbackRepository) {
	h.feedbackRepo = repo
}

// SetLeaderboardRepo sets the leaderboard repository
func (h *Handlers) SetLeaderboardRepo(repo LeaderboardRepository) {
	h.leaderboardRepo = repo
}

// RecordFeedback records operator feedback on a match
func (h *Handlers) RecordFeedback(w http.ResponseWriter, r *http.Request) {
	if h.feedbackRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Feedback service not configured")
		return
	}

	matchID := r.PathValue("id")
	if matchID == "" {
		h.error(w, http.StatusBadRequest, "Missing match ID")
		return
	}

	var req struct {
		Decision   string `json:"decision"`
		Reason     string `json:"reason,omitempty"`
		OperatorID string `json:"operator_id,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		h.error(w, http.StatusBadRequest, "Invalid request body")
		return
	}

	if req.Decision != "CONFIRMED" && req.Decision != "REJECTED" {
		h.error(w, http.StatusBadRequest, "Decision must be CONFIRMED or REJECTED")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	// Get the match to record original score
	match, err := h.matchRepo.GetByID(ctx, matchID)
	if err != nil {
		h.log.Error().Err(err).Str("match_id", matchID).Msg("Failed to get match for feedback")
		h.error(w, http.StatusNotFound, "Match not found")
		return
	}

	feedback := &domain.MatchFeedback{
		MatchID:            matchID,
		OperatorID:         req.OperatorID,
		Decision:           domain.FeedbackDecision(req.Decision),
		Reason:             req.Reason,
		OriginalScore:      match.Score,
		OriginalConfidence: match.MatchedBy, // This stores the confidence band
	}

	if err := h.feedbackRepo.RecordFeedback(ctx, feedback); err != nil {
		h.log.Error().Err(err).Msg("Failed to record feedback")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to record feedback"))
		return
	}

	h.log.Info().
		Str("match_id", matchID).
		Str("decision", req.Decision).
		Float64("original_score", match.Score).
		Msg("Feedback recorded")

	h.success(w, map[string]interface{}{
		"success":  true,
		"feedback": feedback,
	})
}

// GetFeedbackAnalysis returns aggregated feedback statistics
func (h *Handlers) GetFeedbackAnalysis(w http.ResponseWriter, r *http.Request) {
	if h.feedbackRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Feedback service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	// Default to last 30 days
	days := 30
	if d := r.URL.Query().Get("days"); d != "" {
		if parsed, err := strconv.Atoi(d); err == nil && parsed > 0 {
			days = parsed
		}
	}

	analysis, err := h.feedbackRepo.AnalyzeFeedback(ctx, days)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to analyze feedback")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to analyze feedback"))
		return
	}

	h.success(w, analysis)
}

// GetRecentFeedback returns recent feedback entries
func (h *Handlers) GetRecentFeedback(w http.ResponseWriter, r *http.Request) {
	if h.feedbackRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Feedback service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit := 20
	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 {
			limit = parsed
		}
	}

	feedback, err := h.feedbackRepo.GetRecentFeedback(ctx, limit)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get recent feedback")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get feedback"))
		return
	}

	h.success(w, feedback)
}

// GetDemandLeaderboard returns top medications by demand ratio
func (h *Handlers) GetDemandLeaderboard(w http.ResponseWriter, r *http.Request) {
	if h.leaderboardRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Leaderboard service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit := 20
	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 && parsed <= 100 {
			limit = parsed
		}
	}

	stats, err := h.leaderboardRepo.GetTopDemand(ctx, limit)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get demand leaderboard")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get leaderboard"))
		return
	}

	h.success(w, stats)
}

// GetMedicationDemand returns demand stats for a specific medication
func (h *Handlers) GetMedicationDemand(w http.ResponseWriter, r *http.Request) {
	if h.leaderboardRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Leaderboard service not configured")
		return
	}

	medication := r.PathValue("medication")
	if medication == "" {
		h.error(w, http.StatusBadRequest, "Missing medication name")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	stats, err := h.leaderboardRepo.GetDemandForMedication(ctx, medication)
	if err != nil {
		h.log.Error().Err(err).Str("medication", medication).Msg("Failed to get medication demand")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get demand"))
		return
	}

	h.success(w, stats)
}

// RefreshLeaderboard triggers a leaderboard refresh
func (h *Handlers) RefreshLeaderboard(w http.ResponseWriter, r *http.Request) {
	if h.leaderboardRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Leaderboard service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	if err := h.leaderboardRepo.RefreshLeaderboard(ctx); err != nil {
		h.log.Error().Err(err).Msg("Failed to refresh leaderboard")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to refresh leaderboard"))
		return
	}

	h.success(w, map[string]interface{}{
		"success":      true,
		"refreshed_at": time.Now(),
	})
}

// SetAuditRepo sets the audit repository
func (h *Handlers) SetAuditRepo(repo AuditRepository) {
	h.auditRepo = repo
}

// GetAuditLogs returns recent audit log entries
func (h *Handlers) GetAuditLogs(w http.ResponseWriter, r *http.Request) {
	if h.auditRepo == nil {
		h.error(w, http.StatusServiceUnavailable, "Audit service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit := 50
	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 && parsed <= 200 {
			limit = parsed
		}
	}

	// Filter by action if specified
	actionFilter := r.URL.Query().Get("action")
	var logs []*storage.AuditLog
	var err error

	if actionFilter != "" {
		logs, err = h.auditRepo.GetByAction(ctx, storage.AuditAction(actionFilter), limit)
	} else {
		logs, err = h.auditRepo.GetRecent(ctx, limit)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get audit logs")
		h.errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get audit logs"))
		return
	}

	h.success(w, logs)
}

// logAudit is a helper to safely log audit events
func (h *Handlers) logAudit(ctx context.Context, action storage.AuditAction, entityID, details string) {
	if h.auditRepo != nil {
		if err := h.auditRepo.Log(ctx, action, entityID, details); err != nil {
			h.log.Warn().Err(err).Str("action", string(action)).Msg("Failed to log audit event")
		}
	}
}
