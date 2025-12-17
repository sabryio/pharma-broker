package handlers

import (
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

// AnalyzeGin handles manual text analysis with AI
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
