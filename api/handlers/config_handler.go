package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"pharmabroker/internal/domain"
	"time"

	"github.com/rs/zerolog"
)

// ConfigHandler handles configuration-related operations
type ConfigHandler struct {
	repo domain.ConfigRepository
	log  zerolog.Logger
}

// NewConfigHandler creates a new ConfigHandler
func NewConfigHandler(repo domain.ConfigRepository, log zerolog.Logger) *ConfigHandler {
	return &ConfigHandler{
		repo: repo,
		log:  log.With().Str("component", "ConfigHandler").Logger(),
	}
}

// GetConfig returns current configuration
func (h *ConfigHandler) GetConfig(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrConfig("Config not available"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	config, err := h.repo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get config")
		errorWithCode(w, http.StatusInternalServerError, ErrConfig("Failed to get config"))
		return
	}

	success(w, config)
}

// UpdateConfig updates configuration
func (h *ConfigHandler) UpdateConfig(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrConfig("Config not available"))
		return
	}

	var updates map[string]interface{}
	if err := json.NewDecoder(r.Body).Decode(&updates); err != nil {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Invalid request body"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	if err := h.repo.UpdateFromMap(ctx, updates); err != nil {
		h.log.Error().Err(err).Msg("Failed to update config")
		errorWithCode(w, http.StatusInternalServerError, ErrConfig("Failed to update config"))
		return
	}

	// Return updated config
	config, _ := h.repo.GetAll(ctx)
	success(w, config)
}
