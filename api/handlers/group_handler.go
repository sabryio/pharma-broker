package handlers

import (
	"context"
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

// GetGroupsGin returns all groups
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

// SyncGroupsGin fetches groups from WhatsApp and syncs to database
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
	jid, ok := ValidateID(c, "jid")
	if !ok {
		return
	}

	var req UpdateGroupRequest
	if !BindAndValidate(c, &req) {
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
