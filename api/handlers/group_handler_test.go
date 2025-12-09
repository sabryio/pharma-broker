package handlers

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestGroupHandler_GetGroups(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockGroupRepo{groups: []*entity.Group{
		{JID: "group-1@g.us", Name: "Pharmacy Group", Monitored: true},
	}}
	h := NewGroupHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/groups", nil)
	w := httptest.NewRecorder()

	h.GetGroups(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestGroupHandler_SyncGroups_NotConfigured(t *testing.T) {
	log := zerolog.Nop()
	h := NewGroupHandler(&mockGroupRepo{}, log)
	// syncFunc not set

	req := httptest.NewRequest(http.MethodPost, "/api/groups/sync", nil)
	w := httptest.NewRecorder()

	h.SyncGroups(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Expected status 503, got %d", w.Code)
	}
}
