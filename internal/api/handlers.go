package api

import (
	"context"
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
)

// Handlers contains all HTTP handlers
type Handlers struct {
	offerRepo   domain.OfferRepository
	requestRepo domain.RequestRepository
	matchRepo   domain.MatchRepository
	groupRepo   domain.GroupRepository
	statsRepo   domain.StatsRepository
	log         zerolog.Logger
	sseHub      *SSEHub
	syncGroups  func() error // Function to sync groups from WhatsApp
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

// Response helpers
type response struct {
	Success bool        `json:"success"`
	Data    interface{} `json:"data,omitempty"`
	Error   string      `json:"error,omitempty"`
	Meta    *meta       `json:"meta,omitempty"`
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
	h.writeJSON(w, status, response{Success: false, Error: msg})
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
		h.error(w, http.StatusInternalServerError, "Failed to fetch offers")
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
		h.error(w, http.StatusNotFound, "Offer not found")
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
		h.error(w, http.StatusInternalServerError, "Failed to fetch requests")
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
		h.error(w, http.StatusNotFound, "Request not found")
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
		h.error(w, http.StatusInternalServerError, "Failed to fetch matches")
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
