package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
)

// ConfigHandler handles configuration-related operations
type ConfigHandler struct {
	repo repository.ConfigRepository
	log  zerolog.Logger
}

// NewConfigHandler creates a new ConfigHandler
func NewConfigHandler(repo repository.ConfigRepository, log zerolog.Logger) *ConfigHandler {
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

// ============================================================================
// Gin Handlers
// ============================================================================

// GetConfigGin returns current configuration (Gin)
func (h *ConfigHandler) GetConfigGin(c *gin.Context) {
	if h.repo == nil {
		ErrorGin(c, 503, ErrConfig("Config not available"))
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	config, err := h.repo.GetAll(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get config")
		ErrorGin(c, 500, ErrConfig("Failed to get config"))
		return
	}

	SuccessGin(c, config)
}

// UpdateConfigGin updates configuration (Gin)
func (h *ConfigHandler) UpdateConfigGin(c *gin.Context) {
	if h.repo == nil {
		ErrorGin(c, 503, ErrConfig("Config not available"))
		return
	}

	var updates map[string]any
	if !BindJSONGin(c, &updates) {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	if err := h.repo.UpdateFromMap(ctx, updates); err != nil {
		h.log.Error().Err(err).Msg("Failed to update config")
		ErrorGin(c, 500, ErrConfig("Failed to update config"))
		return
	}

	config, _ := h.repo.GetAll(ctx)
	SuccessGin(c, config)
}
