package ai

import (
	"context"
	"math"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/metrics"
	"pharmabroker/internal/storage"
)

// ErrorNotifier interface for reporting system errors
type ErrorNotifier interface {
	NotifyError(err error)
}

// SSEBroadcaster interface for real-time updates
type SSEBroadcaster interface {
	BroadcastNewOffer(offerID string, medication string)
	BroadcastNewRequest(requestID string, medication string)
	BroadcastNewMatch(matchID string, score float64)
}

// ProcessMessage queues a message for processing
func (p *Parser) ProcessMessage(ctx context.Context, msg *domain.RawMessage) {
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
	msg *domain.RawMessage
}

// Parser processes raw messages and creates offers/requests
type Parser struct {
	aiProvider  AIProvider
	rawMsgRepo  domain.RawMessageRepository
	offerRepo   domain.OfferRepository
	requestRepo domain.RequestRepository

	matchRepo      domain.MatchRepository
	medicationRepo domain.MedicationMappingRepository
	log            zerolog.Logger
	parserCfg      *config.ParserConfig

	// Processing
	msgChan            <-chan *domain.RawMessage
	batchSize          int
	stopChan           chan struct{}
	wg                 sync.WaitGroup
	isAutoParseEnabled func() bool // Check if auto-parse is enabled
	matchQueueRepo     domain.MatchQueueRepository

	// Workers
	workers     int
	inputChan   chan inputJob
	matchTicker *time.Ticker
	matchStop   chan struct{}

	// Real-time updates and Dynamic Config
	sseBroadcaster SSEBroadcaster
	configRepo     interface {
		GetAll(ctx context.Context) (*storage.AppConfig, error)
	}
	errorNotifier ErrorNotifier

	// Vector Store (In-Memory)
	embeddingMu          sync.RWMutex
	medicationEmbeddings map[string][]float32

	// Professional Scorer (Phase 1 Matching Enhancement)
	scorer *Scorer
}

// NewParser creates a new Parser instance
func NewParser(
	rawMsgRepo domain.RawMessageRepository,
	aiProvider AIProvider,
	offerRepo domain.OfferRepository,
	requestRepo domain.RequestRepository,
	medicationRepo domain.MedicationMappingRepository,
	matchQueueRepo domain.MatchQueueRepository,
	configRepo interface {
		GetAll(ctx context.Context) (*storage.AppConfig, error)
	},
	errorNotifier ErrorNotifier,
	broadcaster SSEBroadcaster,
	logger zerolog.Logger,
) *Parser {
	return &Parser{
		rawMsgRepo:           rawMsgRepo,
		aiProvider:           aiProvider,
		offerRepo:            offerRepo,
		requestRepo:          requestRepo,
		medicationRepo:       medicationRepo,
		matchQueueRepo:       matchQueueRepo,
		configRepo:           configRepo,
		errorNotifier:        errorNotifier,
		sseBroadcaster:       broadcaster,
		log:                  logger,
		workers:              10, // Default workers for input processing
		inputChan:            make(chan inputJob, 100),
		stopChan:             make(chan struct{}),
		matchTicker:          time.NewTicker(2 * time.Second), // Poll queue every 2s
		matchStop:            make(chan struct{}),
		isAutoParseEnabled:   func() bool { return true }, // Default: always parse
		medicationEmbeddings: make(map[string][]float32),
		scorer:               NewScorer(nil, nil), // Use default weights and thresholds
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

// Start begins processing messages
func (p *Parser) Start(ctx context.Context) {
	p.wg.Add(1)
	go p.processLoop(ctx)

	// Start match workers
	// Limit concurrent matching
	for i := 0; i < p.workers; i++ {
		p.wg.Add(1)
		go p.processLoop(ctx)
	}

	// Start match worker
	p.wg.Add(1)
	go p.matchWorkerLoop(ctx)

	// Load Embeddings (Async)
	go func() {
		if err := p.refreshEmbeddings(ctx); err != nil {
			p.log.Error().Err(err).Msg("Failed to load initial embeddings")
		}
	}()

	p.log.Info().Int("workers", p.workers).Msg("Parser started")
}

// Stop stops the parser
func (p *Parser) Stop() {
	close(p.stopChan)
	p.wg.Wait()
}

// matchWorkerLoop continuously polls the DB for new matching jobs
func (p *Parser) matchWorkerLoop(ctx context.Context) {
	defer p.wg.Done() // Changed from p.workerWg.Done() to p.wg.Done() to match Start()
	p.log.Info().Msg("Match Worker Queue Poller started")

	for {
		select {
		case <-ctx.Done():
			return
		case <-p.matchStop:
			return
		case <-p.matchTicker.C:
			// Poll for jobs
			jobs, err := p.matchQueueRepo.DequeueBatch(ctx, 10) // Process 10 at a time
			if err != nil {
				p.log.Error().Err(err).Msg("Failed to dequeue match jobs")
				continue
			}

			if len(jobs) > 0 {
				p.log.Debug().Int("count", len(jobs)).Msg("Processing match jobs from queue")
			}

			for _, job := range jobs {
				// Process matched job
				switch job.SourceType {
				case "OFFER":
					offer, err := p.offerRepo.GetByID(ctx, job.SourceID)
					if err == nil && offer != nil {
						p.findMatchesForOffer(ctx, offer)
					}
				case "REQUEST":
					req, err := p.requestRepo.GetByID(ctx, job.SourceID)
					if err == nil && req != nil {
						p.findMatchesForRequest(ctx, req)
					}
				}

				// Delete from queue after processing (or if source not found)
				if err := p.matchQueueRepo.Delete(ctx, job.ID); err != nil {
					p.log.Error().Err(err).Str("job_id", job.ID).Msg("Failed to delete match job")
				}
			}
		}
	}
}

func (p *Parser) processLoop(ctx context.Context) {
	defer p.wg.Done()

	// Initial startup
	// No cache needed for FTS

	batch := make([]*domain.RawMessage, 0, p.batchSize)
	ticker := time.NewTicker(p.parserCfg.BatchInterval) // Use config for batch interval
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			// Process remaining batch before exit
			if len(batch) > 0 {
				p.processBatch(context.Background(), batch)
			}
			return
		case <-p.stopChan:
			if len(batch) > 0 {
				p.processBatch(context.Background(), batch)
			}
			return
		case msg := <-p.msgChan:
			batch = append(batch, msg)
			if len(batch) >= p.batchSize {
				p.processBatch(ctx, batch)
				batch = make([]*domain.RawMessage, 0, p.batchSize)
			}
		case <-ticker.C:
			if len(batch) > 0 {
				p.processBatch(ctx, batch)
				batch = make([]*domain.RawMessage, 0, p.batchSize)
			}
		}
	}
}

func (p *Parser) processBatch(ctx context.Context, batch []*domain.RawMessage) {
	// Check if auto-parsing is enabled
	if !p.isAutoParseEnabled() {
		p.log.Warn().
			Str("step", "5_AUTO_PARSE_DISABLED").
			Int("count", len(batch)).
			Msg("⏸️ Auto-parse disabled, skipping batch")
		return
	}

	p.log.Info().
		Str("step", "5_PROCESSING").
		Int("count", len(batch)).
		Msg("🤖 Starting AI batch processing")

	start := time.Now()
	defer func() {
		metrics.MessageProcessingDuration.Observe(time.Since(start).Seconds())
	}()

	// Log each message being processed
	for i, msg := range batch {
		p.log.Info().
			Str("step", "5_BATCH_ITEM").
			Int("index", i).
			Str("msg_id", msg.ID).
			Str("group", msg.GroupName).
			Str("content", msg.Content).
			Msg("📝 Message in batch")
	}

	// Call Gemini API
	p.log.Info().
		Str("step", "6_AI_REQUEST").
		Int("message_count", len(batch)).
		Msg("🚀 Sending to AI provider...")

	// Get relevant mappings from DB using FTS (RAG-Lite)
	filteringStart := time.Now()
	mappings := p.getRelevantMappings(ctx, batch)

	p.log.Info().
		Int("relevant_mappings", len(mappings)).
		Dur("duration", time.Since(filteringStart)).
		Msg("Retrieved relevant medication mappings from DB (FTS)")

	results, err := p.aiProvider.ParseMessages(ctx, batch, mappings)
	if err != nil {
		p.log.Error().
			Err(err).
			Str("step", "6_AI_ERROR").
			Msg("❌ AI parsing failed")
		metrics.SystemErrors.Inc()
		if p.errorNotifier != nil {
			p.errorNotifier.NotifyError(err)
		}
		// Mark all as failed
		for _, msg := range batch {
			if err := p.rawMsgRepo.MarkProcessed(ctx, msg.ID, err); err != nil {
				p.log.Error().Err(err).Str("msg_id", msg.ID).Msg("Failed to mark message as processed")
			}
		}
		return
	}

	p.log.Info().
		Str("step", "7_AI_RESPONSE").
		Int("result_count", len(results)).
		Msg("✅ AI response received")

	// Process each result
	for i, result := range results {
		msg := batch[i]

		p.log.Info().
			Str("step", "8_RESULT").
			Str("msg_id", msg.ID).
			Int("items_found", len(result.Items)).
			Str("raw_json", result.RawJSON).
			Msg("📊 AI result for message")

		if result.Error != "" {
			p.log.Warn().
				Str("step", "8_RESULT_ERROR").
				Str("msg_id", msg.ID).
				Str("error", result.Error).
				Msg("⚠️ AI returned error for message")
			p.rawMsgRepo.MarkProcessed(ctx, msg.ID, nil)
			continue
		}

		if len(result.Items) == 0 {
			p.log.Warn().
				Str("step", "8_NO_ITEMS").
				Str("msg_id", msg.ID).
				Str("content", msg.Content).
				Msg("⚠️ AI found NO offers/requests in message")
		}

		// Create offers and requests from parsed items
		for _, item := range result.Items {
			p.log.Info().
				Str("step", "9_ITEM").
				Str("type", string(item.Type)).
				Str("medication", item.Medication).
				Str("medication_raw", item.MedicationRaw).
				Float64("quantity", item.Quantity).
				Float64("price", item.Price).
				Bool("urgent", item.Urgent).
				Msg("📦 Extracted item from AI")

			switch item.Type {
			case domain.MessageTypeOffer, domain.MessageTypeBoth:
				offer := p.createOffer(msg, &item)
				if err := p.offerRepo.Save(ctx, offer); err != nil {
					p.log.Error().Err(err).Str("offer_id", offer.ID).Msg("Failed to save offer")
				} else {
					p.log.Info().
						Str("step", "10_OFFER_SAVED").
						Str("offer_id", offer.ID).
						Str("medication", offer.Medication).
						Msg("✅ Created new OFFER")

					// Broadcast to connected clients
					if p.sseBroadcaster != nil {
						p.sseBroadcaster.BroadcastNewOffer(offer.ID, offer.Medication)
					}
					// Queue for matching (Persistent)
					err := p.matchQueueRepo.Enqueue(ctx, &domain.MatchQueueItem{
						SourceType: "OFFER",
						SourceID:   offer.ID,
					})
					if err != nil {
						p.log.Error().Err(err).Str("offer_id", offer.ID).Msg("Failed to enqueue match job for offer")
					}

					metrics.OffersCreated.Inc()
				}
			}

			switch item.Type {
			case domain.MessageTypeRequest, domain.MessageTypeBoth:
				request := p.createRequest(msg, &item)
				if err := p.requestRepo.Save(ctx, request); err != nil {
					p.log.Error().Err(err).Str("request_id", request.ID).Msg("Failed to save request")
				} else {
					p.log.Info().
						Str("step", "10_REQUEST_SAVED").
						Str("request_id", request.ID).
						Str("medication", request.Medication).
						Msg("✅ Created new REQUEST")

					// Broadcast to connected clients
					if p.sseBroadcaster != nil {
						p.sseBroadcaster.BroadcastNewRequest(request.ID, request.Medication)
					}
					metrics.RequestsCreated.Inc()

					// Queue for matching (Persistent)
					err := p.matchQueueRepo.Enqueue(ctx, &domain.MatchQueueItem{
						SourceType: "REQUEST",
						SourceID:   request.ID,
					})
					if err != nil {
						p.log.Error().Err(err).Str("request_id", request.ID).Msg("Failed to enqueue match job for request")
					}
				}
			}
		}
		metrics.MessagesProcessed.Inc()

		// Mark message as processed
		if err := p.rawMsgRepo.MarkProcessed(ctx, msg.ID, nil); err != nil {
			p.log.Error().Err(err).Str("msg_id", msg.ID).Msg("Failed to mark message as processed")
		}
	}
}

func (p *Parser) createOffer(msg *domain.RawMessage, item *domain.ParsedItem) *domain.Offer {
	// Expiry and Batch are now in Notes
	// Currency defaults to EGP

	return &domain.Offer{
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
		Status:        domain.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
}

func (p *Parser) createRequest(msg *domain.RawMessage, item *domain.ParsedItem) *domain.Request {
	return &domain.Request{
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
		Status:        domain.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
}

func (p *Parser) findMatchesForOffer(ctx context.Context, offer *domain.Offer) {
	// Search for matching requests by medication name
	requests, err := p.requestRepo.Search(ctx, offer.Medication, 10, 0)
	if err != nil {
		p.log.Error().Err(err).Msg("Failed to search for matching requests")
		return
	}

	// Dynamic Config: Get current match threshold
	// Use default 0.5 if config fetch fails
	matchThreshold := 0.5
	if p.configRepo != nil {
		if cfg, err := p.configRepo.GetAll(ctx); err == nil {
			matchThreshold = cfg.MatchThreshold
		}
	}

	for _, req := range requests {
		score := p.calculateMatchScore(offer, req)
		if score >= matchThreshold {
			match := &domain.Match{
				ID:        uuid.New().String(),
				OfferID:   offer.ID,
				RequestID: req.ID,
				Score:     score,
				Reasoning: generateMatchReasoning(offer, req, score),
				Status:    domain.MatchStatusPending,
				CreatedAt: time.Now(),
			}

			if err := p.matchRepo.Save(ctx, match); err != nil {
				p.log.Error().Err(err).Msg("Failed to save match")
			} else {
				p.log.Info().
					Str("match_id", match.ID).
					Float64("score", score).
					Float64("threshold", matchThreshold).
					Msg("Created potential match")
				// Broadcast new match via SSE
				if p.sseBroadcaster != nil {
					p.sseBroadcaster.BroadcastNewMatch(match.ID, score)
				}
			}
		}
	}
}

func (p *Parser) findMatchesForRequest(ctx context.Context, request *domain.Request) {
	// Search for matching offers by medication name
	offers, err := p.offerRepo.Search(ctx, request.Medication, 10, 0)
	if err != nil {
		p.log.Error().Err(err).Msg("Failed to search for matching offers")
		return
	}

	// Dynamic Config: Get current match threshold
	matchThreshold := 0.5
	if p.configRepo != nil {
		if cfg, err := p.configRepo.GetAll(ctx); err == nil {
			matchThreshold = cfg.MatchThreshold
		}
	}

	for _, offer := range offers {
		score := p.calculateMatchScore(offer, request)
		if score >= matchThreshold {
			match := &domain.Match{
				ID:        uuid.New().String(),
				OfferID:   offer.ID,
				RequestID: request.ID,
				Score:     score,
				Reasoning: generateMatchReasoning(offer, request, score),
				Status:    domain.MatchStatusPending,
				CreatedAt: time.Now(),
			}

			if err := p.matchRepo.Save(ctx, match); err != nil {
				// Might fail on duplicate, that's ok
				if err.Error() != "UNIQUE constraint failed" {
					p.log.Error().Err(err).Msg("Failed to save match")
				}
			} else {
				p.log.Info().
					Str("match_id", match.ID).
					Float64("score", score).
					Float64("threshold", matchThreshold).
					Msg("Created potential match")
				// Broadcast new match via SSE
				if p.sseBroadcaster != nil {
					p.sseBroadcaster.BroadcastNewMatch(match.ID, score)
				}
			}
		}
	}
}

// getRelevantMappings extracts tokens from messages and queries the FTS index
func (p *Parser) getRelevantMappings(ctx context.Context, messages []*domain.RawMessage) map[string]string {
	relevant := make(map[string]string)

	// Tokenize messages to form a search query
	uniqueTokens := make(map[string]struct{})
	for _, msg := range messages {
		// Normalize and split
		content := strings.ToLower(msg.Content)
		// Replace common punctuation with space
		content = strings.ReplaceAll(content, "\n", " ")
		content = strings.ReplaceAll(content, ",", " ")
		content = strings.ReplaceAll(content, ".", " ")
		content = strings.ReplaceAll(content, "-", " ")

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

	// Build OR query
	var queryBuilder strings.Builder
	first := true
	count := 0
	const maxTokens = 50 // Limit to top 50 tokens to avoid query string limit

	for token := range uniqueTokens {
		if count >= maxTokens {
			break
		}
		if !first {
			queryBuilder.WriteString(" OR ")
		}
		// Sanitize token for FTS (remove quotes)
		cleanToken := strings.ReplaceAll(token, "\"", "")
		cleanToken = strings.ReplaceAll(cleanToken, "'", "")
		if cleanToken == "" {
			continue
		}

		// Use quotes to handle tokens with special chars if needed,
		// but simple cleaning is safer.
		// "token" in FTS5
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

	// 2. Fuzzy Search (Fallback)
	// If exact search yields few results, or we want to be more tolerant (e.g. typos).
	// We generate trigrams for words that are "likely items" (length > 3).
	// This is "Poor Man's Vector Search" using FTS5 trigrams.

	// Only do fuzzy if we have enough distinct tokens to justify it,
	// and maybe distinct from what we already found?
	// For simplicity in RAG-Lite: Just add fuzzy results to the map. Map handles deduplication.

	var fuzzyQueryBuilder strings.Builder
	firstFuzzy := true
	fuzzyCount := 0

	for token := range uniqueTokens {
		// Only fuzzy match longer words found in dictionary usually
		if len([]rune(token)) < 4 { // Skip short words (rune count for Arabic)
			continue
		}

		trigrams := generateTrigrams(token)
		if len(trigrams) == 0 {
			continue
		}

		// LIMIT fuzzy queries to avoid massive OR statements
		if fuzzyCount >= 20 {
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

// generateTrigrams splits a string into 3-character substrings
func generateTrigrams(s string) []string {
	runes := []rune(s)
	if len(runes) < 3 {
		return nil
	}
	var trigrams []string
	for i := 0; i < len(runes)-2; i++ {
		trigrams = append(trigrams, string(runes[i:i+3]))
	}
	return trigrams
}

// calculateMatchScore computes similarity between offer and request using multi-field scoring
func (p *Parser) calculateMatchScore(offer *domain.Offer, request *domain.Request) float64 {
	// Calculate medication score using hybrid approach (lexical + semantic)
	medicationScore := p.calculateMedicationScore(offer.Medication, request.Medication)

	// Use the professional scorer for weighted multi-field scoring
	matchScore := p.scorer.ScoreMatch(offer, request, medicationScore)

	// Log detailed breakdown for debugging
	p.log.Debug().
		Str("offer_med", offer.Medication).
		Str("request_med", request.Medication).
		Float64("med_score", matchScore.MedicationScore).
		Float64("qty_score", matchScore.QuantityScore).
		Float64("price_score", matchScore.PriceScore).
		Float64("recency_score", matchScore.RecencyScore).
		Float64("total", matchScore.Total).
		Str("confidence", string(matchScore.Confidence)).
		Str("breakdown", matchScore.Breakdown).
		Msg("Match score calculated")

	return matchScore.Total
}

// calculateMedicationScore computes a hybrid lexical + semantic medication match score
func (p *Parser) calculateMedicationScore(offerMed, requestMed string) float64 {
	// Lexical scoring using fuzzy match
	lexicalScore := fuzzyMatch(offerMed, requestMed)

	// Apply scaling to fuzzy match score to fit 0-1 range better
	normalizedLexical := 0.0
	if lexicalScore >= 1.0 {
		normalizedLexical = 1.0 // Exact match
	} else if lexicalScore >= 0.8 {
		normalizedLexical = 0.85 // High similarity
	} else if lexicalScore >= 0.6 {
		normalizedLexical = 0.6 // Moderate similarity
	} else if containsIgnoreCase(offerMed, requestMed) ||
		containsIgnoreCase(requestMed, offerMed) {
		normalizedLexical = 0.5 // Substring match fallback
	} else {
		normalizedLexical = lexicalScore * 0.5 // Low similarity
	}

	// Semantic scoring using embeddings
	semanticScore := 0.0
	p.embeddingMu.RLock()
	vecA, okA := p.medicationEmbeddings[strings.ToLower(offerMed)]
	vecB, okB := p.medicationEmbeddings[strings.ToLower(requestMed)]
	p.embeddingMu.RUnlock()

	if okA && okB {
		semanticScore = CosineSimilarity(vecA, vecB)
	}

	// Hybrid: Weight semantic if available, otherwise use lexical only
	alpha := p.scorer.semanticWeight // Default 0.6
	if semanticScore > 0 {
		// Both available: weighted average
		return alpha*semanticScore + (1-alpha)*normalizedLexical
	}

	// Semantic not available: use lexical only
	return normalizedLexical
}

// CosineSimilarity computes the cosine similarity between two vectors
func CosineSimilarity(a, b []float32) float64 {
	if len(a) != len(b) || len(a) == 0 {
		return 0.0
	}

	dotProduct := 0.0
	normA := 0.0
	normB := 0.0

	for i := 0; i < len(a); i++ {
		dotProduct += float64(a[i] * b[i])
		normA += float64(a[i] * a[i])
		normB += float64(b[i] * b[i])
	}

	if normA == 0 || normB == 0 {
		return 0.0
	}

	return dotProduct / (math.Sqrt(normA) * math.Sqrt(normB))
}

func containsIgnoreCase(s, substr string) bool {
	s, substr = strings.ToUpper(s), strings.ToUpper(substr)
	return strings.Contains(s, substr)
}

// refreshEmbeddings loads medication embeddings into memory
func (p *Parser) refreshEmbeddings(ctx context.Context) error {
	mappings, err := p.medicationRepo.GetAll(ctx)
	if err != nil {
		return err
	}

	newEmbeddings := make(map[string][]float32)
	count := 0

	for _, m := range mappings {
		if len(m.Embedding) == 0 {
			continue // Skip if no embedding
		}

		// Map all known names to this embedding
		if m.ArabicName != "" {
			newEmbeddings[strings.ToLower(m.ArabicName)] = m.Embedding
		}
		if m.EnglishName != "" {
			newEmbeddings[strings.ToLower(m.EnglishName)] = m.Embedding
		}
		for _, syn := range m.Synonyms {
			if syn != "" {
				newEmbeddings[strings.ToLower(syn)] = m.Embedding
			}
		}
		count++
	}

	p.embeddingMu.Lock()
	p.medicationEmbeddings = newEmbeddings
	p.embeddingMu.Unlock()

	p.log.Info().Int("count", count).Int("keys", len(newEmbeddings)).Msg("Refreshed in-memory embeddings")
	return nil
}

// fuzzyMatch returns a similarity score between 0 and 1 using Levenshtein distance
func fuzzyMatch(s1, s2 string) float64 {
	// Normalize strings: lowercase and trim
	s1 = strings.ToLower(strings.TrimSpace(s1))
	s2 = strings.ToLower(strings.TrimSpace(s2))

	if s1 == s2 {
		return 1.0
	}

	if len(s1) == 0 || len(s2) == 0 {
		return 0.0
	}

	// Calculate Levenshtein distance
	dist := levenshteinDistance(s1, s2)
	maxLen := max(len(s1), len(s2))

	// Convert to similarity score (1 - normalized distance)
	similarity := 1.0 - float64(dist)/float64(maxLen)
	return similarity
}

// levenshteinDistance calculates the edit distance between two strings
func levenshteinDistance(s1, s2 string) int {
	if len(s1) == 0 {
		return len(s2)
	}
	if len(s2) == 0 {
		return len(s1)
	}

	// Use two rows instead of full matrix for memory efficiency
	prev := make([]int, len(s2)+1)
	curr := make([]int, len(s2)+1)

	// Initialize first row
	for j := range prev {
		prev[j] = j
	}

	for i := 1; i <= len(s1); i++ {
		curr[0] = i
		for j := 1; j <= len(s2); j++ {
			cost := 1
			if s1[i-1] == s2[j-1] {
				cost = 0
			}
			curr[j] = min(
				prev[j]+1,      // deletion
				curr[j-1]+1,    // insertion
				prev[j-1]+cost, // substitution
			)
		}
		prev, curr = curr, prev
	}

	return prev[len(s2)]
}

func generateMatchReasoning(offer *domain.Offer, request *domain.Request, score float64) string {
	reasons := []string{}

	if offer.Medication == request.Medication {
		reasons = append(reasons, "Exact medication match")
	} else {
		reasons = append(reasons, "Similar medication names")
	}

	if offer.Quantity >= request.Quantity && request.Quantity > 0 {
		reasons = append(reasons, "Sufficient quantity available")
	}

	if offer.Price > 0 && request.MaxPrice > 0 && offer.Price <= request.MaxPrice {
		reasons = append(reasons, "Price within budget")
	}

	if len(reasons) == 0 {
		return "Potential match based on medication similarity"
	}

	result := reasons[0]
	for i := 1; i < len(reasons); i++ {
		result += "; " + reasons[i]
	}
	return result
}
