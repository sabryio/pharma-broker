package handlers

import (
	"context"
	"encoding/csv"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/api/sse"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// MatchHandler handles match-related operations
type MatchHandler struct {
	matchRepo   repository.MatchRepository
	offerRepo   repository.OfferRepository
	requestRepo repository.RequestRepository
	auditRepo   repository.AuditRepository
	sseHub      *sse.SSEHub
	log         zerolog.Logger
}

// NewMatchHandler creates a new MatchHandler
func NewMatchHandler(
	matchRepo repository.MatchRepository,
	offerRepo repository.OfferRepository,
	requestRepo repository.RequestRepository,
	auditRepo repository.AuditRepository,
	sseHub *sse.SSEHub,
	log zerolog.Logger,
) *MatchHandler {
	return &MatchHandler{
		matchRepo:   matchRepo,
		offerRepo:   offerRepo,
		requestRepo: requestRepo,
		auditRepo:   auditRepo,
		sseHub:      sseHub,
		log:         log.With().Str("component", "MatchHandler").Logger(),
	}
}

func (h *MatchHandler) logAudit(ctx context.Context, action entity.AuditAction, entityID, details string) {
	if h.auditRepo != nil {
		if err := h.auditRepo.Log(ctx, action, entityID, details); err != nil {
			h.log.Warn().Err(err).Str("action", string(action)).Msg("Failed to log audit event")
		}
	}
}

// GetMatchesGin returns pending matches with pagination
func (h *MatchHandler) GetMatchesGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, offset := GetPaginationGin(c)

	matches, err := h.matchRepo.GetPending(ctx, limit, offset)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get matches")
		DatabaseErrorGin(c, "Failed to fetch matches")
		return
	}

	total, _ := h.matchRepo.CountPending(ctx)
	SuccessWithMetaGin(c, matches, &Meta{Total: total, Limit: limit, Offset: offset})
}

// ConfirmMatchGin confirms a pending match
func (h *MatchHandler) ConfirmMatchGin(c *gin.Context) {
	id, ok := ValidateID(c, "id")
	if !ok {
		return
	}

	var req ConfirmMatchRequest
	if !BindAndValidate(c, &req) {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	match, err := h.matchRepo.GetByID(ctx, id)
	if err != nil {
		NotFoundGin(c, ErrMatchNotFound())
		return
	}

	if err := h.matchRepo.UpdateStatus(ctx, id, entity.MatchStatusConfirmed, req.MatchedBy); err != nil {
		h.log.Error().Err(err).Msg("Failed to confirm match")
		InternalErrorGin(c, "Failed to confirm match")
		return
	}

	h.offerRepo.UpdateStatus(ctx, match.OfferID, entity.StatusMatched)
	h.requestRepo.UpdateStatus(ctx, match.RequestID, entity.StatusMatched)

	if h.sseHub != nil {
		h.sseHub.Broadcast(sse.SSEEvent{
			Type: "match_confirmed",
			Data: map[string]string{"match_id": id, "offer_id": match.OfferID, "request_id": match.RequestID},
		})
	}

	h.logAudit(ctx, entity.AuditMatchConfirmed, id, "Offer: "+match.OfferID+", Request: "+match.RequestID)

	SuccessGin(c, map[string]string{"status": "confirmed"})
}

// RejectMatchGin rejects a pending match
func (h *MatchHandler) RejectMatchGin(c *gin.Context) {
	id, ok := ValidateID(c, "id")
	if !ok {
		return
	}

	var req RejectMatchRequest
	// Optional body - don't fail if empty
	c.ShouldBindJSON(&req)
	// Validate if body was provided
	if req.MatchedBy != "" || req.Reason != "" {
		if !req.Validate().ValidateGin(c) {
			return
		}
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	if err := h.matchRepo.UpdateStatus(ctx, id, entity.MatchStatusRejected, req.MatchedBy); err != nil {
		h.log.Error().Err(err).Msg("Failed to reject match")
		InternalErrorGin(c, "Failed to reject match")
		return
	}

	if h.sseHub != nil {
		h.sseHub.Broadcast(sse.SSEEvent{
			Type: "match_rejected",
			Data: map[string]string{"match_id": id},
		})
	}

	h.logAudit(ctx, entity.AuditMatchRejected, id, req.Reason)

	SuccessGin(c, map[string]string{"status": "rejected"})
}

// ExportMatchesCSVGin exports matched pairs to CSV format
func (h *MatchHandler) ExportMatchesCSVGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 30*time.Second)
	defer cancel()

	statusFilter := c.Query("status")

	matches, err := h.matchRepo.GetPending(ctx, 1000, 0)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get matches for export")
		DatabaseErrorGin(c, "Failed to fetch matches")
		return
	}

	c.Header("Content-Type", "text/csv; charset=utf-8")
	c.Header("Content-Disposition", "attachment; filename=matches_export.csv")

	// Write BOM for Excel Arabic support
	c.Writer.Write([]byte{0xEF, 0xBB, 0xBF})

	writer := csv.NewWriter(c.Writer)
	defer writer.Flush()

	headers := []string{
		"Match ID", "Status", "Score", "Offer Medication", "Offer Quantity", "Offer Price",
		"Offer Expiry", "Offer Source", "Offer Group", "Request Medication", "Request Quantity",
		"Request Max Price", "Request Source", "Request Group", "Reasoning", "Matched By",
		"Created At", "Confirmed At",
	}
	writer.Write(headers)

	for _, m := range matches {
		if statusFilter != "" && string(m.Status) != statusFilter {
			continue
		}
		row := []string{m.ID, string(m.Status), "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""}
		writer.Write(row)
	}
}
