package parsing

import (
	"context"
	"strings"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/matching"

	"pharmabroker/pkg/matcher/similarity"

	strUtils "pharmabroker/pkg/text"

	"github.com/google/uuid"
	"github.com/rs/zerolog"
)

// MatchingService handles matching logic
type MatchingService struct {
	offerRepo      repository.OfferRepository
	requestRepo    repository.RequestRepository
	matchRepo      repository.MatchRepository
	matchQueueRepo repository.MatchQueueRepository
	scorer         *matching.Scorer
	embeddings     *EmbeddingCache
	sseBroadcaster SSEBroadcaster
	log            zerolog.Logger
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
	return &MatchingService{
		offerRepo:      offerRepo,
		requestRepo:    requestRepo,
		matchRepo:      matchRepo,
		matchQueueRepo: matchQueueRepo,
		scorer:         scorer,
		embeddings:     embeddings,
		sseBroadcaster: sseBroadcaster,
		log:            log,
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

	for _, req := range requests {
		ms.processMatch(ctx, offer, req, nil)
	}
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

	for _, offer := range offers {
		ms.processMatch(ctx, offer, nil, request)
	}
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

	// Skip matches below minimum threshold (NONE confidence)
	if matchScore.Confidence == matching.ConfidenceNone {
		return
	}

	// Determine match status based on confidence band
	status := entity.MatchStatusPending
	if matchScore.Confidence == matching.ConfidenceAuto {
		status = entity.MatchStatusConfirmed // Auto-confirm high-confidence matches
	}

	match := &entity.Match{
		ID:        uuid.New().String(),
		OfferID:   offer.ID,
		RequestID: request.ID,
		Score:     matchScore.Total,
		Status:    status,
		MatchedBy: string(matchScore.Confidence), // Store confidence band in MatchedBy for now
		Reasoning: matchScore.Breakdown,
		CreatedAt: time.Now(),
	}

	if status == entity.MatchStatusConfirmed {
		now := time.Now()
		match.ConfirmedAt = &now
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
			Str("confidence", string(matchScore.Confidence)).
			Str("breakdown", match.Reasoning).
			Msg("Created match")

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
