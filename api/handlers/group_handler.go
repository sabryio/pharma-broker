package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
)

// GroupHandler handles group-related operations
type GroupHandler struct {
	repo     repository.GroupRepository
	syncFunc func() error
	log      zerolog.Logger
}

// NewGroupHandler creates a new GroupHandler
func NewGroupHandler(repo repository.GroupRepository, log zerolog.Logger) *GroupHandler {
	return &GroupHandler{
		repo: repo,
		log:  log.With().Str("component", "GroupHandler").Logger(),
	}
}

// SetSyncFunc sets the group synchronization function
func (h *GroupHandler) SetSyncFunc(fn func() error) {
	h.syncFunc = fn
}

// GetGroups returns all groups
func (h *GroupHandler) GetGroups(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	groups, err := h.repo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get groups")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch groups"))
		return
	}

	success(w, groups)
}

// SyncGroups fetches groups from WhatsApp and syncs to database
func (h *GroupHandler) SyncGroups(w http.ResponseWriter, r *http.Request) {
	if h.syncFunc == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("WhatsApp not connected"))
		return
	}

	if err := h.syncFunc(); err != nil {
		h.log.Error().Err(err).Msg("Failed to sync groups")
		errorWithCode(w, http.StatusInternalServerError, ErrInternal("Failed to sync groups: "+err.Error()))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	groups, err := h.repo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get groups after sync")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Synced but failed to fetch groups"))
		return
	}

	success(w, groups)
}

// UpdateGroupMonitoring toggles group monitoring
func (h *GroupHandler) UpdateGroupMonitoring(w http.ResponseWriter, r *http.Request) {
	jid := r.PathValue("jid")
	if jid == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing group JID"))
		return
	}

	var req struct {
		Monitored bool `json:"monitored"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Invalid request body"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	if err := h.repo.SetMonitored(ctx, jid, req.Monitored); err != nil {
		h.log.Error().Err(err).Msg("Failed to update group")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to update group"))
		return
	}

	success(w, map[string]bool{"monitored": req.Monitored})
}

// ============================================================================
// Gin Handlers
// ============================================================================

// GetGroupsGin returns all groups (Gin)
func (h *GroupHandler) GetGroupsGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	groups, err := h.repo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get groups")
		DatabaseErrorGin(c, "Failed to fetch groups")
		return
	}

	SuccessGin(c, groups)
}

// SyncGroupsGin fetches groups from WhatsApp and syncs to database (Gin)
func (h *GroupHandler) SyncGroupsGin(c *gin.Context) {
	if h.syncFunc == nil {
		InternalErrorGin(c, "WhatsApp not connected")
		return
	}

	if err := h.syncFunc(); err != nil {
		h.log.Error().Err(err).Msg("Failed to sync groups")
		InternalErrorGin(c, "Failed to sync groups: "+err.Error())
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	groups, err := h.repo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get groups after sync")
		DatabaseErrorGin(c, "Synced but failed to fetch groups")
		return
	}

	SuccessGin(c, groups)
}

// UpdateGroupMonitoringGin toggles group monitoring (Gin)
func (h *GroupHandler) UpdateGroupMonitoringGin(c *gin.Context) {
	jid, ok := GetPathIDGin(c, "jid")
	if !ok {
		return
	}

	var req struct {
		Monitored bool `json:"monitored"`
	}
	if !BindJSONGin(c, &req) {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	if err := h.repo.SetMonitored(ctx, jid, req.Monitored); err != nil {
		h.log.Error().Err(err).Msg("Failed to update group")
		DatabaseErrorGin(c, "Failed to update group")
		return
	}

	SuccessGin(c, map[string]bool{"monitored": req.Monitored})
}
