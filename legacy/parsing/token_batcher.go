package parsing

import (
	"pharmabroker/domain/entity"
	"pharmabroker/pkg/text"
	"sync/atomic"

	"github.com/rs/zerolog"
)

// =============================================================================
// Token-Aware Batching Configuration
// =============================================================================

// TokenBatchConfig holds configuration for token-aware batching.
type TokenBatchConfig struct {
	MaxTokensPerBatch   int // Maximum tokens allowed per batch (default: 6000)
	PromptOverhead      int // Estimated tokens for system prompt + mappings (default: 2000)
	TokensPerMessage    int // Estimated overhead per message structure (default: 50)
	MaxMessagesPerBatch int // Hard limit on messages per batch (default: 10)
}

// DefaultTokenBatchConfig returns sensible defaults for token batching.
func DefaultTokenBatchConfig() TokenBatchConfig {
	return TokenBatchConfig{
		MaxTokensPerBatch:   DefaultMaxTokensPerBatch,
		PromptOverhead:      DefaultPromptOverhead,
		TokensPerMessage:    DefaultTokensPerMessage,
		MaxMessagesPerBatch: DefaultBatchSize,
	}
}

// TokenBatchStats tracks token batching statistics.
type TokenBatchStats struct {
	TotalBatches      atomic.Int64 // Total batches created
	TotalMessages     atomic.Int64 // Total messages processed
	TotalTokens       atomic.Int64 // Total tokens estimated
	SplitBatches      atomic.Int64 // Batches that were split due to token limits
	OversizedMessages atomic.Int64 // Messages that exceeded single-message token limit
}

// GetStats returns a snapshot of token batching statistics.
func (s *TokenBatchStats) GetStats() map[string]int64 {
	return map[string]int64{
		"total_batches":      s.TotalBatches.Load(),
		"total_messages":     s.TotalMessages.Load(),
		"total_tokens":       s.TotalTokens.Load(),
		"split_batches":      s.SplitBatches.Load(),
		"oversized_messages": s.OversizedMessages.Load(),
	}
}

// =============================================================================
// Token Batcher
// =============================================================================

// TokenBatcher splits message batches based on token limits.
type TokenBatcher struct {
	config TokenBatchConfig
	stats  TokenBatchStats
	log    zerolog.Logger
}

// NewTokenBatcher creates a new token-aware batcher.
func NewTokenBatcher(cfg TokenBatchConfig, log zerolog.Logger) *TokenBatcher {
	// Apply defaults for zero values
	if cfg.MaxTokensPerBatch <= 0 {
		cfg.MaxTokensPerBatch = DefaultMaxTokensPerBatch
	}
	if cfg.PromptOverhead <= 0 {
		cfg.PromptOverhead = DefaultPromptOverhead
	}
	if cfg.TokensPerMessage <= 0 {
		cfg.TokensPerMessage = DefaultTokensPerMessage
	}
	if cfg.MaxMessagesPerBatch <= 0 {
		cfg.MaxMessagesPerBatch = DefaultBatchSize
	}

	return &TokenBatcher{
		config: cfg,
		log:    log.With().Str("component", "token-batcher").Logger(),
	}
}

// EstimateMessageTokens estimates the token count for a single message.
func (tb *TokenBatcher) EstimateMessageTokens(msg *entity.RawMessage) int {
	// Base overhead for message structure (From:, Group:, Content: labels)
	tokens := tb.config.TokensPerMessage

	// Count content tokens
	contentTokens, err := text.CountTokens(msg.Content)
	if err != nil {
		// Fallback: estimate ~4 chars per token
		contentTokens = len(msg.Content) / 4
	}
	tokens += contentTokens

	// Add reply context tokens if present
	if msg.ReplyToContent != "" {
		replyTokens, err := text.CountTokens(msg.ReplyToContent)
		if err != nil {
			replyTokens = len(msg.ReplyToContent) / 4
		}
		// Reply is truncated to 200 chars in prompt, so cap it
		if replyTokens > 60 {
			replyTokens = 60
		}
		tokens += replyTokens
	}

	return tokens
}

// SplitIntoBatches splits messages into token-aware batches.
// Each batch will not exceed MaxTokensPerBatch (accounting for prompt overhead).
func (tb *TokenBatcher) SplitIntoBatches(messages []*entity.RawMessage) [][]*entity.RawMessage {
	if len(messages) == 0 {
		return nil
	}

	availableTokens := tb.config.MaxTokensPerBatch - tb.config.PromptOverhead
	if availableTokens <= 0 {
		availableTokens = tb.config.MaxTokensPerBatch / 2 // Safety fallback
	}

	var batches [][]*entity.RawMessage
	var currentBatch []*entity.RawMessage
	currentTokens := 0
	totalTokens := 0

	for _, msg := range messages {
		msgTokens := tb.EstimateMessageTokens(msg)
		totalTokens += msgTokens

		// Check if single message exceeds limit (oversized)
		if msgTokens > availableTokens {
			tb.stats.OversizedMessages.Add(1)
			tb.log.Warn().
				Str("msg_id", msg.ID).
				Int("tokens", msgTokens).
				Int("limit", availableTokens).
				Msg("⚠️ Oversized message exceeds token limit, processing alone")

			// Flush current batch if not empty
			if len(currentBatch) > 0 {
				batches = append(batches, currentBatch)
				currentBatch = nil
				currentTokens = 0
			}

			// Add oversized message as its own batch
			batches = append(batches, []*entity.RawMessage{msg})
			continue
		}

		// Check if adding this message would exceed limits
		wouldExceedTokens := currentTokens+msgTokens > availableTokens
		wouldExceedCount := len(currentBatch) >= tb.config.MaxMessagesPerBatch

		if wouldExceedTokens || wouldExceedCount {
			// Flush current batch
			if len(currentBatch) > 0 {
				batches = append(batches, currentBatch)
				tb.stats.SplitBatches.Add(1)
			}
			currentBatch = nil
			currentTokens = 0
		}

		// Add message to current batch
		currentBatch = append(currentBatch, msg)
		currentTokens += msgTokens
	}

	// Don't forget the last batch
	if len(currentBatch) > 0 {
		batches = append(batches, currentBatch)
	}

	// Update stats
	tb.stats.TotalBatches.Add(int64(len(batches)))
	tb.stats.TotalMessages.Add(int64(len(messages)))
	tb.stats.TotalTokens.Add(int64(totalTokens))

	// Log if batches were split
	if len(batches) > 1 {
		tb.log.Info().
			Int("original_count", len(messages)).
			Int("batch_count", len(batches)).
			Int("total_tokens", totalTokens).
			Int("max_tokens", tb.config.MaxTokensPerBatch).
			Msg("📦 Split messages into token-aware batches")
	}

	return batches
}

// GetStats returns the current token batching statistics.
func (tb *TokenBatcher) GetStats() map[string]int64 {
	return tb.stats.GetStats()
}

// GetConfig returns the current configuration.
func (tb *TokenBatcher) GetConfig() TokenBatchConfig {
	return tb.config
}

// SetConfig updates the token batching configuration.
func (tb *TokenBatcher) SetConfig(cfg TokenBatchConfig) {
	tb.config = cfg
	tb.log.Info().
		Int("max_tokens", cfg.MaxTokensPerBatch).
		Int("prompt_overhead", cfg.PromptOverhead).
		Int("max_messages", cfg.MaxMessagesPerBatch).
		Msg("Token batcher configuration updated")
}

// EstimateBatchTokens estimates total tokens for a batch of messages.
func (tb *TokenBatcher) EstimateBatchTokens(messages []*entity.RawMessage) int {
	total := tb.config.PromptOverhead
	for _, msg := range messages {
		total += tb.EstimateMessageTokens(msg)
	}
	return total
}
