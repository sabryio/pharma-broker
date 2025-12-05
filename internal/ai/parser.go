package ai

import (
	"context"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

// SSEBroadcaster interface for real-time updates
type SSEBroadcaster interface {
	BroadcastNewOffer(offerID string, medication string)
	BroadcastNewRequest(requestID string, medication string)
	BroadcastNewMatch(matchID string, score float64)
}

// Parser processes raw messages and creates offers/requests
type Parser struct {
	gemini      *GeminiClient
	rawMsgRepo  domain.RawMessageRepository
	offerRepo   domain.OfferRepository
	requestRepo domain.RequestRepository
	matchRepo   domain.MatchRepository
	log         zerolog.Logger
	cfg         *config.GeminiConfig

	// Processing
	msgChan            <-chan *domain.RawMessage
	batchSize          int
	stopChan           chan struct{}
	wg                 sync.WaitGroup
	isAutoParseEnabled func() bool // Check if auto-parse is enabled

	// Real-time updates
	sseBroadcaster SSEBroadcaster
}

// NewParser creates a new message parser
func NewParser(
	gemini *GeminiClient,
	rawMsgRepo domain.RawMessageRepository,
	offerRepo domain.OfferRepository,
	requestRepo domain.RequestRepository,
	matchRepo domain.MatchRepository,
	msgChan <-chan *domain.RawMessage,
	cfg *config.GeminiConfig,
	log zerolog.Logger,
) *Parser {
	return &Parser{
		gemini:             gemini,
		rawMsgRepo:         rawMsgRepo,
		offerRepo:          offerRepo,
		requestRepo:        requestRepo,
		matchRepo:          matchRepo,
		log:                log.With().Str("component", "parser").Logger(),
		cfg:                cfg,
		msgChan:            msgChan,
		batchSize:          cfg.MaxMessagesPerRequest,
		stopChan:           make(chan struct{}),
		isAutoParseEnabled: func() bool { return true }, // Default: always parse
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
}

// Stop stops the parser
func (p *Parser) Stop() {
	close(p.stopChan)
	p.wg.Wait()
}

func (p *Parser) processLoop(ctx context.Context) {
	defer p.wg.Done()

	batch := make([]*domain.RawMessage, 0, p.batchSize)
	ticker := time.NewTicker(5 * time.Second) // Process batch every 5 seconds
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
		Msg("🚀 Sending to Gemini AI...")

	results, err := p.gemini.ParseMessages(ctx, batch)
	if err != nil {
		p.log.Error().
			Err(err).
			Str("step", "6_AI_ERROR").
			Msg("❌ AI parsing failed")
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
				Int("quantity", item.Quantity).
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

					// Find matching requests
					go p.findMatchesForOffer(context.Background(), offer)
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

					// Find matching offers
					go p.findMatchesForRequest(context.Background(), request)
				}
			}
		}

		// Mark message as processed
		if err := p.rawMsgRepo.MarkProcessed(ctx, msg.ID, nil); err != nil {
			p.log.Error().Err(err).Str("msg_id", msg.ID).Msg("Failed to mark message as processed")
		}
	}
}

func (p *Parser) createOffer(msg *domain.RawMessage, item *domain.ParsedItem) *domain.Offer {
	var expiryDate *time.Time
	if item.ExpiryDate != "" {
		if t, err := time.Parse("2006-01", item.ExpiryDate); err == nil {
			expiryDate = &t
		}
	}

	currency := item.Currency
	if currency == "" {
		currency = "EGP"
	}

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
		Currency:      currency,
		ExpiryDate:    expiryDate,
		BatchNumber:   item.BatchNumber,
		Notes:         item.Notes,
		RawMessage:    msg.Content,
		Status:        domain.StatusActive,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}
}

func (p *Parser) createRequest(msg *domain.RawMessage, item *domain.ParsedItem) *domain.Request {
	currency := item.Currency
	if currency == "" {
		currency = "EGP"
	}

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
		Currency:      currency,
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

	for _, req := range requests {
		score := calculateMatchScore(offer, req)
		if score >= 0.5 { // Minimum 50% match
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

	for _, offer := range offers {
		score := calculateMatchScore(offer, request)
		if score >= 0.5 { // Minimum 50% match
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
					Msg("Created potential match")
				// Broadcast new match via SSE
				if p.sseBroadcaster != nil {
					p.sseBroadcaster.BroadcastNewMatch(match.ID, score)
				}
			}
		}
	}
}

// calculateMatchScore computes similarity between offer and request
func calculateMatchScore(offer *domain.Offer, request *domain.Request) float64 {
	score := 0.0
	weights := 0.0

	// Medication name match (most important)
	if offer.Medication == request.Medication {
		score += 0.6
	} else if containsIgnoreCase(offer.Medication, request.Medication) ||
		containsIgnoreCase(request.Medication, offer.Medication) {
		score += 0.4
	}
	weights += 0.6

	// Quantity compatibility
	if offer.Quantity > 0 && request.Quantity > 0 {
		if offer.Quantity >= request.Quantity {
			score += 0.2
		} else {
			score += 0.1 // Partial match
		}
		weights += 0.2
	}

	// Price compatibility
	if offer.Price > 0 && request.MaxPrice > 0 {
		if offer.Price <= request.MaxPrice {
			score += 0.2
		}
		weights += 0.2
	}

	if weights > 0 {
		return score / weights
	}
	return score
}

func containsIgnoreCase(s, substr string) bool {
	return len(s) > 0 && len(substr) > 0 &&
		(s == substr || len(s) > len(substr) && s[:len(substr)] == substr)
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
