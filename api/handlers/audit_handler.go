package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// AuditHandler handles audit log operations
type AuditHandler struct {
	repo repository.AuditRepository
	log  zerolog.Logger
}

// NewAuditHandler creates a new AuditHandler
func NewAuditHandler(repo repository.AuditRepository, log zerolog.Logger) *AuditHandler {
	return &AuditHandler{
		repo: repo,
		log:  log.With().Str("component", "AuditHandler").Logger(),
	}
}

// GetAuditLogsGin returns recent audit log entries
func (h *AuditHandler) GetAuditLogsGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Audit service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, _ := GetPaginationGin(c)
	actionFilter := c.Query("action")

	var logs []*entity.AuditLog
	var err error

	if actionFilter != "" {
		logs, err = h.repo.GetByAction(ctx, entity.AuditAction(actionFilter), limit)
	} else {
		logs, err = h.repo.GetRecent(ctx, limit)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get audit logs")
		DatabaseErrorGin(c, "Failed to get audit logs")
		return
	}

	SuccessGin(c, logs)
}
