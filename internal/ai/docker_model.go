package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/metrics"
	"strings"
	"sync"
	"time"

	"github.com/invopop/jsonschema"
	"github.com/openai/openai-go"
	"github.com/openai/openai-go/option"
	"github.com/rs/zerolog"
	"github.com/sony/gobreaker"
)

// DockerModelClient handles communication with Docker Model Runner
// using the OpenAI-compatible API.
type DockerModelClient struct {
	cfg    *config.DockerModelConfig
	client openai.Client
	log    zerolog.Logger
	cb     *gobreaker.CircuitBreaker
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
var aiParseResultSchema = GenerateSchema[domain.AIParseResult]()

// NewDockerModelClient creates a new Docker Model Runner client.
// It connects to the OpenAI-compatible API endpoint exposed by Docker Model Runner.
func NewDockerModelClient(cfg *config.DockerModelConfig, log zerolog.Logger) (*DockerModelClient, error) {
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

	return &DockerModelClient{
		cfg:    cfg,
		client: client,
		log:    log.With().Str("component", "docker-model").Logger(),
		cb:     gobreaker.NewCircuitBreaker(st),
	}, nil
}

// ParseMessages implements AIProvider.ParseMessages using Docker Model Runner.
func (c *DockerModelClient) ParseMessages(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	if len(messages) == 0 {
		return nil, nil
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
		chunks      []*domain.RawMessage
	}
	originalToChunks := make(map[int]*splitMap) // index -> chunks

	var workingSet []*domain.RawMessage

	for i, msg := range messages {
		// Calculate lines
		lines := strings.Count(msg.Content, "\n")

		if lines <= maxMessageLines {
			workingSet = append(workingSet, msg)
			originalToChunks[i] = &splitMap{originalIdx: i, chunks: []*domain.RawMessage{msg}}
			continue
		}

		// Split massive message
		c.log.Info().
			Str("msg_id", msg.ID).
			Int("lines", lines).
			Msg("Message too long, splitting content")

		contentLines := strings.Split(msg.Content, "\n")
		var chunks []*domain.RawMessage

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
	flatResultsMap := make(map[int][]*domain.AIParseResult)
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
		go func(bgIdx int, batch []*domain.RawMessage) {
			defer wg.Done()

			// Acquire semaphore
			sem <- struct{}{}
			defer func() { <-sem }()

			c.log.Debug().
				Int("chunk_start", bgIdx).
				Int("chunk_size", len(batch)).
				Msg("Processing chunk batch")

			results, err := c.processBatch(ctx, batch, mappings)

			mu.Lock()
			defer mu.Unlock()

			if err != nil {
				c.log.Error().Err(err).Int("chunk_start", bgIdx).Msg("Failed to process batch")
				var errResults []*domain.AIParseResult
				for range batch {
					errResults = append(errResults, &domain.AIParseResult{
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
	var flatResults []*domain.AIParseResult
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
				flatResults = append(flatResults, &domain.AIParseResult{Error: "Internal error: missing batch result"})
			}
		}
	}

	// Now merge flatResults back to original structure
	finalResults := make([]*domain.AIParseResult, len(messages))
	flatIdx := 0

	for i := range messages {
		origMap := originalToChunks[i]
		if origMap == nil {
			// Should not happen
			finalResults[i] = &domain.AIParseResult{Error: "Internal error: missing mapping"}
			continue
		}

		numChunks := len(origMap.chunks)
		if numChunks == 0 {
			finalResults[i] = &domain.AIParseResult{}
			continue
		}

		// Collect results for this message
		mergedResult := &domain.AIParseResult{}
		mergedItems := []domain.ParsedItem{}
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
	enforceMappings(finalResults, mappings)

	return finalResults, nil
}

// enforceMappings iterates over results and strictly applies known mappings
// overwriting the AI's output if a known Arabic term is found in the Raw field.
func enforceMappings(results []*domain.AIParseResult, mappings map[string]string) {
	if len(mappings) == 0 {
		return
	}

	for _, res := range results {
		for i := range res.Items {
			item := &res.Items[i]
			rawLower := strings.ToLower(item.MedicationRaw)

			// Check all mappings
			// Note: This O(N*M) look up is fine associated with the small size of mappings/items per message
			for arabic, english := range mappings {
				// If the raw text contains the known Arabic brand
				if strings.Contains(rawLower, strings.ToLower(arabic)) {
					// Check if the current English output is "incorrect" (doesn't contain the mapped term)
					// We use a loose check: if the English term isn't part of the output, we force it.
					if !strings.Contains(strings.ToLower(item.Medication), strings.ToLower(english)) {
						// Heuristic: Reconstruct the name by replacing the Arabic part in Raw with the English part
						// This preserves dosage/strength info usually present in Raw "زولادكس 3.6" -> "Zoladex 3.6"
						// We use the original Case form of Arabic if possible, but map keys are usually normalized.
						// Let's use string replacement on the Raw string.

						// Replace all occurrences if repetition exists
						newItem := strings.ReplaceAll(item.MedicationRaw, arabic, english)

						// If the replacing didn't work (case mismatch), just prepend the English name?
						// Or simpler: just set Medication = English + " " + (rest of raw?).
						// Safe bet: strings.Replace is good if the Arabic matches the key.
						// To handle case insensitivity of Arabic (rare but possible), we might need regex.
						// But for now, simple replace.
						if newItem == item.MedicationRaw {
							// Replace failed (case mismatch?), try simple overwrite
							// Or try to just use the mapped value + (maybe parsing logic?)
							// Let's just set it to properties we know + raw remainder?
							// Actually, if replace failed, maybe the AI Hallucinated the Raw too?
							// Assuming Raw is correct (extracted from text).

							// Fallback: Just use the English name.
							// Risk: Losing "3.6" or "500ml".
							// Better: Append Raw? "Zoladex (زولادكس 3.6)"?
							// Let's stick to the mapped name if logic fails.
							item.Medication = english
						} else {
							item.Medication = newItem
						}
					}
				}
			}
		}
	}
}

// processBatch processes a small batch of messages
func (c *DockerModelClient) processBatch(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	// Build prompt with messages
	prompt := buildParsePrompt(messages, mappings)

	// Count tokens
	tokenCount, err := CountTokens(c.cfg.Model, prompt)
	if err != nil {
		c.log.Warn().Err(err).Msg("Failed to count tokens")
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
	var parseResults []*domain.AIParseResult

	// First try: single AIParseResult object (most common from structured output)
	var singleResult domain.AIParseResult
	if err := json.Unmarshal([]byte(responseText), &singleResult); err == nil && singleResult.Items != nil {
		// Successfully parsed as single object with items field (even if empty)
		parseResults = []*domain.AIParseResult{&singleResult}
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
func countTotalItems(results []*domain.AIParseResult) int {
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
func (c *DockerModelClient) Embed(ctx context.Context, text string) ([]float32, error) {
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
func (c *DockerModelClient) EmbedBatch(ctx context.Context, texts []string) ([][]float32, error) {
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
