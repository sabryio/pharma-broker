package handlers

import (
	"net/http"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestGroupHandler_GetGroups(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockGroupRepo{groups: []*entity.Group{
		{JID: "group-1@g.us", Name: "Pharmacy Group", Monitored: true},
	}}
	h := NewGroupHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/groups", nil)

	h.GetGroupsGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestGroupHandler_SyncGroups_NotConfigured(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	h := NewGroupHandler(&mockGroupRepo{}, log)
	// syncFunc not set

	c, w := th.CreateContext("POST", "/api/groups/sync", nil)

	h.SyncGroupsGin(c)

	th.AssertStatus(w, http.StatusInternalServerError) // Gin internal error is 500
}
