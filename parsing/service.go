package parsing

import (
	"context"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"

	ai "pharmabroker/ai"
	aiCircuitBreaker "pharmabroker/ai/circuitbreaker"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/matching"
	"pharmabroker/pkg/config"
)

// ProcessMessage queues a message for processing
func (p *Parser) ProcessMessage(ctx context.Context, msg *entity.RawMessage) {
	select {
	case p.inputChan <- inputJob{ctx: ctx, msg: msg}:
		// Queued
	default:
		p.log.Warn().Str("msg_id", msg.ID).Msg("Input queue full, dropping message")
	}
}

// inputJob represents a processing job for the worker pool
type inputJob struct {
	ctx context.Context
	msg *entity.RawMessage
}

// Parser processes raw messages and creates offers/requests
type Parser struct {
	aiProvider  ai.Provider
	rawMsgRepo  repository.RawMessageRepository
	offerRepo   repository.OfferRepository
	requestRepo repository.RequestRepository

	matchRepo      repository.MatchRepository
	medicationRepo repository.MedicationMappingRepository
	log            zerolog.Logger
	parserCfg      *config.ParserConfig

	// Processing
	batchSize          int
	stopChan           chan struct{}
	wg                 sync.WaitGroup
	isAutoParseEnabled func() bool // Check if auto-parse is enabled
	matchQueueRepo     repository.MatchQueueRepository

	// Workers
	workers       int
	inputChan     chan inputJob
	matchTicker   *time.Ticker
	matchStop     chan struct{}
	matchPoolSize int // Configurable match worker pool size

	// Circuit Breaker for AI calls
	aiCircuitBreaker *aiCircuitBreaker.Breaker

	// Real-time updates and Dynamic Config
	// Real-time updates and Dynamic Config
	sseBroadcaster SSEBroadcaster
	configRepo     interface {
		GetAll(ctx context.Context) (*entity.AppConfig, error)
	}
	errorNotifier ErrorNotifier

	// Services
	embeddingCache  *EmbeddingCache
	matchingService *MatchingService

	// Multi-Pass Parsing
	reviewQueueRepo repository.ReviewQueueRepository
	multiPassConfig MultiPassConfig

	// Synchronization
	stopOnce sync.Once
}

// NewParser creates a new Parser instance
func NewParser(
	rawMsgRepo repository.RawMessageRepository,
	aiProvider ai.Provider,
	offerRepo repository.OfferRepository,
	requestRepo repository.RequestRepository,
	matchRepo repository.MatchRepository, // Added dependency
	medicationRepo repository.MedicationMappingRepository,
	matchQueueRepo repository.MatchQueueRepository,
	configRepo interface {
		GetAll(ctx context.Context) (*entity.AppConfig, error)
	},
	errorNotifier ErrorNotifier,
	broadcaster SSEBroadcaster,
	logger zerolog.Logger,
) *Parser {
	// Initialize Services
	embeddingCache := NewEmbeddingCache(medicationRepo, logger)
	scorer := matching.NewScorer(nil, nil)
	matchingService := NewMatchingService(
		offerRepo,
		requestRepo,
		matchRepo,
		matchQueueRepo,
		scorer,
		embeddingCache,
		broadcaster,
		logger,
	)

	return &Parser{
		rawMsgRepo:     rawMsgRepo,
		aiProvider:     aiProvider,
		offerRepo:      offerRepo,
		requestRepo:    requestRepo,
		matchRepo:      matchRepo,
		medicationRepo: medicationRepo,
		matchQueueRepo: matchQueueRepo,
		configRepo:     configRepo,
		errorNotifier:  errorNotifier,
		sseBroadcaster: broadcaster,
		log:            logger,
		workers:        DefaultWorkerCount,
		inputChan:      make(chan inputJob, DefaultInputChannelSize),
		stopChan:       make(chan struct{}),
		matchTicker:    time.NewTicker(2 * time.Second),
		matchStop:      make(chan struct{}),
		matchPoolSize:  DefaultMatchPoolSize,
		aiCircuitBreaker: aiCircuitBreaker.New(aiCircuitBreaker.Config{
			Name:             "ai_provider",
			Timeout:          DefaultCircuitBreakerTimeout,
			FailureThreshold: DefaultCircuitBreakerThreshold,
		}, logger),
		isAutoParseEnabled: func() bool { return true },
		embeddingCache:     embeddingCache,
		matchingService:    matchingService,
		batchSize:          DefaultBatchSize,
		parserCfg: &config.ParserConfig{
			BatchInterval:     DefaultBatchInterval,
			MatchThreshold:    DefaultMatchThreshold,
			MessageBufferSize: DefaultMessageBufferSize,
		},
	}
}

// SetSSEBroadcaster sets the SSE broadcaster for real-time updates
func (p *Parser) SetSSEBroadcaster(broadcaster SSEBroadcaster) {
	p.sseBroadcaster = broadcaster
}

// SetAutoParseChecker sets the function to check if auto-parse is enabled
func (p *Parser) SetAutoParseChecker(fn func() bool) {
	p.isAutoParseEnabled = fn
}

// SetReviewQueueRepo sets the review queue repository for multi-pass parsing
func (p *Parser) SetReviewQueueRepo(repo repository.ReviewQueueRepository) {
	p.reviewQueueRepo = repo
	p.multiPassConfig = DefaultMultiPassConfig()
}

// SetMultiPassConfig sets the multi-pass parsing configuration
func (p *Parser) SetMultiPassConfig(cfg MultiPassConfig) {
	p.multiPassConfig = cfg
}

// calculateAvgConfidence computes the average AI confidence across parsed items
func (p *Parser) calculateAvgConfidence(items []entity.ParsedItem) float64 {
	if len(items) == 0 {
		return 0.0
	}
	var sum float64
	for _, item := range items {
		sum += item.AIConfidence
	}
	return sum / float64(len(items))
}

// shouldQueueForReview checks if a parse result needs manual review
func (p *Parser) shouldQueueForReview(result *entity.AIParseResult) bool {
	if p.reviewQueueRepo == nil {
		return false // Review queue not configured
	}

	// Errors are handled separately (dead letter queue, etc.)
	if result.Error != "" {
		return false
	}

	// Empty results (no items, no error) might need review
	if len(result.Items) == 0 {
		return p.multiPassConfig.EnableReviewQueue
	}

	// Low confidence results need review
	avgConf := p.calculateAvgConfidence(result.Items)
	return avgConf < p.multiPassConfig.RelaxedMinConfidence && p.multiPassConfig.EnableReviewQueue
}

// queueForReview adds a message to the review queue
func (p *Parser) queueForReview(ctx context.Context, msg *entity.RawMessage, result *entity.AIParseResult, pass int, reason string) {
	if p.reviewQueueRepo == nil {
		return
	}

	item := &entity.ReviewQueueItem{
		RawMessageID:  msg.ID,
		GroupName:     msg.GroupName,
		SenderName:    msg.SenderName,
		Content:       msg.Content,
		ReplyContext:  msg.ReplyToContent,
		PartialItems:  result.Items,
		ParsePass:     pass,
		AvgConfidence: p.calculateAvgConfidence(result.Items),
		FailureReason: reason,
		Status:        entity.ReviewStatusPending,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}

	if err := p.reviewQueueRepo.Save(ctx, item); err != nil {
		p.log.Error().
			Err(err).
			Str("msg_id", msg.ID).
			Msg("Failed to queue message for review")
	} else {
		p.log.Info().
			Str("msg_id", msg.ID).
			Int("pass", pass).
			Float64("avg_confidence", item.AvgConfidence).
			Str("reason", reason).
			Msg("📋 Message queued for manual review")
	}
}

// mapToMedicationMappings converts map[string]string to []*domain.MedicationMapping
func mapToMedicationMappings(mappings map[string]string) []*entity.MedicationMapping {
	result := make([]*entity.MedicationMapping, 0, len(mappings))
	for arabic, english := range mappings {
		result = append(result, &entity.MedicationMapping{
			ArabicName:  arabic,
			EnglishName: english,
		})
	}
	return result
}

func (p *Parser) createOffer(msg *entity.RawMessage, item *entity.ParsedItem) *entity.Offer {
	// Expiry and Batch are now in Notes
	// Currency defaults to EGP

	return &entity.Offer{
		ID:            uuid.New().String(),
		RawMessageID:  msg.ID,
		SourcePhone:   msg.SenderPhone,
		SourceName:    msg.SenderName,
		SourceGroup:   msg.GroupJID,
		GroupName:     msg.GroupName,
		Medication:    item.Medication,
		MedicationRaw: item.MedicationRaw,
		Quantity:      item.Quantity,
		Unit:          item.Unit,
		Price:         item.Price,
		Currency:      "EGP",
		ExpiryDate:    nil,
		BatchNumber:   "",
		Notes:         item.Notes,
		RawMessage:    msg.Content,
		Status:        entity.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
}

func (p *Parser) createRequest(msg *entity.RawMessage, item *entity.ParsedItem) *entity.Request {
	return &entity.Request{
		ID:            uuid.New().String(),
		RawMessageID:  msg.ID,
		SourcePhone:   msg.SenderPhone,
		SourceName:    msg.SenderName,
		SourceGroup:   msg.GroupJID,
		GroupName:     msg.GroupName,
		Medication:    item.Medication,
		MedicationRaw: item.MedicationRaw,
		Quantity:      item.Quantity,
		Unit:          item.Unit,
		MaxPrice:      item.MaxPrice,
		Currency:      "EGP",
		Urgent:        item.Urgent,
		Notes:         item.Notes,
		RawMessage:    msg.Content,
		Status:        entity.StatusActive,
		CreatedAt:     time.Now(),
	}
}

// tokenReplacer sanitizes tokens for FTS5 queries.
// Removes quotes, colons (column prefix), exclamation (NOT), asterisks (wildcard), and Arabic diacritics.
var tokenReplacer = strings.NewReplacer(
	"\"", "",
	"'", "",
	":", "", // FTS5 column prefix operator
	"!", "", // FTS5 NOT operator
	"*", "", // FTS5 wildcard
	"(", "", // FTS5 grouping
	")", "", // FTS5 grouping
	"^", "", // FTS5 boost
	"~", "", // FTS5 NEAR
	"ً", "", // Arabic fathatan
	"ٌ", "", // Arabic dammatan
	"ٍ", "", // Arabic kasratan
	"َ", "", // Arabic fatha
	"ُ", "", // Arabic damma
	"ِ", "", // Arabic kasra
	"ّ", "", // Arabic shadda
	"ْ", "", // Arabic sukun
)

// contentReplacer normalizes message content
var contentReplacer = strings.NewReplacer("\n", " ", ",", " ", ".", " ", "-", " ")

// getRelevantMappings extracts tokens from messages and queries the FTS index.
// It uses both exact and fuzzy matching to find relevant medication mappings.
func (p *Parser) getRelevantMappings(ctx context.Context, messages []*entity.RawMessage) map[string]string {
	relevant := make(map[string]string)

	// Tokenize messages to form a search query
	uniqueTokens := make(map[string]struct{})
	for _, msg := range messages {
		// Normalize and split using reusable replacer
		content := strings.ToLower(msg.Content)
		content = contentReplacer.Replace(content)

		words := strings.Fields(content)
		for _, w := range words {
			// Filter out short words and common junk
			if len(w) > 2 {
				uniqueTokens[w] = struct{}{}
			}
		}
	}

	if len(uniqueTokens) == 0 {
		return relevant
	}

	// Build OR query with memory safeguard
	var queryBuilder strings.Builder
	first := true
	count := 0

	for token := range uniqueTokens {
		if count >= MaxFTSTokens {
			break
		}
		// Memory safeguard: prevent massive queries
		if queryBuilder.Len() > MaxQueryLength {
			p.log.Warn().Int("query_length", queryBuilder.Len()).Msg("Query length limit reached")
			break
		}

		if !first {
			queryBuilder.WriteString(" OR ")
		}
		// Sanitize token for FTS using optimized replacer
		cleanToken := tokenReplacer.Replace(token)
		if cleanToken == "" {
			continue
		}

		queryBuilder.WriteString("\"")
		queryBuilder.WriteString(cleanToken)
		queryBuilder.WriteString("\"")

		first = false
		count++
	}

	if count == 0 {
		return relevant
	}

	mappings, err := p.medicationRepo.Search(ctx, queryBuilder.String())
	if err != nil {
		p.log.Error().Err(err).Msg("Failed to search medication mappings (exact)")
	} else {
		for _, m := range mappings {
			relevant[m.ArabicName] = m.EnglishName
		}
	}

	// Fuzzy Search (Fallback)
	var fuzzyQueryBuilder strings.Builder
	firstFuzzy := true
	fuzzyCount := 0

	for token := range uniqueTokens {
		// Only fuzzy match longer words
		if len([]rune(token)) < MinWordLengthForFuzzy {
			continue
		}

		trigrams := generateTrigrams(token)
		if len(trigrams) == 0 {
			continue
		}

		if fuzzyCount >= MaxFuzzyQueries {
			break
		}

		// Memory safeguard for fuzzy queries
		if fuzzyQueryBuilder.Len() > MaxQueryLength {
			break
		}

		// For each word, we add its trigrams to the query
		for _, tri := range trigrams {
			if !firstFuzzy {
				fuzzyQueryBuilder.WriteString(" OR ")
			}
			fuzzyQueryBuilder.WriteString("\"")
			fuzzyQueryBuilder.WriteString(tri)
			fuzzyQueryBuilder.WriteString("\"")
			firstFuzzy = false
		}
		fuzzyCount++
	}

	if fuzzyCount > 0 {
		fuzzyResults, err := p.medicationRepo.Search(ctx, fuzzyQueryBuilder.String())
		if err != nil {
			p.log.Warn().Err(err).Msg("Failed to search medication mappings (fuzzy)")
		} else {
			p.log.Debug().Int("count", len(fuzzyResults)).Msg("Fuzzy search results")
			for _, m := range fuzzyResults {
				relevant[m.ArabicName] = m.EnglishName
			}
		}
	}

	return relevant
}
