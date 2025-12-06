package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/openai/openai-go"
	"github.com/openai/openai-go/option"
	"github.com/rs/zerolog"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

// DockerModelClient handles communication with Docker Model Runner
// using the OpenAI-compatible API.
type DockerModelClient struct {
	cfg    *config.DockerModelConfig
	client openai.Client
	log    zerolog.Logger
}

// NewDockerModelClient creates a new Docker Model Runner client.
// It connects to the OpenAI-compatible API endpoint exposed by Docker Model Runner.
func NewDockerModelClient(cfg *config.DockerModelConfig, log zerolog.Logger) (*DockerModelClient, error) {
	client := openai.NewClient(
		option.WithBaseURL(cfg.BaseURL),
		option.WithAPIKey("not-needed"), // Docker Model Runner doesn't require API key
	)

	return &DockerModelClient{
		cfg:    cfg,
		client: client,
		log:    log.With().Str("component", "docker-model").Logger(),
	}, nil
}

// ParseMessages implements AIProvider.ParseMessages using Docker Model Runner.
func (c *DockerModelClient) ParseMessages(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	if len(messages) == 0 {
		return nil, nil
	}

	// Constants for chunking
	const (
		maxBatchSize    = 1  // Process 1 chunk per request (safer for context)
		maxMessageLines = 20 // Split messages longer than 20 lines
	)

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

	return finalResults, nil
}

// processBatch processes a small batch of messages
func (c *DockerModelClient) processBatch(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	// Build prompt with messages
	prompt := buildParsePrompt(messages, mappings)

	c.log.Debug().
		Int("message_count", len(messages)).
		Msg("Sending messages to Docker Model Runner")

	// Create timeout context
	ctx, cancel := context.WithTimeout(ctx, c.cfg.RequestTimeout)
	defer cancel()

	// Retry logic with exponential backoff
	var result *openai.ChatCompletion
	var lastErr error

	for attempt := 1; attempt <= c.cfg.MaxRetries; attempt++ {
		result, lastErr = c.client.Chat.Completions.New(ctx, openai.ChatCompletionNewParams{
			Model: c.cfg.Model,
			Messages: []openai.ChatCompletionMessageParamUnion{
				openai.UserMessage(prompt), // prompt already includes system instructions
			},
			Temperature: openai.Float(0.1), // Low temperature for consistent parsing
		})

		if lastErr == nil {
			break
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

	if lastErr != nil {
		return nil, fmt.Errorf("docker model runner failed after %d attempts: %w", c.cfg.MaxRetries, lastErr)
	}

	// Extract response content
	if len(result.Choices) == 0 || result.Choices[0].Message.Content == "" {
		c.log.Warn().Msg("Empty response from Docker Model Runner")
		return nil, nil
	}

	responseText := result.Choices[0].Message.Content

	c.log.Debug().
		Int("response_length", len(responseText)).
		Msg("Received response from Docker Model Runner")

	// Parse JSON response - handle both array and single object responses
	var parseResults []*domain.AIParseResult

	cleanResponse := cleanJSON(responseText)

	// First try to parse as array
	if err := json.Unmarshal([]byte(cleanResponse), &parseResults); err != nil {
		// Try parsing as single object (model returns this for single message)
		var singleResult domain.AIParseResult
		if singleErr := json.Unmarshal([]byte(cleanResponse), &singleResult); singleErr != nil {
			c.log.Error().
				Err(err).
				Str("response", truncateForLog(responseText, 500)).
				Msg("Failed to parse Docker Model Runner response")
			return nil, fmt.Errorf("parse response: %w", err)
		}
		parseResults = []*domain.AIParseResult{&singleResult}
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

var invalidEscapeRegex = regexp.MustCompile(`\\([^"\\/bfnrtu])`)

// cleanJSON removes invalid control characters and fixes common JSON issues from LLMs
func cleanJSON(s string) string {
	// Fix invalid escapes: backslash followed by invalid char (not " \ / b f n r t u)
	// We escape the backslash to make it a literal backslash.
	// Example: "path\7" -> "path/7" (replace backslash with forward slash) which is valid JSON
	// This helps with things like "30\70" becoming "30/70"
	return invalidEscapeRegex.ReplaceAllString(s, "/$1")
}

// truncateForLog truncates a string for logging purposes
func truncateForLog(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}
