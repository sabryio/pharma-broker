package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"
	"unicode"

	"github.com/invopop/jsonschema"
	"github.com/openai/openai-go"
	"github.com/openai/openai-go/option"
	"github.com/rs/zerolog"
	"github.com/sony/gobreaker"

	"pharmabroker/ai/prompts"
	arabicPkg "pharmabroker/pkg/arabic"
	"pharmabroker/pkg/config"
	fuzzyPkg "pharmabroker/pkg/fuzzy"
	"pharmabroker/pkg/metrics"
	textPkg "pharmabroker/pkg/text"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/pkg/matcher/filtering"
	"pharmabroker/pkg/matcher/similarity"
)

// Arabic Unicode range table for character detection
var arabicRange = &unicode.RangeTable{
	R16: []unicode.Range16{{Lo: 0x0600, Hi: 0x06FF, Stride: 1}},
}

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

// splitMap tracks the relationship between original messages and their chunks
type splitMap struct {
	originalIdx int
	chunks      []*entity.RawMessage
}

// mappingEnforcementContext holds all parameters needed for mapping enforcement
type mappingEnforcementContext struct {
	results      []*entity.AIParseResult
	mappings     map[string]string
	log          zerolog.Logger
	unmappedRepo repository.UnmappedMedicationRepo
	messages     []*entity.RawMessage
}

// GenerateSchema generates a JSON schema for the given type using reflection.
// The schema is used for structured output from the AI model.
func GenerateSchema[T any]() any {
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

	cb := newCircuitBreaker(cfg, log)

	return &Client{
		cfg:        cfg,
		client:     client,
		log:        log.With().Str("component", "docker-model").Logger(),
		cb:         cb,
		vectorTopK: DefaultVectorTopK,
	}, nil
}

// newCircuitBreaker creates a configured circuit breaker with sensible defaults
func newCircuitBreaker(cfg *config.DockerModelConfig, log zerolog.Logger) *gobreaker.CircuitBreaker {
	cbMaxRequests := cfg.CBMaxRequests
	if cbMaxRequests == 0 {
		cbMaxRequests = DefaultCBMaxRequests
	}
	cbInterval := cfg.CBInterval
	if cbInterval == 0 {
		cbInterval = DefaultCBInterval
	}
	cbTimeout := cfg.CBTimeout
	if cbTimeout == 0 {
		cbTimeout = DefaultCBTimeout
	}
	cbFailureRatio := cfg.CBFailureRatio
	if cbFailureRatio == 0 {
		cbFailureRatio = DefaultCBFailureRatio
	}

	st := gobreaker.Settings{
		Name:        "DockerModel",
		MaxRequests: cbMaxRequests,
		Interval:    cbInterval,
		Timeout:     cbTimeout,
		ReadyToTrip: func(counts gobreaker.Counts) bool {
			failureRatio := float64(counts.TotalFailures) / float64(counts.Requests)
			return counts.Requests >= DefaultCBMinRequests && failureRatio >= cbFailureRatio
		},
		OnStateChange: func(name string, from gobreaker.State, to gobreaker.State) {
			log.Warn().Str("name", name).Str("from", from.String()).Str("to", to.String()).Msg("Circuit Breaker state changed")
		},
	}

	return gobreaker.NewCircuitBreaker(st)
}

// SetMappings sets the full medication mappings for hybrid filtering.
// This enables keyword + vector search when ParseMessages is called.
func (c *Client) SetMappings(mappings []*entity.MedicationMapping) {
	c.allMappings = mappings
	if c.vectorTopK == 0 {
		c.vectorTopK = DefaultVectorTopK
	}
	c.log.Info().Int("count", len(mappings)).Msg("Loaded medication mappings for hybrid filtering")
}

// SetUnmappedRepo sets the repository for saving unmapped medications (active learning).
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
	effectiveMappingsMap := c.getEffectiveMappings(ctx, messages, mappings)

	// Split large messages and prepare working set
	workingSet, originalToChunks := c.splitLargeMessages(messages)

	// Process chunks in parallel
	flatResultsMap := c.processChunksParallel(ctx, workingSet, effectiveMappingsMap)

	// Flatten and merge results back to original structure
	finalResults := c.mergeChunkResults(messages, workingSet, originalToChunks, flatResultsMap)

	// Post-process to enforce mappings (Fix for AI hallucinations)
	enforceMappings(ctx, &mappingEnforcementContext{
		results:      finalResults,
		mappings:     effectiveMappingsMap,
		log:          c.log,
		unmappedRepo: c.unmappedRepo,
		messages:     messages,
	})

	return finalResults, nil
}

// getEffectiveMappings applies hybrid filtering if allMappings is configured,
// otherwise returns the provided mappings as a map.
func (c *Client) getEffectiveMappings(ctx context.Context, messages []*entity.RawMessage, mappings []*entity.MedicationMapping) map[string]string {
	if len(c.allMappings) == 0 {
		return filtering.MappingsEntityToMap(mappings)
	}

	// Concatenate all message contents for filtering
	combinedContent := c.buildCombinedContent(messages)

	// Hybrid filter: keyword + vector (always combined)
	effectiveMappingsMap := filterMappingsHybrid(ctx, combinedContent, c.allMappings, c, c.vectorTopK)

	c.log.Info().
		Int("original_mappings", len(mappings)).
		Int("filtered_mappings", len(effectiveMappingsMap)).
		Msg("Applied hybrid mapping filter")

	return effectiveMappingsMap
}

// buildCombinedContent concatenates all message contents with pre-allocated capacity.
func (c *Client) buildCombinedContent(messages []*entity.RawMessage) string {
	// Pre-calculate total size for efficient allocation
	totalSize := 0
	for _, msg := range messages {
		totalSize += len(msg.Content) + 1 // +1 for space separator
	}

	var contentBuilder strings.Builder
	contentBuilder.Grow(totalSize)

	for _, msg := range messages {
		contentBuilder.WriteString(msg.Content)
		contentBuilder.WriteString(" ")
	}

	return contentBuilder.String()
}

// splitLargeMessages splits messages that exceed maxMessageLines into smaller chunks.
// Returns the working set of all chunks and a map tracking original message to chunks.
func (c *Client) splitLargeMessages(messages []*entity.RawMessage) ([]*entity.RawMessage, map[int]*splitMap) {
	maxMessageLines := c.cfg.MaxMessageLines
	if maxMessageLines <= 0 {
		maxMessageLines = DefaultMaxMessageLines
	}

	originalToChunks := make(map[int]*splitMap, len(messages))
	var workingSet []*entity.RawMessage

	for i, msg := range messages {
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

		chunks := c.splitMessageIntoChunks(msg, maxMessageLines)
		workingSet = append(workingSet, chunks...)
		originalToChunks[i] = &splitMap{originalIdx: i, chunks: chunks}
	}

	return workingSet, originalToChunks
}

// splitMessageIntoChunks splits a single message into chunks of maxLines each.
func (c *Client) splitMessageIntoChunks(msg *entity.RawMessage, maxLines int) []*entity.RawMessage {
	contentLines := strings.Split(msg.Content, "\n")
	var chunks []*entity.RawMessage

	for j := 0; j < len(contentLines); j += maxLines {
		end := min(j+maxLines, len(contentLines))
		chunkContent := strings.Join(contentLines[j:end], "\n")

		subMsg := *msg // Shallow copy
		subMsg.Content = chunkContent
		chunks = append(chunks, &subMsg)
	}

	return chunks
}

// processChunksParallel processes all chunks in parallel with bounded concurrency.
// Returns a map of batch index to results for ordered reconstruction.
func (c *Client) processChunksParallel(ctx context.Context, workingSet []*entity.RawMessage, effectiveMappingsMap map[string]string) map[int][]*entity.AIParseResult {
	flatResultsMap := make(map[int][]*entity.AIParseResult)
	var mu sync.Mutex
	var wg sync.WaitGroup

	sem := make(chan struct{}, DefaultConcurrencyLimit)

	c.log.Info().
		Int("total_chunks", len(workingSet)).
		Int("batch_size", DefaultMaxBatchSize).
		Int("concurrency", DefaultConcurrencyLimit).
		Msg("Processing messages in chunks (parallel)")

	mappingsSlice := filtering.MapToMappingsEntity(effectiveMappingsMap)

	for i := 0; i < len(workingSet); i += DefaultMaxBatchSize {
		end := min(i+DefaultMaxBatchSize, len(workingSet))
		chunkBatch := workingSet[i:end]
		batchIdx := i

		wg.Add(1)
		go func(bgIdx int, batch []*entity.RawMessage) {
			defer wg.Done()

			// Check context before acquiring semaphore to fail fast
			select {
			case <-ctx.Done():
				mu.Lock()
				flatResultsMap[bgIdx] = c.createErrorResults(batch, ctx.Err().Error())
				mu.Unlock()
				return
			case sem <- struct{}{}:
			}
			defer func() { <-sem }()

			c.log.Debug().
				Int("chunk_start", bgIdx).
				Int("chunk_size", len(batch)).
				Msg("Processing chunk batch")

			results, err := c.processBatch(ctx, batch, mappingsSlice)

			mu.Lock()
			defer mu.Unlock()

			if err != nil {
				c.log.Error().Err(err).Int("chunk_start", bgIdx).Msg("Failed to process batch")
				flatResultsMap[bgIdx] = c.createErrorResults(batch, fmt.Sprintf("Processing failed: %v", err))
			} else {
				flatResultsMap[bgIdx] = results
			}
		}(batchIdx, chunkBatch)
	}

	wg.Wait()
	return flatResultsMap
}

// createErrorResults creates error results for a batch of messages.
func (c *Client) createErrorResults(batch []*entity.RawMessage, errMsg string) []*entity.AIParseResult {
	errResults := make([]*entity.AIParseResult, len(batch))
	for i := range batch {
		errResults[i] = &entity.AIParseResult{Error: errMsg}
	}
	return errResults
}

// mergeChunkResults flattens parallel results and merges chunks back to original messages.
func (c *Client) mergeChunkResults(messages []*entity.RawMessage, workingSet []*entity.RawMessage, originalToChunks map[int]*splitMap, flatResultsMap map[int][]*entity.AIParseResult) []*entity.AIParseResult {
	// Flatten results in order
	flatResults := c.flattenResults(workingSet, flatResultsMap)

	// Merge back to original structure
	finalResults := make([]*entity.AIParseResult, len(messages))
	flatIdx := 0

	for i := range messages {
		origMap := originalToChunks[i]
		if origMap == nil {
			finalResults[i] = &entity.AIParseResult{Error: "Internal error: missing mapping"}
			continue
		}

		numChunks := len(origMap.chunks)
		if numChunks == 0 {
			finalResults[i] = &entity.AIParseResult{}
			continue
		}

		finalResults[i], flatIdx = c.mergeChunksForMessage(flatResults, flatIdx, numChunks)
	}

	return finalResults
}

// flattenResults converts the map of batch results into an ordered slice.
func (c *Client) flattenResults(workingSet []*entity.RawMessage, flatResultsMap map[int][]*entity.AIParseResult) []*entity.AIParseResult {
	var flatResults []*entity.AIParseResult

	for i := 0; i < len(workingSet); i += DefaultMaxBatchSize {
		if res, ok := flatResultsMap[i]; ok {
			flatResults = append(flatResults, res...)
		} else {
			c.log.Error().Int("batch_index", i).Msg("Missing results for batch")
			end := min(i+DefaultMaxBatchSize, len(workingSet))
			count := end - i
			for range count {
				flatResults = append(flatResults, &entity.AIParseResult{Error: "Internal error: missing batch result"})
			}
		}
	}

	return flatResults
}

// mergeChunksForMessage merges multiple chunk results into a single result for one message.
func (c *Client) mergeChunksForMessage(flatResults []*entity.AIParseResult, startIdx, numChunks int) (*entity.AIParseResult, int) {
	mergedResult := &entity.AIParseResult{}
	var mergedItems []entity.ParsedItem
	var errors []string

	flatIdx := startIdx
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

	return mergedResult, flatIdx
}

// enforceMappings iterates over results and strictly applies known mappings,
// overwriting the AI's output if a known Arabic term is found in the Raw field.
// Uses Arabic normalization and fuzzy matching for improved accuracy.
func enforceMappings(ctx context.Context, ec *mappingEnforcementContext) {
	if len(ec.mappings) == 0 {
		return
	}

	// Pre-sort keys by length (longest first) so more specific matches take priority
	// e.g., "ابيجونال" should match before "جونال"
	sortedKeys := getSortedMappingKeys(ec.mappings)

	for idx, res := range ec.results {
		sourceInfo := getSourceInfo(ec.messages, idx)
		processResultItems(ctx, res, sortedKeys, ec, sourceInfo)
	}
}

// sourceInfo holds context information about the source message
type sourceInfo struct {
	message   string
	groupName string
	messageID string
}

// getSourceInfo extracts source information from messages if available.
func getSourceInfo(messages []*entity.RawMessage, idx int) sourceInfo {
	if idx < len(messages) && messages[idx] != nil {
		return sourceInfo{
			message:   messages[idx].Content,
			groupName: messages[idx].GroupName,
			messageID: messages[idx].ID,
		}
	}
	return sourceInfo{}
}

// getSortedMappingKeys returns mapping keys sorted by length (longest first).
func getSortedMappingKeys(mappings map[string]string) []string {
	sortedKeys := make([]string, 0, len(mappings))
	for k := range mappings {
		sortedKeys = append(sortedKeys, k)
	}
	sort.Slice(sortedKeys, func(i, j int) bool {
		return len(sortedKeys[i]) > len(sortedKeys[j])
	})
	return sortedKeys
}

// processResultItems processes all items in a result, applying mappings as needed.
func processResultItems(ctx context.Context, res *entity.AIParseResult, sortedKeys []string, ec *mappingEnforcementContext, source sourceInfo) {
	for i := range res.Items {
		item := &res.Items[i]
		matched := tryMatchItem(item, sortedKeys, ec.mappings, ec.log)

		// Auto-learn - log and persist unmapped medications for review
		if !matched && item.MedicationRaw != "" {
			handleUnmappedMedication(ctx, item, ec, source)
		}
	}
}

// tryMatchItem attempts to match an item using exact and fuzzy matching.
// Returns true if a match was found.
func tryMatchItem(item *entity.ParsedItem, sortedKeys []string, mappings map[string]string, log zerolog.Logger) bool {
	rawNormalized := arabicPkg.NormalizeForMatching(item.MedicationRaw)

	// Step 1: Try exact match (with normalization)
	if matched := tryExactMatch(item, rawNormalized, sortedKeys, mappings, log); matched {
		return true
	}

	// Step 2: Try fuzzy matching
	return tryFuzzyMatch(item, mappings, log)
}

// tryExactMatch attempts exact matching with Arabic normalization.
func tryExactMatch(item *entity.ParsedItem, rawNormalized string, sortedKeys []string, mappings map[string]string, log zerolog.Logger) bool {
	for _, arabicKey := range sortedKeys {
		english := mappings[arabicKey]
		arabicNormalized := arabicPkg.NormalizeForMatching(arabicKey)

		if strings.Contains(rawNormalized, arabicNormalized) {
			if !strings.Contains(strings.ToLower(item.Medication), strings.ToLower(english)) {
				applyMapping(item, arabicKey, english, fuzzyPkg.ConfidenceExact, log)
			}
			return true // Match found (either applied or already correct)
		}
	}
	return false
}

// tryFuzzyMatch attempts fuzzy matching for items that didn't match exactly.
func tryFuzzyMatch(item *entity.ParsedItem, mappings map[string]string, log zerolog.Logger) bool {
	fuzzyResult := fuzzyPkg.FindBest(item.MedicationRaw, mappings, MaxFuzzyDistance)
	if fuzzyResult == nil {
		return false
	}

	if !strings.Contains(strings.ToLower(item.Medication), strings.ToLower(fuzzyResult.Value)) {
		applyMapping(item, fuzzyResult.Key, fuzzyResult.Value, fuzzyPkg.ConfidenceFuzzy, log)
	}
	return true
}

// handleUnmappedMedication logs and persists unmapped medications for active learning.
func handleUnmappedMedication(ctx context.Context, item *entity.ParsedItem, ec *mappingEnforcementContext, source sourceInfo) {
	ec.log.Warn().
		Str("raw", item.MedicationRaw).
		Str("ai_output", item.Medication).
		Msg("Unmapped medication detected - consider adding to database")

	item.MatchConfidence = string(fuzzyPkg.ConfidenceTransliterated)

	// Save to DB for active learning review queue
	if ec.unmappedRepo != nil {
		if err := ec.unmappedRepo.Save(ctx, item.MedicationRaw, item.Medication, source.message, source.groupName, source.messageID); err != nil {
			ec.log.Error().Err(err).Str("raw", item.MedicationRaw).Msg("Failed to save unmapped medication")
		}
	}
}

// applyMapping updates an item with the correct English mapping.
func applyMapping(item *entity.ParsedItem, arabic, english string, confidence fuzzyPkg.MatchConfidence, log zerolog.Logger) {
	// Try to replace Arabic with English in the raw text
	newItem := strings.ReplaceAll(item.MedicationRaw, arabic, english)
	item.MatchConfidence = string(confidence)

	if newItem == item.MedicationRaw {
		// Replace failed, just use the English name
		item.Medication = english
		log.Debug().
			Str("raw", item.MedicationRaw).
			Str("mapped_to", english).
			Str("confidence", string(confidence)).
			Msg("Mapping forced (no replacement possible)")
		return
	}

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

// containsArabic checks if a string contains any Arabic characters.
// Uses unicode.Is for idiomatic character range checking.
func containsArabic(s string) bool {
	for _, r := range s {
		if unicode.Is(arabicRange, r) {
			return true
		}
	}
	return false
}

// processBatch processes a small batch of messages through the AI model.
func (c *Client) processBatch(ctx context.Context, messages []*entity.RawMessage, mappings []*entity.MedicationMapping) ([]*entity.AIParseResult, error) {
	prompt := prompts.BuildParsePrompt(messages, mappings)

	c.logTokenUsage(prompt, mappings)

	// Create timeout context
	ctx, cancel := context.WithTimeout(ctx, c.cfg.RequestTimeout)
	defer cancel()

	result, err := c.executeWithCircuitBreaker(ctx, prompt)
	if err != nil {
		return nil, err
	}

	return c.parseResponse(result)
}

// logTokenUsage logs token usage for monitoring.
func (c *Client) logTokenUsage(prompt string, mappings []*entity.MedicationMapping) {
	tokenCount, err := textPkg.CountTokens(prompt)
	if err != nil {
		c.log.Warn().Err(err).Msg("Failed to count tokens")
		return
	}

	if len(mappings) > 0 {
		mappingStr := prompts.FormatMappings(mappings)
		mappingTokens, _ := textPkg.CountTokens(mappingStr)
		c.log.Info().Int("mapping_tokens", mappingTokens).Int("total_prompt_tokens", tokenCount).Msg("Token usage breakdown")
	}

	c.log.Info().
		Int("message_count", len(mappings)).
		Int("tokens", tokenCount).
		Msg("Sending messages to Docker Model Runner")

	metrics.AITokensUsed.Observe(float64(tokenCount))
}

// executeWithCircuitBreaker executes the API call with circuit breaker and retry logic.
func (c *Client) executeWithCircuitBreaker(ctx context.Context, prompt string) (*openai.ChatCompletion, error) {
	start := time.Now()

	cbResult, err := c.cb.Execute(func() (any, error) {
		return c.executeWithRetry(ctx, prompt)
	})

	c.recordMetrics(start, err)

	if err != nil {
		return nil, err
	}

	return cbResult.(*openai.ChatCompletion), nil
}

// executeWithRetry executes the API call with exponential backoff retry.
func (c *Client) executeWithRetry(ctx context.Context, prompt string) (*openai.ChatCompletion, error) {
	var lastErr error

	for attempt := 1; attempt <= c.cfg.MaxRetries; attempt++ {
		result, err := c.client.Chat.Completions.New(ctx, openai.ChatCompletionNewParams{
			Model: c.cfg.Model,
			Messages: []openai.ChatCompletionMessageParamUnion{
				openai.UserMessage(prompt),
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

		if err == nil {
			return result, nil
		}

		lastErr = err
		c.log.Warn().
			Err(err).
			Int("attempt", attempt).
			Int("max_retries", c.cfg.MaxRetries).
			Msg("Docker Model Runner API call failed, retrying...")

		if attempt < c.cfg.MaxRetries {
			if err := c.waitWithBackoff(ctx, attempt); err != nil {
				return nil, err
			}
		}
	}

	return nil, fmt.Errorf("failed after %d attempts: %w", c.cfg.MaxRetries, lastErr)
}

// waitWithBackoff waits with exponential backoff, respecting context cancellation.
func (c *Client) waitWithBackoff(ctx context.Context, attempt int) error {
	delay := c.cfg.RetryBaseDelay * time.Duration(1<<(attempt-1))
	select {
	case <-ctx.Done():
		return fmt.Errorf("context cancelled during retry: %w", ctx.Err())
	case <-time.After(delay):
		return nil
	}
}

// recordMetrics records request duration metrics.
func (c *Client) recordMetrics(start time.Time, err error) {
	status := "success"
	if err != nil {
		status = "error"
	}
	metrics.AIRequestDuration.WithLabelValues(status).Observe(time.Since(start).Seconds())
}

// parseResponse parses the AI response into structured results.
func (c *Client) parseResponse(result *openai.ChatCompletion) ([]*entity.AIParseResult, error) {
	if len(result.Choices) == 0 || result.Choices[0].Message.Content == "" {
		c.log.Warn().Msg("Empty response from Docker Model Runner")
		return nil, nil
	}

	responseText := result.Choices[0].Message.Content

	c.log.Debug().
		Int("response_length", len(responseText)).
		Msg("Received response from Docker Model Runner")

	parseResults, err := unmarshalAIResponse(responseText)
	if err != nil {
		c.log.Error().Err(err).Str("content", truncateForLog(responseText, MaxLogTruncateLen)).Msg("Failed to unmarshal AI response")
		return nil, fmt.Errorf("failed to parse AI response: %w", err)
	}

	c.log.Info().
		Int("parsed_count", len(parseResults)).
		Int("items_count", countTotalItems(parseResults)).
		Msg("Chunk parsed successfully")

	return parseResults, nil
}

// unmarshalAIResponse handles both single object and array response formats.
func unmarshalAIResponse(responseText string) ([]*entity.AIParseResult, error) {
	// First try: single AIParseResult object (most common from structured output)
	var singleResult entity.AIParseResult
	if err := json.Unmarshal([]byte(responseText), &singleResult); err == nil && singleResult.Items != nil {
		return []*entity.AIParseResult{&singleResult}, nil
	}

	// Second try: array of AIParseResult
	var parseResults []*entity.AIParseResult
	if err := json.Unmarshal([]byte(responseText), &parseResults); err != nil {
		return nil, err
	}

	return parseResults, nil
}

// countTotalItems counts total items across all results.
func countTotalItems(results []*entity.AIParseResult) int {
	count := 0
	for _, res := range results {
		count += len(res.Items)
	}
	return count
}

// truncateForLog truncates a string for logging purposes.
func truncateForLog(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

// Embed generates embeddings using the OpenAI-compatible endpoint.
func (c *Client) Embed(ctx context.Context, text string) ([]float32, error) {
	model := c.getEmbeddingModel()

	resp, err := c.client.Embeddings.New(ctx, openai.EmbeddingNewParams{
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

	return convertFloat64ToFloat32(resp.Data[0].Embedding), nil
}

// EmbedBatch generates embeddings for a batch of texts using the OpenAI-compatible endpoint.
func (c *Client) EmbedBatch(ctx context.Context, texts []string) ([][]float32, error) {
	if len(texts) == 0 {
		return nil, nil
	}

	model := c.getEmbeddingModel()

	resp, err := c.client.Embeddings.New(ctx, openai.EmbeddingNewParams{
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

	return c.mapEmbeddingResponse(resp.Data, len(texts)), nil
}

// getEmbeddingModel returns the configured embedding model or the default.
func (c *Client) getEmbeddingModel() string {
	if c.cfg.EmbeddingModelName != "" {
		return c.cfg.EmbeddingModelName
	}
	return DefaultEmbeddingModel
}

// mapEmbeddingResponse maps the embedding response to the correct indices.
func (c *Client) mapEmbeddingResponse(data []openai.Embedding, expectedLen int) [][]float32 {
	results := make([][]float32, expectedLen)

	for _, item := range data {
		idx := int(item.Index)
		if idx >= expectedLen {
			c.log.Warn().Int("index", idx).Int("expected_len", expectedLen).Msg("Unexpected embedding index")
			continue
		}
		results[idx] = convertFloat64ToFloat32(item.Embedding)
	}

	return results
}

// convertFloat64ToFloat32 converts a slice of float64 to float32.
func convertFloat64ToFloat32(vec64 []float64) []float32 {
	vec32 := make([]float32, len(vec64))
	for i, v := range vec64 {
		vec32[i] = float32(v)
	}
	return vec32
}

// ---- Mapping filter utilities ----

// embedder interface for generating text embeddings.
type embedder interface {
	Embed(ctx context.Context, text string) ([]float32, error)
}

// filterMappingsByKeyword returns only mappings where Arabic key appears in content.
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

// filterMappingsBySimilarity returns top-K semantically similar mappings.
func filterMappingsBySimilarity(ctx context.Context, content string, allMappings []*entity.MedicationMapping, emb embedder, topK int) (map[string]string, error) {
	if len(allMappings) == 0 || topK <= 0 {
		return make(map[string]string), nil
	}

	contentEmbedding, err := emb.Embed(ctx, content)
	if err != nil {
		return nil, err
	}

	scored := scoreMappingsBySimilarity(allMappings, contentEmbedding)

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

// scoredMapping pairs a mapping with its similarity score.
type scoredMapping struct {
	mapping *entity.MedicationMapping
	score   float64
}

// scoreMappingsBySimilarity scores all mappings by cosine similarity.
func scoreMappingsBySimilarity(allMappings []*entity.MedicationMapping, contentEmbedding []float32) []scoredMapping {
	var scored []scoredMapping
	cosineSimilarity := similarity.CosineComparator{}
	for _, m := range allMappings {
		if len(m.Embedding) == 0 {
			continue
		}
		score, _ := cosineSimilarity.Similarity(contentEmbedding, m.Embedding)
		scored = append(scored, scoredMapping{m, score})
	}

	return scored
}

// filterMappingsHybrid combines keyword matching and vector similarity.
func filterMappingsHybrid(ctx context.Context, content string, allMappings []*entity.MedicationMapping, emb embedder, vectorTopK int) map[string]string {
	// Build map for keyword filtering (including synonyms)
	fullMap := buildFullMappingMap(allMappings)

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

// buildFullMappingMap builds a complete mapping including synonyms.
func buildFullMappingMap(allMappings []*entity.MedicationMapping) map[string]string {
	fullMap := make(map[string]string)
	for _, m := range allMappings {
		fullMap[m.ArabicName] = m.EnglishName
		for _, syn := range m.Synonyms {
			fullMap[syn] = m.EnglishName
		}
	}
	return fullMap
}
