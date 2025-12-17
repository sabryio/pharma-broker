package handlers

import (
	"net/http"
	"testing"

	"github.com/rs/zerolog"
)

func TestConfigHandler_GetConfig(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	h := NewConfigHandler(&mockConfigRepo{}, log)

	c, w := th.CreateContext("GET", "/api/config", nil)

	h.GetConfigGin(c)

	th.AssertStatus(w, http.StatusOK)
}
