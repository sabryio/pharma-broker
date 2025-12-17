package parsing

import (
	"context"
	"strings"
	"sync"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/matching"

	"pharmabroker/pkg/matcher/similarity"
	"pharmabroker/pkg/metrics"

	strUtils "pharmabroker/pkg/text"

	"github.com/google/uuid"
	"github.com/rs/zerolog"
)

// MatchingService handles matching logic
type MatchingService struct {
	offerRepo       repository.OfferRepository
	requestRepo     repository.RequestRepository
	matchRepo       repository.MatchRepository
	matchQueueRepo  repository.MatchQueueRepository
	scorer          *matching.Scorer
	embeddings      *EmbeddingCache
	sseBroadcaster  SSEBroadcaster
	matchFilter     *MatchFilter
	autoAction      *AutoActionHandler
	smoothThreshold *SmoothThresholdCalculator
	calibrator      *ConfidenceCalibrator
	auditTrail      *AuditTrail
	log             zerolog.Logger
}

// NewMatchingService creates a new matching service
func NewMatchingService(
	offerRepo repository.OfferRepository,
	requestRepo repository.RequestRepository,
	matchRepo repository.MatchRepository,
	matchQueueRepo repository.MatchQueueRepository,
	scorer *matching.Scorer,
	embeddings *EmbeddingCache,
	sseBroadcaster SSEBroadcaster,
	log zerolog.Logger,
) *MatchingService {
	// Initialize match filter with default config
	matchFilter := NewMatchFilter(DefaultMatchFilterConfig(), log)

	// Initialize auto-action handler with default config
	autoAction := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	// Initialize smooth threshold calculator
	smoothThreshold := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Initialize confidence calibrator
	calibrator := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Initialize audit trail with in-memory logger
	auditLogger := NewMemoryAuditLogger(10000, log)
	auditTrail := NewAuditTrail(DefaultAuditTrailConfig(), auditLogger, log)

	return &MatchingService{
		offerRepo:       offerRepo,
		requestRepo:     requestRepo,
		matchRepo:       matchRepo,
		matchQueueRepo:  matchQueueRepo,
		scorer:          scorer,
		embeddings:      embeddings,
		sseBroadcaster:  sseBroadcaster,
		matchFilter:     matchFilter,
		autoAction:      autoAction,
		smoothThreshold: smoothThreshold,
		calibrator:      calibrator,
		auditTrail:      auditTrail,
		log:             log,
	}
}

// FindMatchesForOffer finds matching requests for a given offer
func (ms *MatchingService) FindMatchesForOffer(ctx context.Context, offer *entity.Offer) {
	// Search for matching requests by medication name (candidate generation)
	query := sanitizeForFTS(offer.Medication)
	if query == "" {
		return
	}

	requests, err := ms.requestRepo.Search(ctx, query, 10, 0)
	if err != nil {
		ms.log.Error().Err(err).Msg("Failed to search for matching requests")
		return
	}

	// Filter candidates (stale offers, same-sender exclusion)
	if ms.matchFilter != nil {
		requests = ms.matchFilter.FilterRequests(requests, offer)
	}

	// Process matches in parallel with bounded concurrency
	ms.processMatchesParallel(ctx, offer, requests, nil)
}

// FindMatchesForRequest finds matching offers for a given request
func (ms *MatchingService) FindMatchesForRequest(ctx context.Context, request *entity.Request) {
	// Search for matching offers by medication name (candidate generation)
	query := sanitizeForFTS(request.Medication)
	if query == "" {
		return
	}

	offers, err := ms.offerRepo.Search(ctx, query, 10, 0)
	if err != nil {
		ms.log.Error().Err(err).Msg("Failed to search for matching offers")
		return
	}

	// Filter candidates (stale offers, same-sender exclusion)
	if ms.matchFilter != nil {
		offers = ms.matchFilter.FilterOffers(offers, request)
	}

	// Process matches in parallel with bounded concurrency
	ms.processMatchesParallel(ctx, nil, nil, &matchContext{offers: offers, request: request})
}

// matchContext holds context for parallel offer matching
type matchContext struct {
	offers  []*entity.Offer
	request *entity.Request
}

// processMatchesParallel scores candidates in parallel with bounded concurrency
func (ms *MatchingService) processMatchesParallel(ctx context.Context, offer *entity.Offer, requests []*entity.Request, offerCtx *matchContext) {
	const maxConcurrency = 5 // Limit concurrent scoring goroutines

	var wg sync.WaitGroup
	sem := make(chan struct{}, maxConcurrency)

	if requests != nil {
		// Matching requests for an offer
		for _, req := range requests {
			wg.Add(1)
			go func(r *entity.Request) {
				defer wg.Done()

				select {
				case <-ctx.Done():
					return
				case sem <- struct{}{}:
				}
				defer func() { <-sem }()

				ms.processMatch(ctx, offer, r, nil)
			}(req)
		}
	} else if offerCtx != nil {
		// Matching offers for a request
		for _, o := range offerCtx.offers {
			wg.Add(1)
			go func(offer *entity.Offer) {
				defer wg.Done()

				select {
				case <-ctx.Done():
					return
				case sem <- struct{}{}:
				}
				defer func() { <-sem }()

				ms.processMatch(ctx, offer, nil, offerCtx.request)
			}(o)
		}
	}

	wg.Wait()
}

func (ms *MatchingService) processMatch(ctx context.Context, offer *entity.Offer, req *entity.Request, existingReq *entity.Request) {
	// Determine which object is new/being processed
	var request *entity.Request
	if req != nil {
		request = req
	} else {
		request = existingReq
	}

	// Calculate multi-field medication score
	medicationScore := ms.calculateMedicationScore(offer.Medication, request.Medication)

	// Get full match score with breakdown
	matchScore := ms.scorer.ScoreMatch(offer, request, medicationScore)

	// Apply confidence calibration if enabled
	if ms.calibrator != nil {
		rawScore := matchScore.Total
		matchScore.Total = ms.calibrator.Calibrate(rawScore)
		if rawScore != matchScore.Total {
			ms.log.Debug().
				Float64("raw_score", rawScore).
				Float64("calibrated_score", matchScore.Total).
				Msg("📊 Applied confidence calibration")
		}
	}

	// Record scoring metrics
	ms.recordScoreMetrics(matchScore)

	// Use auto-action handler to determine what to do with this match
	var actionResult MatchActionResult
	if ms.autoAction != nil {
		actionResult = ms.autoAction.DetermineAction(matchScore)
	} else {
		// Fallback: basic logic if no auto-action handler
		if matchScore.Confidence == matching.ConfidenceNone {
			return
		}
		actionResult = MatchActionResult{
			Action:     ActionReview,
			Status:     entity.MatchStatusPending,
			ShouldSave: true,
		}
	}

	// Skip if action says to ignore
	if !actionResult.ShouldSave {
		return
	}

	match := &entity.Match{
		ID:        uuid.New().String(),
		OfferID:   offer.ID,
		RequestID: request.ID,
		Score:     matchScore.Total,
		Status:    actionResult.Status,
		MatchedBy: string(matchScore.Confidence),
		Reasoning: matchScore.Breakdown,
		CreatedAt: time.Now(),
	}

	// Process auto-action (sets status, timestamps, sends notifications)
	if ms.autoAction != nil {
		ms.autoAction.ProcessMatchAction(ctx, match, offer, request, actionResult)
	}

	// Save match
	if err := ms.matchRepo.Save(ctx, match); err != nil {
		// Might fail on duplicate, that's ok
		if !strings.Contains(err.Error(), "UNIQUE constraint") {
			ms.log.Error().Err(err).Msg("Failed to save match")
		}
	} else {
		ms.log.Info().
			Str("match_id", match.ID).
			Float64("score", match.Score).
			Str("status", string(match.Status)).
			Str("action", string(actionResult.Action)).
			Str("confidence", string(matchScore.Confidence)).
			Str("breakdown", match.Reasoning).
			Msg("✅ Created match with auto-action")

		// Log to audit trail for compliance
		if ms.auditTrail != nil {
			reason := "Score: " + matchScore.Breakdown
			_ = ms.auditTrail.LogMatchAction(ctx, match, offer, request, actionResult.Action, reason)
		}

		// Notify via SSE if real-time
		if ms.sseBroadcaster != nil {
			ms.sseBroadcaster.BroadcastNewMatch(match.ID, match.Score)
		}
	}
}

// calculateMedicationScore computes a hybrid lexical + semantic medication match score
func (ms *MatchingService) calculateMedicationScore(offerMed, requestMed string) float64 {
	// Step 1: Check if medications are synonyms (instant 1.0 match)
	if ms.embeddings != nil && ms.embeddings.AreSynonyms(offerMed, requestMed) {
		return 1.0 // Perfect match via synonym lookup
	}

	// Step 2: Lexical scoring using fuzzy match
	lexicalScore := fuzzyMatch(offerMed, requestMed)

	// Apply scaling to fuzzy match score to fit 0-1 range better
	normalizedLexical := 0.0
	if lexicalScore >= 1.0 {
		normalizedLexical = 1.0 // Exact match
	} else if lexicalScore >= 0.8 {
		normalizedLexical = 0.85 // High similarity
	} else if lexicalScore >= 0.6 {
		normalizedLexical = 0.6 // Moderate similarity
	} else if strUtils.ContainsIgnoreCase(offerMed, requestMed) ||
		strUtils.ContainsIgnoreCase(requestMed, offerMed) {
		normalizedLexical = 0.5 // Substring match fallback
	} else {
		normalizedLexical = lexicalScore * 0.5 // Low similarity
	}

	// Step 3: Semantic scoring using embeddings
	var semanticScore float64
	if ms.embeddings != nil {
		vecA, okA := ms.embeddings.GetEmbedding(offerMed)
		vecB, okB := ms.embeddings.GetEmbedding(requestMed)
		if okA && okB {
			cosineComparator := similarity.CosineComparator{}
			semanticScore, err := cosineComparator.Similarity(vecA, vecB)
			if semanticScore < 0 {
				ms.log.Error().Err(err).Msg("Failed to calculate semantic score")
				semanticScore = 0.0
			}
		}
	}

	// Hybrid: Weight semantic if available, otherwise use lexical only
	alpha := ms.scorer.GetSemanticWeight() // Default 0.6
	if semanticScore > 0 {
		// Both available: weighted average
		return alpha*semanticScore + (1-alpha)*normalizedLexical
	}

	// Semantic not available: use lexical only
	return normalizedLexical
}

// recordScoreMetrics records match scoring metrics for observability
func (ms *MatchingService) recordScoreMetrics(score *matching.MatchScore) {
	// Record overall score distribution
	metrics.MatchScoreDistribution.Observe(score.Total)

	// Record by confidence band
	metrics.MatchesByConfidenceBand.WithLabelValues(string(score.Confidence)).Inc()

	// Record component score breakdowns
	metrics.MatchScoreMedication.Observe(score.MedicationScore)
	metrics.MatchScoreDosage.Observe(score.DosageScore)
	metrics.MatchScoreQuantity.Observe(score.QuantityScore)
	metrics.MatchScorePrice.Observe(score.PriceScore)
	metrics.MatchScoreRecency.Observe(score.RecencyScore)
}

// =============================================================================
// Match Filter Configuration Methods
// =============================================================================

// GetMatchFilterStats returns the current match filter statistics.
func (ms *MatchingService) GetMatchFilterStats() map[string]int64 {
	if ms.matchFilter == nil {
		return nil
	}
	return ms.matchFilter.GetStats()
}

// GetMatchFilterConfig returns the current match filter configuration.
func (ms *MatchingService) GetMatchFilterConfig() MatchFilterConfig {
	if ms.matchFilter == nil {
		return MatchFilterConfig{}
	}
	return ms.matchFilter.GetConfig()
}

// SetMatchFilterConfig updates the match filter configuration.
func (ms *MatchingService) SetMatchFilterConfig(cfg MatchFilterConfig) {
	if ms.matchFilter != nil {
		ms.matchFilter.SetConfig(cfg)
	}
}

// SetMaxOfferAge sets the maximum offer age for stale filtering.
func (ms *MatchingService) SetMaxOfferAge(age time.Duration) {
	if ms.matchFilter != nil {
		ms.matchFilter.SetMaxOfferAge(age)
	}
}

// EnableStaleFilter enables or disables stale offer filtering.
func (ms *MatchingService) EnableStaleFilter(enabled bool) {
	if ms.matchFilter != nil {
		ms.matchFilter.EnableStaleFilter(enabled)
	}
}

// EnableSameSenderExclusion enables or disables same-sender exclusion.
func (ms *MatchingService) EnableSameSenderExclusion(enabled bool) {
	if ms.matchFilter != nil {
		ms.matchFilter.EnableSameSenderExclusion(enabled)
	}
}

// =============================================================================
// Auto-Action Configuration Methods
// =============================================================================

// GetAutoActionStats returns the current auto-action statistics.
func (ms *MatchingService) GetAutoActionStats() map[string]int64 {
	if ms.autoAction == nil {
		return nil
	}
	return ms.autoAction.GetStats()
}

// GetAutoActionConfig returns the current auto-action configuration.
func (ms *MatchingService) GetAutoActionConfig() AutoActionConfig {
	if ms.autoAction == nil {
		return AutoActionConfig{}
	}
	return ms.autoAction.GetConfig()
}

// SetAutoActionConfig updates the auto-action configuration.
func (ms *MatchingService) SetAutoActionConfig(cfg AutoActionConfig) {
	if ms.autoAction != nil {
		ms.autoAction.SetConfig(cfg)
	}
}

// EnableAutoConfirm enables or disables auto-confirmation.
func (ms *MatchingService) EnableAutoConfirm(enabled bool) {
	if ms.autoAction != nil {
		ms.autoAction.EnableAutoConfirm(enabled)
	}
}

// SetMinScoreForAutoConfirm sets the minimum score for auto-confirmation.
func (ms *MatchingService) SetMinScoreForAutoConfirm(score float64) {
	if ms.autoAction != nil {
		ms.autoAction.SetMinScoreForAutoConfirm(score)
	}
}

// SetMatchNotifier sets the notifier for match notifications.
func (ms *MatchingService) SetMatchNotifier(notifier MatchNotifier) {
	if ms.autoAction != nil {
		ms.autoAction.SetNotifier(notifier)
	}
}

// =============================================================================
// Smooth Threshold Configuration Methods
// =============================================================================

// GetSmoothConfidence calculates smooth confidence for a score.
func (ms *MatchingService) GetSmoothConfidence(score float64) SmoothConfidenceResult {
	if ms.smoothThreshold == nil {
		return SmoothConfidenceResult{
			RawScore:         score,
			SmoothConfidence: score,
			BandStrength:     1.0,
		}
	}
	return ms.smoothThreshold.CalculateSmoothConfidence(score)
}

// GetAdjustedAction returns adjusted action based on smooth confidence.
func (ms *MatchingService) GetAdjustedAction(score float64) AdjustedActionResult {
	if ms.smoothThreshold == nil {
		return AdjustedActionResult{
			RawScore:         score,
			SmoothConfidence: score,
			BandStrength:     1.0,
		}
	}
	return ms.smoothThreshold.GetAdjustedAction(score)
}

// GetSmoothThresholdConfig returns the smooth threshold configuration.
func (ms *MatchingService) GetSmoothThresholdConfig() SmoothThresholdConfig {
	if ms.smoothThreshold == nil {
		return SmoothThresholdConfig{}
	}
	return ms.smoothThreshold.GetConfig()
}

// SetSmoothThresholdConfig updates the smooth threshold configuration.
func (ms *MatchingService) SetSmoothThresholdConfig(cfg SmoothThresholdConfig) {
	if ms.smoothThreshold != nil {
		ms.smoothThreshold.SetConfig(cfg)
	}
}

// SetTransitionWidth sets the transition zone width.
func (ms *MatchingService) SetTransitionWidth(width float64) {
	if ms.smoothThreshold != nil {
		ms.smoothThreshold.SetTransitionWidth(width)
	}
}

// EnableSmoothTransitions enables or disables smooth transitions.
func (ms *MatchingService) EnableSmoothTransitions(enabled bool) {
	if ms.smoothThreshold != nil {
		ms.smoothThreshold.EnableSmoothing(enabled)
	}
}

// =============================================================================
// Confidence Calibration Configuration Methods
// =============================================================================

// GetCalibrationStats returns the current calibration statistics.
func (ms *MatchingService) GetCalibrationStats() map[string]int64 {
	if ms.calibrator == nil {
		return nil
	}
	return ms.calibrator.GetStats()
}

// GetCalibrationConfig returns the current calibration configuration.
func (ms *MatchingService) GetCalibrationConfig() CalibrationConfig {
	if ms.calibrator == nil {
		return CalibrationConfig{}
	}
	return ms.calibrator.GetConfig()
}

// SetCalibrationConfig updates the calibration configuration.
func (ms *MatchingService) SetCalibrationConfig(cfg CalibrationConfig) {
	if ms.calibrator != nil {
		ms.calibrator.SetConfig(cfg)
	}
}

// EnableCalibration enables or disables confidence calibration.
func (ms *MatchingService) EnableCalibration(enabled bool) {
	if ms.calibrator != nil {
		ms.calibrator.Enable(enabled)
	}
}

// SetCalibrationSmoothingFactor sets the smoothing factor for calibration.
func (ms *MatchingService) SetCalibrationSmoothingFactor(factor float64) {
	if ms.calibrator != nil {
		ms.calibrator.SetSmoothingFactor(factor)
	}
}

// GetCalibrationReport returns a detailed calibration report.
func (ms *MatchingService) GetCalibrationReport() CalibrationReport {
	if ms.calibrator == nil {
		return CalibrationReport{}
	}
	return ms.calibrator.GetCalibrationReport()
}

// RecordCalibrationOutcome records a prediction-outcome pair for calibration.
func (ms *MatchingService) RecordCalibrationOutcome(predictedConfidence float64, actualPositive bool) {
	if ms.calibrator != nil {
		ms.calibrator.RecordOutcome(predictedConfidence, actualPositive)
	}
}

// ResetCalibration clears all calibration data.
func (ms *MatchingService) ResetCalibration() {
	if ms.calibrator != nil {
		ms.calibrator.Reset()
	}
}

// CalibrateScore calibrates a raw confidence score.
func (ms *MatchingService) CalibrateScore(rawScore float64) float64 {
	if ms.calibrator == nil {
		return rawScore
	}
	return ms.calibrator.Calibrate(rawScore)
}

// =============================================================================
// Audit Trail Configuration Methods
// =============================================================================

// GetAuditTrailConfig returns the current audit trail configuration.
func (ms *MatchingService) GetAuditTrailConfig() AuditTrailConfig {
	if ms.auditTrail == nil {
		return AuditTrailConfig{}
	}
	return ms.auditTrail.GetConfig()
}

// SetAuditTrailConfig updates the audit trail configuration.
func (ms *MatchingService) SetAuditTrailConfig(cfg AuditTrailConfig) {
	if ms.auditTrail != nil {
		ms.auditTrail.SetConfig(cfg)
	}
}

// EnableAuditTrail enables or disables the audit trail.
func (ms *MatchingService) EnableAuditTrail(enabled bool) {
	if ms.auditTrail != nil {
		ms.auditTrail.Enable(enabled)
	}
}

// GetMatchAuditHistory retrieves the audit history for a specific match.
func (ms *MatchingService) GetMatchAuditHistory(ctx context.Context, matchID string) ([]AuditEntry, error) {
	if ms.auditTrail == nil {
		return nil, nil
	}
	return ms.auditTrail.GetMatchHistory(ctx, matchID)
}

// GetRecentAuditActions retrieves recent audit entries.
func (ms *MatchingService) GetRecentAuditActions(ctx context.Context, limit int) ([]AuditEntry, error) {
	if ms.auditTrail == nil {
		return nil, nil
	}
	return ms.auditTrail.GetRecentActions(ctx, limit)
}

// LogAuditConfigChange logs a configuration change to the audit trail.
func (ms *MatchingService) LogAuditConfigChange(ctx context.Context, configType string, oldValue, newValue interface{}, actor string) error {
	if ms.auditTrail == nil {
		return nil
	}
	return ms.auditTrail.LogConfigChange(ctx, configType, oldValue, newValue, actor)
}
