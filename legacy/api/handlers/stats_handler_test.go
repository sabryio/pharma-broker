package handlers

import (
	"net/http"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestStatsHandler_GetStats(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	h := NewStatsHandler(&mockStatsRepo{}, log)

	c, w := th.CreateContext("GET", "/api/stats", nil)

	h.GetStatsGin(c)

	th.AssertStatus(w, http.StatusOK)

	var resp struct {
		Data *entity.Stats `json:"data"`
	}
	th.AssertJSONResponse(w, &resp)

	if resp.Data.ActiveOffers != 10 {
		t.Errorf("Expected 10 offers, got %d", resp.Data.ActiveOffers)
	}
}
