package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"pharmabroker/ai/prompts"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/internal/config"
	"pharmabroker/internal/metrics"
	arabicPkg "pharmabroker/pkg/arabic"
	fuzzyPkg "pharmabroker/pkg/fuzzy"
	textPkg "pharmabroker/pkg/text"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/invopop/jsonschema"
	"github.com/openai/openai-go"
	"github.com/openai/openai-go/option"
	"github.com/rs/zerolog"
	"github.com/sony/gobreaker"
)

// Client handles communication with Docker Model Runner
// using the OpenAI-compatible API.
type Client struct {
	cfg          *config.DockerModelConfig
	client       openai.Client
	log          zerolog.Logger
	cb           *gobreaker.CircuitBreaker
	allMappings  []*entity.MedicationMapping       // For hybrid filtering
	vectorTopK   int                               // Top-K for vector search (default 10)
	unmappedRepo repository.UnmappedMedicationRepo // For active learning (optional)
}

func GenerateSchema[T any]() interface{} {
	reflector := jsonschema.Reflector{
		AllowAdditionalProperties: false,
		DoNotReference:            true,
	}
	var v T
	return reflector.Reflect(v)
}

// Cached schema to avoid reflection on every call
var aiParseResultSchema = GenerateSchema[entity.AIParseResult]()

// NewClient creates a new Docker Model Runner client.
// It connects to the OpenAI-compatible API endpoint exposed by Docker Model Runner.
func NewClient(cfg *config.DockerModelConfig, log zerolog.Logger) (*Client, error) {
	client := openai.NewClient(
		option.WithBaseURL(cfg.BaseURL),
		option.WithAPIKey("not-needed"), // Docker Model Runner doesn't require API key
	)

	// Circuit Breaker Settings - use config values with fallbacks
	cbMaxRequests := cfg.CBMaxRequests
	if cbMaxRequests == 0 {
		cbMaxRequests = 3
	}
	cbInterval := cfg.CBInterval
	if cbInterval == 0 {
		cbInterval = 60 * time.Second
	}
	cbTimeout := cfg.CBTimeout
	if cbTimeout == 0 {
		cbTimeout = 30 * time.Second
	}
	cbFailureRatio := cfg.CBFailureRatio
	if cbFailureRatio == 0 {
		cbFailureRatio = 0.6
	}

	st := gobreaker.Settings{
		Name:        "DockerModel",
		MaxRequests: cbMaxRequests,
		Interval:    cbInterval,
		Timeout:     cbTimeout,
		ReadyToTrip: func(counts gobreaker.Counts) bool {
			failureRatio := float64(counts.TotalFailures) / float64(counts.Requests)
			return counts.Requests >= 5 && failureRatio >= cbFailureRatio
		},
		OnStateChange: func(name string, from gobreaker.State, to gobreaker.State) {
			log.Warn().Str("name", name).Str("from", from.String()).Str("to", to.String()).Msg("Circuit Breaker state changed")
		},
	}

	return &Client{
		cfg:    cfg,
		client: client,
		log:    log.With().Str("component", "docker-model").Logger(),
		cb:     gobreaker.NewCircuitBreaker(st),
	}, nil
}

// SetMappings sets the full medication mappings for hybrid filtering
// This enables keyword + vector search when ParseMessages is called
func (c *Client) SetMappings(mappings []*entity.MedicationMapping) {
	c.allMappings = mappings
	if c.vectorTopK == 0 {
		c.vectorTopK = 10 // Default top-K
	}
	c.log.Info().Int("count", len(mappings)).Msg("Loaded medication mappings for hybrid filtering")
}

// SetUnmappedRepo sets the repository for saving unmapped medications (active learning)
func (c *Client) SetUnmappedRepo(repo repository.UnmappedMedicationRepo) {
	c.unmappedRepo = repo
	c.log.Info().Msg("Active learning enabled - unmapped medications will be saved")
}

// ParseMessages implements AIProvider.ParseMessages using Docker Model Runner.
func (c *Client) ParseMessages(ctx context.Context, messages []*entity.RawMessage, mappings []*entity.MedicationMapping) ([]*entity.AIParseResult, error) {
	if len(messages) == 0 {
		return nil, nil
	}

	// Apply hybrid filtering if allMappings is configured
	// Use map[string]string for internal filtering operations
	var effectiveMappingsMap map[string]string
	if len(c.allMappings) > 0 {
		// Concatenate all message contents for filtering
		var contentBuilder strings.Builder
		for _, msg := range messages {
			contentBuilder.WriteString(msg.Content)
			contentBuilder.WriteString(" ")
		}
		combinedContent := contentBuilder.String()

		// Hybrid filter: keyword + vector (always combined)
		effectiveMappingsMap = filterMappingsHybrid(ctx, combinedContent, c.allMappings, c, c.vectorTopK)

		c.log.Info().
			Int("original_mappings", len(mappings)).
			Int("filtered_mappings", len(effectiveMappingsMap)).
			Msg("Applied hybrid mapping filter")
	} else {
		// No filtering, use provided mappings as map
		effectiveMappingsMap = medicationMappingsToMap(mappings)
	}

	// Constants for chunking
	const maxBatchSize = 1 // Process 1 chunk per request (safer for context)

	maxMessageLines := c.cfg.MaxMessageLines
	if maxMessageLines <= 0 {
		maxMessageLines = 20 // Default fallback
	}

	// Map unique original ID to list of split chunks
	// We need to preserve the order of original messages
	type splitMap struct {
		originalIdx int
		chunks      []*entity.RawMessage
	}
	originalToChunks := make(map[int]*splitMap) // index -> chunks

	var workingSet []*entity.RawMessage

	for i, msg := range messages {
		// Calculate lines
		lines := strings.Count(msg.Content, "\n")

		if lines <= maxMessageLines {
			workingSet = append(workingSet, msg)
			originalToChunks[i] = &splitMap{originalIdx: i, chunks: []*entity.RawMessage{msg}}
			continue
		}

		// Split massive message
		c.log.Info().
			Str("msg_id", msg.ID).
			Int("lines", lines).
			Msg("Message too long, splitting content")

		contentLines := strings.Split(msg.Content, "\n")
		var chunks []*entity.RawMessage

		for j := 0; j < len(contentLines); j += maxMessageLines {
			end := min(j+maxMessageLines, len(contentLines))

			chunkContent := strings.Join(contentLines[j:end], "\n")
			subMsg := *msg // Shallow copy
			subMsg.Content = chunkContent
			chunks = append(chunks, &subMsg)
			workingSet = append(workingSet, &subMsg)
		}
		originalToChunks[i] = &splitMap{originalIdx: i, chunks: chunks}
	}

	// Create a single flat list of all results
	// Use a map for thread-safe collection, then flatten
	flatResultsMap := make(map[int][]*entity.AIParseResult)
	var mu sync.Mutex
	var wg sync.WaitGroup

	// Limit concurrency to avoid OOM
	concurrencyLimit := 5
	sem := make(chan struct{}, concurrencyLimit)

	c.log.Info().
		Int("original_messages", len(messages)).
		Int("total_chunks", len(workingSet)).
		Int("batch_size", maxBatchSize).
		Int("concurrency", concurrencyLimit).
		Msg("Processing messages in chunks (parallel)")

	for i := 0; i < len(workingSet); i += maxBatchSize {
		end := min(i+maxBatchSize, len(workingSet))
		chunkBatch := workingSet[i:end]
		batchIdx := i // Capture for closure

		wg.Add(1)
		go func(bgIdx int, batch []*entity.RawMessage) {
			defer wg.Done()

			// Acquire semaphore
			sem <- struct{}{}
			defer func() { <-sem }()

			c.log.Debug().
				Int("chunk_start", bgIdx).
				Int("chunk_size", len(batch)).
				Msg("Processing chunk batch")

			results, err := c.processBatch(ctx, batch, mapToMedicationMappings(effectiveMappingsMap))

			mu.Lock()
			defer mu.Unlock()

			if err != nil {
				c.log.Error().Err(err).Int("chunk_start", bgIdx).Msg("Failed to process batch")
				var errResults []*entity.AIParseResult
				for range batch {
					errResults = append(errResults, &entity.AIParseResult{
						Error: fmt.Sprintf("Processing failed: %v", err),
					})
				}
				flatResultsMap[bgIdx] = errResults
			} else {
				flatResultsMap[bgIdx] = results
			}
		}(batchIdx, chunkBatch)
	}

	wg.Wait()

	// Flatten results in order
	var flatResults []*entity.AIParseResult
	// Iterate in steps of maxBatchSize to match the loop above
	for i := 0; i < len(workingSet); i += maxBatchSize {
		if res, ok := flatResultsMap[i]; ok {
			flatResults = append(flatResults, res...)
		} else {
			// Should not happen if logic is correct
			c.log.Error().Int("batch_index", i).Msg("Missing results for batch")
			// Fill with errors to keep alignment
			end := min(i+maxBatchSize, len(workingSet))
			count := end - i
			for range count {
				flatResults = append(flatResults, &entity.AIParseResult{Error: "Internal error: missing batch result"})
			}
		}
	}

	// Now merge flatResults back to original structure
	finalResults := make([]*entity.AIParseResult, len(messages))
	flatIdx := 0

	for i := range messages {
		origMap := originalToChunks[i]
		if origMap == nil {
			// Should not happen
			finalResults[i] = &entity.AIParseResult{Error: "Internal error: missing mapping"}
			continue
		}

		numChunks := len(origMap.chunks)
		if numChunks == 0 {
			finalResults[i] = &entity.AIParseResult{}
			continue
		}

		// Collect results for this message
		mergedResult := &entity.AIParseResult{}
		mergedItems := []entity.ParsedItem{}
		var errors []string

		for j := range numChunks {
			if flatIdx >= len(flatResults) {
				c.log.Error().Msg("Flat results index out of bounds during merge")
				break
			}
			res := flatResults[flatIdx]
			flatIdx++

			if res.Error != "" {
				errors = append(errors, fmt.Sprintf("Chunk %d: %s", j+1, res.Error))
			}
			mergedItems = append(mergedItems, res.Items...)
		}

		mergedResult.Items = mergedItems
		if len(errors) > 0 {
			mergedResult.Error = strings.Join(errors, "; ")
		}
		finalResults[i] = mergedResult
	}

	// Post-process to enforce mappings (Fix for AI hallucinations)
	enforceMappings(ctx, finalResults, effectiveMappingsMap, c.log, c.unmappedRepo, messages)

	return finalResults, nil
}

// enforceMappings iterates over results and strictly applies known mappings
// overwriting the AI's output if a known Arabic term is found in the Raw field.
// Uses Arabic normalization and fuzzy matching for improved accuracy.
// The log parameter enables auto-learn logging for unmapped medications.
// The unmappedRepo (optional) saves unmapped medications for active learning.
func enforceMappings(ctx context.Context, results []*entity.AIParseResult, mappings map[string]string, log zerolog.Logger, unmappedRepo repository.UnmappedMedicationRepo, messages []*entity.RawMessage) {
	if len(mappings) == 0 {
		return
	}

	const maxFuzzyDistance = 2 // Allow up to 2 character edits for fuzzy matching

	// Pre-sort keys by length (longest first) so more specific matches take priority
	// e.g., "ابيجونال" should match before "جونال"
	// This is done once per batch for performance
	sortedKeys := make([]string, 0, len(mappings))
	for k := range mappings {
		sortedKeys = append(sortedKeys, k)
	}
	sort.Slice(sortedKeys, func(i, j int) bool {
		return len(sortedKeys[i]) > len(sortedKeys[j])
	})

	for idx, res := range results {
		// Get message info for context if available
		var sourceMessage, sourceGroup, messageID string
		if idx < len(messages) && messages[idx] != nil {
			sourceMessage = messages[idx].Content
			sourceGroup = messages[idx].GroupName
			messageID = messages[idx].ID
		}

		for i := range res.Items {
			item := &res.Items[i]
			rawNormalized := arabicPkg.NormalizeForMatching(item.MedicationRaw)
			matched := false

			// Step 1: Try exact match (with normalization)
			for _, arabicKey := range sortedKeys {
				english := mappings[arabicKey]
				arabicNormalized := arabicPkg.NormalizeForMatching(arabicKey)
				if strings.Contains(rawNormalized, arabicNormalized) {
					if !strings.Contains(strings.ToLower(item.Medication), strings.ToLower(english)) {
						applyMapping(item, arabicKey, english, fuzzyPkg.ConfidenceExact, log)
						matched = true
						break
					} else {
						matched = true // Already correct
						break
					}
				}
			}

			// Step 2: If no exact match, try fuzzy matching
			if !matched {
				fuzzyResult := fuzzyPkg.FindBest(item.MedicationRaw, mappings, maxFuzzyDistance)
				if fuzzyResult != nil {
					if !strings.Contains(strings.ToLower(item.Medication), strings.ToLower(fuzzyResult.Value)) {
						applyMapping(item, fuzzyResult.Key, fuzzyResult.Value, fuzzyPkg.ConfidenceFuzzy, log)
						matched = true
					}
				}
			}

			// Step 3: Auto-learn - log and persist unmapped medications for review
			if !matched && item.MedicationRaw != "" {
				log.Warn().
					Str("raw", item.MedicationRaw).
					Str("ai_output", item.Medication).
					Msg("Unmapped medication detected - consider adding to database")
				item.MatchConfidence = string(fuzzyPkg.ConfidenceTransliterated)

				// Save to DB for active learning review queue
				if unmappedRepo != nil {
					if err := unmappedRepo.Save(ctx, item.MedicationRaw, item.Medication, sourceMessage, sourceGroup, messageID); err != nil {
						log.Error().Err(err).Str("raw", item.MedicationRaw).Msg("Failed to save unmapped medication")
					}
				}
			}
		}
	}
}

// applyMapping updates an item with the correct English mapping
func applyMapping(item *entity.ParsedItem, arabic, english string, confidence fuzzyPkg.MatchConfidence, log zerolog.Logger) {
	// Try to replace Arabic with English in the raw text
	newItem := strings.ReplaceAll(item.MedicationRaw, arabic, english)

	// Set the match confidence
	item.MatchConfidence = string(confidence)

	if newItem == item.MedicationRaw {
		// Replace failed, just use the English name
		item.Medication = english
		log.Debug().
			Str("raw", item.MedicationRaw).
			Str("mapped_to", english).
			Str("confidence", string(confidence)).
			Msg("Mapping forced (no replacement possible)")
	} else {
		// Check for mixed Arabic/English content - if detected, use pure English name
		if containsArabic(newItem) {
			item.Medication = english
			log.Debug().
				Str("raw", item.MedicationRaw).
				Str("attempted", newItem).
				Str("mapped_to", english).
				Str("confidence", string(confidence)).
				Msg("Mixed content detected, using pure English")
		} else {
			item.Medication = newItem
			log.Debug().
				Str("raw", item.MedicationRaw).
				Str("mapped_to", newItem).
				Str("confidence", string(confidence)).
				Msg("Mapping applied")
		}
	}
}

// containsArabic checks if a string contains any Arabic characters
func containsArabic(s string) bool {
	for _, r := range s {
		// Arabic Unicode range: U+0600 to U+06FF
		if r >= 0x0600 && r <= 0x06FF {
			return true
		}
	}
	return false
}

// processBatch processes a small batch of messages
func (c *Client) processBatch(ctx context.Context, messages []*entity.RawMessage, mappings []*entity.MedicationMapping) ([]*entity.AIParseResult, error) {
	// Build prompt with messages
	prompt := prompts.BuildParsePrompt(messages, mappings)

	// Count tokens
	tokenCount, err := textPkg.CountTokens(prompt)
	if err != nil {
		c.log.Warn().Err(err).Msg("Failed to count tokens")
	}

	// Log mapping specific tokens
	if len(mappings) > 0 {
		mappingStr := prompts.FormatMappings(mappings)
		mappingTokens, _ := textPkg.CountTokens(mappingStr)
		c.log.Info().Int("mapping_tokens", mappingTokens).Int("total_prompt_tokens", tokenCount).Msg("Token usage breakdown")
	}

	c.log.Info().
		Int("message_count", len(messages)).
		Int("tokens", tokenCount).
		Msg("Sending messages to Docker Model Runner")

	metrics.AITokensUsed.Observe(float64(tokenCount))

	// Create timeout context
	ctx, cancel := context.WithTimeout(ctx, c.cfg.RequestTimeout)
	defer cancel()

	// Retry logic with exponential backoff wrapped in Circuit Breaker
	var result *openai.ChatCompletion
	start := time.Now()

	cbResult, err := c.cb.Execute(func() (interface{}, error) {
		var apiResult *openai.ChatCompletion
		var lastErr error

		for attempt := 1; attempt <= c.cfg.MaxRetries; attempt++ {
			apiResult, lastErr = c.client.Chat.Completions.New(ctx, openai.ChatCompletionNewParams{
				Model: c.cfg.Model,
				Messages: []openai.ChatCompletionMessageParamUnion{
					openai.UserMessage(prompt), // prompt already includes system instructions
				},
				ResponseFormat: openai.ChatCompletionNewParamsResponseFormatUnion{
					OfJSONSchema: &openai.ResponseFormatJSONSchemaParam{
						JSONSchema: openai.ResponseFormatJSONSchemaJSONSchemaParam{
							Name:        "pharma_parsing",
							Description: openai.String("Extract medication offers and requests"),
							Schema:      aiParseResultSchema,
							Strict:      openai.Bool(true),
						},
					},
				},
				Temperature: openai.Float(0.1), // Low temperature for consistent parsing
			})

			if lastErr == nil {
				return apiResult, nil
			}

			c.log.Warn().
				Err(lastErr).
				Int("attempt", attempt).
				Int("max_retries", c.cfg.MaxRetries).
				Msg("Docker Model Runner API call failed, retrying...")

			if attempt < c.cfg.MaxRetries {
				delay := c.cfg.RetryBaseDelay * time.Duration(1<<(attempt-1))
				select {
				case <-ctx.Done():
					return nil, fmt.Errorf("context cancelled during retry: %w", ctx.Err())
				case <-time.After(delay):
				}
			}
		}
		return nil, fmt.Errorf("failed after %d attempts: %w", c.cfg.MaxRetries, lastErr)
	})

	metrics.AIRequestDuration.WithLabelValues(func() string {
		if err != nil {
			return "error"
		}
		return "success"
	}()).Observe(time.Since(start).Seconds())

	if err != nil {
		return nil, err
	}
	result = cbResult.(*openai.ChatCompletion)

	// Extract response content
	if len(result.Choices) == 0 || result.Choices[0].Message.Content == "" {
		c.log.Warn().Msg("Empty response from Docker Model Runner")
		return nil, nil
	}

	responseText := result.Choices[0].Message.Content

	c.log.Debug().
		Int("response_length", len(responseText)).
		Msg("Received response from Docker Model Runner")

	// Parse JSON response - handle both formats:
	// 1. Single object: {"items": [...]} (what AI typically returns)
	// 2. Array: [{"items": [...]}, ...] (original expectation)
	var parseResults []*entity.AIParseResult

	// First try: single AIParseResult object (most common from structured output)
	var singleResult entity.AIParseResult
	if err := json.Unmarshal([]byte(responseText), &singleResult); err == nil && singleResult.Items != nil {
		// Successfully parsed as single object with items field (even if empty)
		parseResults = []*entity.AIParseResult{&singleResult}
	} else {
		// Second try: array of AIParseResult
		if err := json.Unmarshal([]byte(responseText), &parseResults); err != nil {
			c.log.Error().Err(err).Str("content", truncateForLog(responseText, 500)).Msg("Failed to unmarshal AI response")
			return nil, fmt.Errorf("failed to parse AI response: %w", err)
		}
	}

	c.log.Info().
		Int("parsed_count", len(parseResults)).
		Int("items_count", countTotalItems(parseResults)).
		Msg("Chunk parsed successfully")

	return parseResults, nil
}

// countTotalItems counts total items in results
func countTotalItems(results []*entity.AIParseResult) int {
	count := 0
	for _, res := range results {
		count += len(res.Items)
	}
	return count
}

// truncateForLog truncates a string for logging purposes
func truncateForLog(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

// Embed generates embeddings using the OpenAI-compatible endpoint
func (c *Client) Embed(ctx context.Context, text string) ([]float32, error) {
	// Create a separate client for embeddings if the base URL differs
	client := c.client

	// OpenAI 'Creating Embeddings' API
	model := c.cfg.EmbeddingModelName
	if model == "" {
		model = "ai/embeddinggemma" // Fallback default
	}

	resp, err := client.Embeddings.New(ctx, openai.EmbeddingNewParams{
		Model: openai.EmbeddingModel(model),
		Input: openai.EmbeddingNewParamsInputUnion{
			OfString: openai.String(text),
		},
	})
	if err != nil {
		c.log.Error().Err(err).Msg("Failed to generate embedding")
		return nil, err
	}

	if len(resp.Data) == 0 {
		return nil, fmt.Errorf("empty embedding response")
	}

	// Convert float64 to float32
	vec64 := resp.Data[0].Embedding
	vec32 := make([]float32, len(vec64))
	for i, v := range vec64 {
		vec32[i] = float32(v)
	}

	return vec32, nil
}

// EmbedBatch generates embeddings for a batch of texts using the OpenAI-compatible endpoint
func (c *Client) EmbedBatch(ctx context.Context, texts []string) ([][]float32, error) {
	if len(texts) == 0 {
		return nil, nil
	}

	client := c.client
	model := c.cfg.EmbeddingModelName
	if model == "" {
		model = "ai/embeddinggemma"
	}

	resp, err := client.Embeddings.New(ctx, openai.EmbeddingNewParams{
		Model: openai.EmbeddingModel(model),
		Input: openai.EmbeddingNewParamsInputUnion{
			OfArrayOfStrings: texts,
		},
	})
	if err != nil {
		c.log.Error().Err(err).Int("batch_size", len(texts)).Msg("Failed to generate batch embeddings")
		return nil, err
	}

	if len(resp.Data) == 0 {
		return nil, fmt.Errorf("empty embedding response")
	}

	// Sort response by Index to ensure order matches input
	// OpenAI guarantees order usually, but explicit sorting is safer if indices are provided.
	// Actually, the SDK returns a slice in order. We map it directly.
	results := make([][]float32, len(texts))
	for _, item := range resp.Data {
		if int(item.Index) < len(results) {
			vec64 := item.Embedding
			vec32 := make([]float32, len(vec64))
			for i, v := range vec64 {
				vec32[i] = float32(v)
			}
			results[item.Index] = vec32
		}
	}

	return results, nil
}

// ---- Mapping filter utilities (inlined to avoid circular imports) ----

// embedder interface for generating text embeddings
type embedder interface {
	Embed(ctx context.Context, text string) ([]float32, error)
}

// filterMappingsByKeyword returns only mappings where Arabic key appears in content
func filterMappingsByKeyword(content string, mappings map[string]string) map[string]string {
	result := make(map[string]string)
	contentNormalized := arabicPkg.NormalizeForMatching(content)

	for arabicKey, english := range mappings {
		arabicNormalized := arabicPkg.NormalizeForMatching(arabicKey)
		if strings.Contains(contentNormalized, arabicNormalized) {
			result[arabicKey] = english
		}
	}

	return result
}

// filterMappingsBySimilarity returns top-K semantically similar mappings
func filterMappingsBySimilarity(ctx context.Context, content string, allMappings []*entity.MedicationMapping, emb embedder, topK int) (map[string]string, error) {
	if len(allMappings) == 0 || topK <= 0 {
		return make(map[string]string), nil
	}

	// Embed the message content
	contentEmbedding, err := emb.Embed(ctx, content)
	if err != nil {
		return nil, err
	}

	// Score all mappings by similarity
	type scoredMapping struct {
		mapping *entity.MedicationMapping
		score   float32
	}
	var scored []scoredMapping

	for _, m := range allMappings {
		if len(m.Embedding) == 0 {
			continue
		}
		score := cosineSimilarity(contentEmbedding, m.Embedding)
		scored = append(scored, scoredMapping{m, score})
	}

	// Sort by score descending
	sort.Slice(scored, func(i, j int) bool {
		return scored[i].score > scored[j].score
	})

	// Take top-K
	result := make(map[string]string)
	for i := 0; i < topK && i < len(scored); i++ {
		m := scored[i].mapping
		result[m.ArabicName] = m.EnglishName
	}

	return result, nil
}

// filterMappingsHybrid combines keyword matching and vector similarity
func filterMappingsHybrid(ctx context.Context, content string, allMappings []*entity.MedicationMapping, emb embedder, vectorTopK int) map[string]string {
	// Build map for keyword filtering
	fullMap := make(map[string]string)
	for _, m := range allMappings {
		fullMap[m.ArabicName] = m.EnglishName
		// Include synonyms
		for _, syn := range m.Synonyms {
			fullMap[syn] = m.EnglishName
		}
	}

	// Step 1: Keyword filtering (always)
	result := filterMappingsByKeyword(content, fullMap)

	// Step 2: Vector similarity (always add top-K)
	vectorMatches, err := filterMappingsBySimilarity(ctx, content, allMappings, emb, vectorTopK)
	if err == nil {
		// Merge vector matches into result (deduped by map key)
		for arabic, english := range vectorMatches {
			if _, exists := result[arabic]; !exists {
				result[arabic] = english
			}
		}
	}

	return result
}

// cosineSimilarity calculates cosine similarity between two vectors
func cosineSimilarity(a, b []float32) float32 {
	if len(a) != len(b) || len(a) == 0 {
		return 0
	}

	var dot, normA, normB float32
	for i := range a {
		dot += a[i] * b[i]
		normA += a[i] * a[i]
		normB += b[i] * b[i]
	}

	if normA == 0 || normB == 0 {
		return 0
	}

	return dot / (float32(math.Sqrt(float64(normA))) * float32(math.Sqrt(float64(normB))))
}

// mapToMedicationMappings converts map[string]string to []*entity.MedicationMapping
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

// medicationMappingsToMap converts []*entity.MedicationMapping to map[string]string
func medicationMappingsToMap(mappings []*entity.MedicationMapping) map[string]string {
	result := make(map[string]string)
	for _, m := range mappings {
		result[m.ArabicName] = m.EnglishName
	}
	return result
}
