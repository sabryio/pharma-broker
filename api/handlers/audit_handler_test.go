package handlers

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestAuditHandler_GetAuditLogs(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockAuditRepo{logs: []*entity.AuditLog{
		{ID: "log-1", Action: entity.AuditMatchConfirmed, EntityID: "match-1"},
	}}
	h := NewAuditHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/audit", nil)
	w := httptest.NewRecorder()

	h.GetAuditLogs(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}
