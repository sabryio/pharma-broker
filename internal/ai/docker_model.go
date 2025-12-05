package ai

import (
	"context"
	"encoding/json"
	"fmt"
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
func (c *DockerModelClient) ParseMessages(ctx context.Context, messages []*domain.RawMessage) ([]*domain.AIParseResult, error) {
	if len(messages) == 0 {
		return nil, nil
	}

	// Build prompt with messages
	prompt := buildParsePrompt(messages)

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

	// Parse JSON response
	var parseResults []*domain.AIParseResult
	if err := json.Unmarshal([]byte(responseText), &parseResults); err != nil {
		c.log.Error().
			Err(err).
			Str("response", truncateForLog(responseText, 500)).
			Msg("Failed to parse Docker Model Runner response")
		return nil, fmt.Errorf("parse response: %w", err)
	}

	return parseResults, nil
}

// truncateForLog truncates a string for logging purposes
func truncateForLog(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}
