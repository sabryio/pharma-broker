package parsing

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/metrics"
)

// Start begins processing messages
func (p *Parser) Start(ctx context.Context) {
	// Start processing workers (exactly p.workers count)
	for i := 0; i < p.workers; i++ {
		p.wg.Add(1)
		go p.processLoop(ctx)
	}

	// Start match worker
	p.wg.Add(1)
	go p.matchWorkerLoop(ctx)

	// Load Embeddings (Async) - tracked in WaitGroup to prevent leak
	p.wg.Add(1)
	go func() {
		defer p.wg.Done()
		// Use timeout to prevent hanging on startup
		embedCtx, cancel := context.WithTimeout(ctx, EmbeddingRefreshTimeout)
		defer cancel()
		if err := p.embeddingCache.Refresh(embedCtx); err != nil {
			p.log.Error().Err(err).Msg("Failed to load initial embeddings")
		}
	}()

	p.log.Info().Int("workers", p.workers).Msg("Parser started")
}

// Stop stops the parser
func (p *Parser) Stop() {
	p.stopOnce.Do(func() {
		close(p.stopChan)
		close(p.matchStop)
		p.matchTicker.Stop()
	})
	p.wg.Wait()
}

// matchWorkerLoop continuously polls the DB for new matching jobs
func (p *Parser) matchWorkerLoop(ctx context.Context) {
	defer p.wg.Done()
	defer func() {
		if r := recover(); r != nil {
			p.log.Error().Interface("panic", r).Msg("Match worker panicked!")
			select {
			case <-p.matchStop:
				return
			default:
				p.wg.Add(1)
				go p.matchWorkerLoop(ctx)
			}
		}
	}()
	p.log.Info().Int("pool_size", p.matchPoolSize).Msg("Match Worker Pool started")

	// Semaphore for concurrent job processing - configurable size
	poolSize := p.matchPoolSize
	if poolSize <= 0 {
		poolSize = DefaultMatchPoolSize
	}
	sem := make(chan struct{}, poolSize)

	for {
		select {
		case <-ctx.Done():
			return
		case <-p.matchStop:
			return
		case <-p.matchTicker.C:
			// Poll for jobs
			jobs, err := p.matchQueueRepo.DequeueBatch(ctx, 10)
			if err != nil {
				p.log.Error().Err(err).Msg("Failed to dequeue match jobs")
				continue
			}

			// Update queue depth metric
			metrics.MatchQueueDepth.Set(float64(len(jobs)))

			if len(jobs) > 0 {
				p.log.Debug().Int("count", len(jobs)).Msg("Processing match jobs concurrently")
			}

			for _, job := range jobs {
				// Acquire semaphore slot
				sem <- struct{}{}

				go func(j *entity.MatchQueueItem) {
					defer func() { <-sem }() // Release slot
					start := time.Now()

					// Process based on type
					switch j.SourceType {
					case "OFFER":
						if offer, err := p.offerRepo.GetByID(ctx, j.SourceID); err == nil && offer != nil {
							p.matchingService.FindMatchesForOffer(ctx, offer)
						}
					case "REQUEST":
						if req, err := p.requestRepo.GetByID(ctx, j.SourceID); err == nil && req != nil {
							p.matchingService.FindMatchesForRequest(ctx, req)
						}
					}

					// Record metrics
					metrics.MatchJobsProcessed.Inc()
					metrics.MatchProcessingDuration.Observe(time.Since(start).Seconds())

					// Delete from queue after processing
					if err := p.matchQueueRepo.Delete(ctx, j.ID); err != nil {
						p.log.Error().Err(err).Str("job_id", j.ID).Msg("Failed to delete match job")
					}
				}(job)
			}
		}
	}
}

func (p *Parser) processLoop(ctx context.Context) {
	defer p.wg.Done()
	defer func() {
		if r := recover(); r != nil {
			p.log.Error().Interface("panic", r).Msg("Parser worker panicked!")
			select {
			case <-p.stopChan:
				return // Don't restart if stopping
			default:
				p.wg.Add(1)
				go p.processLoop(ctx)
			}
		}
	}()

	// Initial startup
	// No cache needed for FTS

	batch := make([]*entity.RawMessage, 0, p.batchSize)
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
		case job := <-p.inputChan:
			// Read from inputChan where ProcessMessage writes
			batch = append(batch, job.msg)
			if len(batch) >= p.batchSize {
				p.processBatch(ctx, batch)
				batch = make([]*entity.RawMessage, 0, p.batchSize)
			}
		case <-ticker.C:
			if len(batch) > 0 {
				p.processBatch(ctx, batch)
				batch = make([]*entity.RawMessage, 0, p.batchSize)
			}
		}
	}
}

func (p *Parser) processBatch(ctx context.Context, batch []*entity.RawMessage) {
	// Check context cancellation before expensive operations
	select {
	case <-ctx.Done():
		p.log.Warn().Msg("Context cancelled, skipping batch")
		return
	default:
	}

	// Check if auto-parsing is enabled
	if !p.isAutoParseEnabled() {
		p.log.Warn().
			Str("phase", "batch_processing").
			Int("batch_size", len(batch)).
			Msg("Auto-parse disabled, skipping batch")
		return
	}

	p.log.Info().
		Str("phase", "batch_processing").
		Int("batch_size", len(batch)).
		Msg("Starting AI batch processing")

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

	results, err := p.parseWithAI(ctx, batch)
	if err != nil {
		p.handleBatchError(ctx, batch, err)
		return
	}

	p.processResults(ctx, batch, results)
}

func (p *Parser) parseWithAI(ctx context.Context, batch []*entity.RawMessage) ([]*entity.AIParseResult, error) {
	// Get relevant mappings from DB using FTS (RAG-Lite)
	filteringStart := time.Now()
	mappings := p.getRelevantMappings(ctx, batch)

	p.log.Info().
		Int("relevant_mappings", len(mappings)).
		Dur("duration", time.Since(filteringStart)).
		Msg("Retrieved relevant medication mappings from DB (FTS)")

	mappingsSlice := mapToMedicationMappings(mappings)

	// Use circuit breaker if configured
	if p.aiCircuitBreaker != nil {
		result, err := p.aiCircuitBreaker.ExecuteWithContext(ctx, func(ctx context.Context) (any, error) {
			return p.aiProvider.ParseMessages(ctx, batch, mappingsSlice)
		})

		// Update metrics
		metrics.CircuitBreakerState.WithLabelValues(p.aiCircuitBreaker.Name()).Set(float64(p.aiCircuitBreaker.State()))

		if err != nil {
			p.log.Warn().
				Err(err).
				Str("step", "6_CIRCUIT_BREAKER").
				Str("circuit", p.aiCircuitBreaker.Name()).
				Bool("is_open", p.aiCircuitBreaker.IsOpen()).
				Msg("⚡ AI call failed or circuit open")
			metrics.CircuitBreakerFailures.WithLabelValues(p.aiCircuitBreaker.Name()).Inc()
			return nil, err
		}

		results := result.([]*entity.AIParseResult)
		p.log.Info().
			Str("step", "7_AI_RESPONSE").
			Int("result_count", len(results)).
			Msg("✅ AI response received")

		return results, nil
	}

	// Fallback: no circuit breaker configured
	results, err := p.aiProvider.ParseMessages(ctx, batch, mappingsSlice)
	if err != nil {
		return nil, err
	}

	p.log.Info().
		Str("step", "7_AI_RESPONSE").
		Int("result_count", len(results)).
		Msg("✅ AI response received")

	return results, nil
}

func (p *Parser) handleBatchError(ctx context.Context, batch []*entity.RawMessage, err error) {
	p.log.Error().
		Err(err).
		Str("step", "6_AI_ERROR").
		Msg("❌ AI parsing failed")
	metrics.SystemErrors.Inc()

	if p.errorNotifier != nil {
		p.errorNotifier.NotifyError(ctx, err)
	}
	// Mark all as failed
	for _, msg := range batch {
		if err := p.rawMsgRepo.MarkProcessed(ctx, msg.ID, err); err != nil {
			p.log.Error().Err(err).Str("msg_id", msg.ID).Msg("Failed to mark message as processed")
		}
	}
}

func (p *Parser) processResults(ctx context.Context, batch []*entity.RawMessage, results []*entity.AIParseResult) {
	for i, result := range results {
		msg := batch[i]
		p.processSingleResult(ctx, msg, result)
	}
	metrics.MessagesProcessed.Add(float64(len(batch)))
}

func (p *Parser) processSingleResult(ctx context.Context, msg *entity.RawMessage, result *entity.AIParseResult) {
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
		return
	}

	if len(result.Items) == 0 {
		p.log.Warn().
			Str("step", "8_NO_ITEMS").
			Str("msg_id", msg.ID).
			Str("content", msg.Content).
			Msg("⚠️ AI found NO offers/requests in message")

		// Queue for review if message has content but no extractions
		if len(msg.Content) > 20 && p.shouldQueueForReview(result) {
			p.queueForReview(ctx, msg, result, int(ParsePassStrict), "No items extracted from message with content")
		}
		p.rawMsgRepo.MarkProcessed(ctx, msg.ID, nil)
		return
	}

	// Check average confidence - queue low-confidence results for review
	avgConfidence := p.calculateAvgConfidence(result.Items)
	if avgConfidence < p.multiPassConfig.StrictMinConfidence && p.multiPassConfig.EnableReviewQueue {
		p.log.Info().
			Str("step", "8_LOW_CONFIDENCE").
			Str("msg_id", msg.ID).
			Float64("avg_confidence", avgConfidence).
			Float64("threshold", p.multiPassConfig.StrictMinConfidence).
			Msg("📋 Low confidence result, queuing for review")

		if p.reviewQueueRepo != nil {
			p.queueForReview(ctx, msg, result, int(ParsePassStrict), "Low average AI confidence")
		}
	}

	// Create offers and requests from parsed items
	for _, item := range result.Items {
		p.log.Info().
			Str("step", "9_ITEM").
			Str("type", string(item.Type)).
			Str("medication", item.Medication).
			Msg("📦 Extracted item from AI")

		switch item.Type {
		case entity.MessageTypeOffer, entity.MessageTypeBoth:
			offer := p.createOffer(msg, &item)
			if err := p.offerRepo.Save(ctx, offer); err != nil {
				p.log.Error().Err(err).Str("offer_id", offer.ID).Msg("Failed to save offer")
			} else {
				p.log.Info().
					Str("step", "10_OFFER_SAVED").
					Str("offer_id", offer.ID).
					Msg("✅ Created new OFFER")

				if p.sseBroadcaster != nil {
					p.sseBroadcaster.BroadcastNewOffer(offer.ID, offer.Medication)
				}
				err := p.matchQueueRepo.Enqueue(ctx, &entity.MatchQueueItem{
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
		case entity.MessageTypeRequest, entity.MessageTypeBoth:
			request := p.createRequest(msg, &item)
			if err := p.requestRepo.Save(ctx, request); err != nil {
				p.log.Error().Err(err).Str("request_id", request.ID).Msg("Failed to save request")
			} else {
				p.log.Info().
					Str("step", "10_REQUEST_SAVED").
					Str("request_id", request.ID).
					Msg("✅ Created new REQUEST")

				if p.sseBroadcaster != nil {
					p.sseBroadcaster.BroadcastNewRequest(request.ID, request.Medication)
				}
				metrics.RequestsCreated.Inc()

				err := p.matchQueueRepo.Enqueue(ctx, &entity.MatchQueueItem{
					SourceType: "REQUEST",
					SourceID:   request.ID,
				})
				if err != nil {
					p.log.Error().Err(err).Str("request_id", request.ID).Msg("Failed to enqueue match job for request")
				}
			}
		}
	}

	// Mark message as processed
	if err := p.rawMsgRepo.MarkProcessed(ctx, msg.ID, nil); err != nil {
		p.log.Error().Err(err).Str("msg_id", msg.ID).Msg("Failed to mark message as processed")
	}
}
