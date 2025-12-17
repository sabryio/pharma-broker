package handlers

import (
	"encoding/json"
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"
)

// AnalysisHandler handles analysis-related operations
type AnalysisHandler struct {
	analyzeFunc func(text string) (*AnalyzeResult, error)
	log         zerolog.Logger
}

// NewAnalysisHandler creates a new AnalysisHandler
func NewAnalysisHandler(log zerolog.Logger) *AnalysisHandler {
	return &AnalysisHandler{
		log: log.With().Str("component", "AnalysisHandler").Logger(),
	}
}

// SetAnalyzeFunc sets the AI analysis function
func (h *AnalysisHandler) SetAnalyzeFunc(fn func(text string) (*AnalyzeResult, error)) {
	h.analyzeFunc = fn
}

// Analyze handles manual text analysis with AI
func (h *AnalysisHandler) Analyze(w http.ResponseWriter, r *http.Request) {
	if h.analyzeFunc == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Analyze function not configured"))
		return
	}

	var req struct {
		Text       string `json:"text"`
		SourceName string `json:"source_name,omitempty"`
		GroupName  string `json:"group_name,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Invalid request body"))
		return
	}

	if req.Text == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Text is required"))
		return
	}

	result, err := h.analyzeFunc(req.Text)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to analyze text")
		errorWithCode(w, http.StatusInternalServerError, ErrAIParse("Analysis failed: "+err.Error()))
		return
	}

	success(w, result)
}

// ============================================================================
// Gin Handlers
// ============================================================================

// AnalyzeGin handles manual text analysis with AI (Gin)
func (h *AnalysisHandler) AnalyzeGin(c *gin.Context) {
	if h.analyzeFunc == nil {
		InternalErrorGin(c, "Analyze function not configured")
		return
	}

	var req struct {
		Text       string `json:"text"`
		SourceName string `json:"source_name,omitempty"`
		GroupName  string `json:"group_name,omitempty"`
	}
	if !BindJSONGin(c, &req) {
		return
	}

	if req.Text == "" {
		BadRequestGin(c, "Text is required")
		return
	}

	result, err := h.analyzeFunc(req.Text)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to analyze text")
		ErrorGin(c, http.StatusInternalServerError, ErrAIParse("Analysis failed: "+err.Error()))
		return
	}

	SuccessGin(c, result)
}
