package ai

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/rs/zerolog"
	"google.golang.org/genai"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

// GeminiClient handles communication with Gemini API using official SDK
type GeminiClient struct {
	cfg    *config.GeminiConfig
	client *genai.Client
	log    zerolog.Logger

	// Rate limiting
	mu               sync.Mutex
	requestsThisHour int
	hourStart        time.Time
}

// NewGeminiClient creates a new Gemini API client
func NewGeminiClient(ctx context.Context, cfg *config.GeminiConfig, log zerolog.Logger) (*GeminiClient, error) {
	client, err := genai.NewClient(ctx, &genai.ClientConfig{
		APIKey: cfg.APIKey,
	})
	if err != nil {
		return nil, fmt.Errorf("create genai client: %w", err)
	}

	return &GeminiClient{
		cfg:       cfg,
		client:    client,
		log:       log.With().Str("component", "gemini").Logger(),
		hourStart: time.Now(),
	}, nil
}

// JSON schema for parsing pharmaceutical messages
var pharmaParseSchema = map[string]any{
	"type": "array",
	"items": map[string]any{
		"type": "object",
		"properties": map[string]any{
			"message_index": map[string]any{
				"type":        "integer",
				"description": "0-based index of the message being parsed",
			},
			"items": map[string]any{
				"type": "array",
				"items": map[string]any{
					"type": "object",
					"properties": map[string]any{
						"type": map[string]any{
							"type":        "string",
							"enum":        []string{"OFFER", "REQUEST", "BOTH"},
							"description": "Type of message: OFFER for selling, REQUEST for buying, BOTH for swap",
						},
						"medication": map[string]any{
							"type":        "string",
							"description": "Normalized medication name in English",
						},
						"medication_raw": map[string]any{
							"type":        "string",
							"description": "Original medication name as written in the message",
						},
						"quantity": map[string]any{
							"type":        "integer",
							"description": "Quantity if specified, 0 otherwise",
						},
						"unit": map[string]any{
							"type":        "string",
							"description": "Unit of quantity: boxes, strips, ampules, bottles, or null",
						},
						"price": map[string]any{
							"type":        "number",
							"description": "Price for offers, 0 if not specified",
						},
						"max_price": map[string]any{
							"type":        "number",
							"description": "Maximum price for requests, 0 if not specified",
						},
						"currency": map[string]any{
							"type":        "string",
							"description": "Currency code, default EGP",
						},
						"expiry_date": map[string]any{
							"type":        "string",
							"description": "Expiry date in YYYY-MM format if mentioned, null otherwise",
						},
						"batch_number": map[string]any{
							"type":        "string",
							"description": "Batch number if mentioned, null otherwise",
						},
						"urgent": map[string]any{
							"type":        "boolean",
							"description": "True if request is marked as urgent",
						},
						"notes": map[string]any{
							"type":        "string",
							"description": "Any additional details or notes",
						},
					},
					"required": []string{"type", "medication", "medication_raw"},
				},
			},
			"error": map[string]any{
				"type":        "string",
				"description": "Error message if parsing failed for this message",
			},
		},
		"required": []string{"message_index", "items"},
	},
}

// ParseMessages parses one or more messages and extracts offers/requests
func (c *GeminiClient) ParseMessages(ctx context.Context, messages []*domain.RawMessage, mappings map[string]string) ([]*domain.AIParseResult, error) {
	// Check rate limit
	if !c.checkRateLimit() {
		return nil, fmt.Errorf("rate limit exceeded (%d requests/hour)", c.cfg.RateLimitPerHour)
	}

	// Build prompt with all messages
	prompt := buildParsePrompt(messages, mappings)

	// Configure generation with JSON schema
	config := &genai.GenerateContentConfig{
		ResponseMIMEType:   "application/json",
		ResponseJsonSchema: pharmaParseSchema,
		Temperature:        genai.Ptr(float32(0.1)), // Low temperature for consistent parsing
	}

	// Retry configuration from config
	maxRetries := c.cfg.MaxRetries
	baseDelay := c.cfg.RetryBaseDelay

	var result *genai.GenerateContentResponse
	var lastErr error

	for attempt := 1; attempt <= maxRetries; attempt++ {
		// Make API request
		result, lastErr = c.client.Models.GenerateContent(
			ctx,
			c.cfg.Model,
			genai.Text(prompt),
			config,
		)

		if lastErr == nil {
			// Success - break out of retry loop
			break
		}

		// Log retry attempt
		c.log.Warn().
			Err(lastErr).
			Int("attempt", attempt).
			Int("max_retries", maxRetries).
			Msg("Gemini API call failed, retrying...")

		// Don't delay after last attempt
		if attempt < maxRetries {
			// Exponential backoff: 1s, 2s, 4s
			delay := baseDelay * time.Duration(1<<(attempt-1))

			// Check context cancellation before sleeping
			select {
			case <-ctx.Done():
				return nil, fmt.Errorf("context cancelled during retry: %w", ctx.Err())
			case <-time.After(delay):
				// Continue to next attempt
			}
		}
	}

	if lastErr != nil {
		return nil, fmt.Errorf("generate content after %d retries: %w", maxRetries, lastErr)
	}

	// Get response text
	responseText := result.Text()
	c.log.Debug().Str("response", responseText).Msg("Gemini API response")

	// Parse the JSON response
	return c.parseResponse(responseText, messages)
}

func (c *GeminiClient) checkRateLimit() bool {
	c.mu.Lock()
	defer c.mu.Unlock()

	now := time.Now()

	// Reset counter if hour has passed
	if now.Sub(c.hourStart) >= time.Hour {
		c.requestsThisHour = 0
		c.hourStart = now
	}

	if c.requestsThisHour >= c.cfg.RateLimitPerHour {
		return false
	}

	c.requestsThisHour++
	return true
}

func (c *GeminiClient) parseResponse(responseText string, messages []*domain.RawMessage) ([]*domain.AIParseResult, error) {
	// Expected format: array of results, one per message
	var rawResults []struct {
		MessageIndex int                 `json:"message_index"`
		Items        []domain.ParsedItem `json:"items"`
		Error        string              `json:"error,omitempty"`
	}

	if err := json.Unmarshal([]byte(responseText), &rawResults); err != nil {
		// Try to parse as single result for single message
		var singleResult struct {
			Items []domain.ParsedItem `json:"items"`
			Error string              `json:"error,omitempty"`
		}
		if err2 := json.Unmarshal([]byte(responseText), &singleResult); err2 != nil {
			return nil, fmt.Errorf("parse JSON response: %w (original: %w)", err2, err)
		}

		// Convert to array format
		rawResults = []struct {
			MessageIndex int                 `json:"message_index"`
			Items        []domain.ParsedItem `json:"items"`
			Error        string              `json:"error,omitempty"`
		}{
			{MessageIndex: 0, Items: singleResult.Items, Error: singleResult.Error},
		}
	}

	// Map results back to messages
	results := make([]*domain.AIParseResult, len(messages))
	for i := range messages {
		results[i] = &domain.AIParseResult{
			Items:   []domain.ParsedItem{},
			RawJSON: responseText,
		}
	}

	for _, r := range rawResults {
		if r.MessageIndex >= 0 && r.MessageIndex < len(results) {
			results[r.MessageIndex].Items = r.Items
			results[r.MessageIndex].Error = r.Error
		}
	}

	return results, nil
}

// GetRateLimitStatus returns current rate limit usage
func (c *GeminiClient) GetRateLimitStatus() (used, limit int, resetIn time.Duration) {
	c.mu.Lock()
	defer c.mu.Unlock()

	elapsed := time.Since(c.hourStart)
	resetIn = max(time.Hour-elapsed, 0)

	return c.requestsThisHour, c.cfg.RateLimitPerHour, resetIn
}

// Close closes the client
func (c *GeminiClient) Close() error {
	// The genai client doesn't require explicit closing
	return nil
}

// Embed generates embeddings for the given text
func (c *GeminiClient) Embed(ctx context.Context, text string) ([]float32, error) {
	// Use text-embedding-004 for better performance
	model := "text-embedding-004"

	resp, err := c.client.Models.EmbedContent(ctx, model, genai.Text(text), nil)
	if err != nil {
		return nil, fmt.Errorf("gemini embed error: %w", err)
	}

	if resp == nil || len(resp.Embeddings) == 0 {
		return nil, fmt.Errorf("empty embedding response")
	}

	// First embedding
	return resp.Embeddings[0].Values, nil
}

// EmbedBatch generates embeddings for a batch of texts by calling Embed concurrently
// Note: Google GenAI SDK (v0.x) does not yet expose BatchEmbedContents in a stable way.
// We use a worker pool to simulate batching for performance.
func (c *GeminiClient) EmbedBatch(ctx context.Context, texts []string) ([][]float32, error) {
	if len(texts) == 0 {
		return nil, nil
	}

	results := make([][]float32, len(texts))
	var wg sync.WaitGroup
	var mu sync.Mutex
	var firstErr error

	// 5 concurrent requests usually safe for Gemini quotas
	sem := make(chan struct{}, 5)

	for i, text := range texts {
		wg.Add(1)
		go func(idx int, txt string) {
			defer wg.Done()

			sem <- struct{}{}
			defer func() { <-sem }()

			// Check if we should abort due to previous error
			mu.Lock()
			if firstErr != nil {
				mu.Unlock()
				return
			}
			mu.Unlock()

			// Use the existing Embed method
			// Note: Rate limiting is handled inside Embed or implicitly by API
			vec, err := c.Embed(ctx, txt)

			mu.Lock()
			defer mu.Unlock()

			if err != nil {
				if firstErr == nil {
					firstErr = err
				}
				return
			}
			results[idx] = vec
		}(i, text)
	}

	wg.Wait()

	if firstErr != nil {
		return nil, firstErr
	}
	return results, nil
}

// SetMappings is a no-op for GeminiClient as it doesn't use hybrid RAG filtering.
// The Gemini provider uses the full mappings passed to ParseMessages directly.
func (c *GeminiClient) SetMappings(mappings []*domain.MedicationMapping) {
	// No-op: Gemini doesn't use hybrid RAG filtering
	c.log.Debug().Int("count", len(mappings)).Msg("SetMappings called (no-op for Gemini)")
}
