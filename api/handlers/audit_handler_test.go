package handlers

import (
	"net/http"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestAuditHandler_GetAuditLogs(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockAuditRepo{logs: []*entity.AuditLog{
		{ID: "log-1", Action: entity.AuditMatchConfirmed, EntityID: "match-1"},
	}}
	h := NewAuditHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/audit", nil)

	h.GetAuditLogsGin(c)

	th.AssertStatus(w, http.StatusOK)
}
