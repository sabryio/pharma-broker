package parsing

import (
	"pharmabroker/domain/entity"
	"strings"
	"testing"

	"github.com/rs/zerolog"
)

// =============================================================================
// TokenBatchConfig Tests
// =============================================================================

func TestDefaultTokenBatchConfig(t *testing.T) {
	cfg := DefaultTokenBatchConfig()

	if cfg.MaxTokensPerBatch != DefaultMaxTokensPerBatch {
		t.Errorf("MaxTokensPerBatch = %d, want %d", cfg.MaxTokensPerBatch, DefaultMaxTokensPerBatch)
	}
	if cfg.PromptOverhead != DefaultPromptOverhead {
		t.Errorf("PromptOverhead = %d, want %d", cfg.PromptOverhead, DefaultPromptOverhead)
	}
	if cfg.TokensPerMessage != DefaultTokensPerMessage {
		t.Errorf("TokensPerMessage = %d, want %d", cfg.TokensPerMessage, DefaultTokensPerMessage)
	}
	if cfg.MaxMessagesPerBatch != DefaultBatchSize {
		t.Errorf("MaxMessagesPerBatch = %d, want %d", cfg.MaxMessagesPerBatch, DefaultBatchSize)
	}
}

// =============================================================================
// TokenBatchStats Tests
// =============================================================================

func TestTokenBatchStats_GetStats(t *testing.T) {
	stats := &TokenBatchStats{}
	stats.TotalBatches.Store(5)
	stats.TotalMessages.Store(25)
	stats.TotalTokens.Store(5000)
	stats.SplitBatches.Store(2)
	stats.OversizedMessages.Store(1)

	result := stats.GetStats()

	if result["total_batches"] != 5 {
		t.Errorf("total_batches = %d, want 5", result["total_batches"])
	}
	if result["total_messages"] != 25 {
		t.Errorf("total_messages = %d, want 25", result["total_messages"])
	}
	if result["total_tokens"] != 5000 {
		t.Errorf("total_tokens = %d, want 5000", result["total_tokens"])
	}
	if result["split_batches"] != 2 {
		t.Errorf("split_batches = %d, want 2", result["split_batches"])
	}
	if result["oversized_messages"] != 1 {
		t.Errorf("oversized_messages = %d, want 1", result["oversized_messages"])
	}
}

// =============================================================================
// NewTokenBatcher Tests
// =============================================================================

func TestNewTokenBatcher_DefaultValues(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{} // All zero values

	batcher := NewTokenBatcher(cfg, log)

	if batcher.config.MaxTokensPerBatch != DefaultMaxTokensPerBatch {
		t.Errorf("MaxTokensPerBatch = %d, want %d", batcher.config.MaxTokensPerBatch, DefaultMaxTokensPerBatch)
	}
	if batcher.config.PromptOverhead != DefaultPromptOverhead {
		t.Errorf("PromptOverhead = %d, want %d", batcher.config.PromptOverhead, DefaultPromptOverhead)
	}
	if batcher.config.TokensPerMessage != DefaultTokensPerMessage {
		t.Errorf("TokensPerMessage = %d, want %d", batcher.config.TokensPerMessage, DefaultTokensPerMessage)
	}
	if batcher.config.MaxMessagesPerBatch != DefaultBatchSize {
		t.Errorf("MaxMessagesPerBatch = %d, want %d", batcher.config.MaxMessagesPerBatch, DefaultBatchSize)
	}
}

func TestNewTokenBatcher_CustomConfig(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{
		MaxTokensPerBatch:   8000,
		PromptOverhead:      1500,
		TokensPerMessage:    30,
		MaxMessagesPerBatch: 15,
	}

	batcher := NewTokenBatcher(cfg, log)

	if batcher.config.MaxTokensPerBatch != 8000 {
		t.Errorf("MaxTokensPerBatch = %d, want 8000", batcher.config.MaxTokensPerBatch)
	}
	if batcher.config.PromptOverhead != 1500 {
		t.Errorf("PromptOverhead = %d, want 1500", batcher.config.PromptOverhead)
	}
}

// =============================================================================
// EstimateMessageTokens Tests
// =============================================================================

func TestTokenBatcher_EstimateMessageTokens(t *testing.T) {
	log := zerolog.Nop()
	batcher := NewTokenBatcher(DefaultTokenBatchConfig(), log)

	tests := []struct {
		name        string
		msg         *entity.RawMessage
		minExpected int
		maxExpected int
	}{
		{
			name: "short message",
			msg: &entity.RawMessage{
				ID:      "1",
				Content: "عندي اوجمنتين",
			},
			minExpected: 50,  // At least overhead
			maxExpected: 100, // Short content
		},
		{
			name: "medium message",
			msg: &entity.RawMessage{
				ID:      "2",
				Content: "عندي 5 علب اوجمنتين 1 جم ب 300 جنيه للعلبة متوفر للتسليم فوري",
			},
			minExpected: 50,
			maxExpected: 150,
		},
		{
			name: "message with reply context",
			msg: &entity.RawMessage{
				ID:             "3",
				Content:        "نفسه بكام؟",
				ReplyToContent: "عندي كونكور 5 متوفر",
			},
			minExpected: 50,
			maxExpected: 150,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tokens := batcher.EstimateMessageTokens(tt.msg)
			if tokens < tt.minExpected || tokens > tt.maxExpected {
				t.Errorf("EstimateMessageTokens() = %d, want between %d and %d",
					tokens, tt.minExpected, tt.maxExpected)
			}
		})
	}
}

// =============================================================================
// SplitIntoBatches Tests
// =============================================================================

func TestTokenBatcher_SplitIntoBatches_EmptyInput(t *testing.T) {
	log := zerolog.Nop()
	batcher := NewTokenBatcher(DefaultTokenBatchConfig(), log)

	batches := batcher.SplitIntoBatches(nil)
	if batches != nil {
		t.Errorf("SplitIntoBatches(nil) = %v, want nil", batches)
	}

	batches = batcher.SplitIntoBatches([]*entity.RawMessage{})
	if batches != nil {
		t.Errorf("SplitIntoBatches([]) = %v, want nil", batches)
	}
}

func TestTokenBatcher_SplitIntoBatches_SingleMessage(t *testing.T) {
	log := zerolog.Nop()
	batcher := NewTokenBatcher(DefaultTokenBatchConfig(), log)

	messages := []*entity.RawMessage{
		{ID: "1", Content: "عندي اوجمنتين"},
	}

	batches := batcher.SplitIntoBatches(messages)

	if len(batches) != 1 {
		t.Errorf("len(batches) = %d, want 1", len(batches))
	}
	if len(batches[0]) != 1 {
		t.Errorf("len(batches[0]) = %d, want 1", len(batches[0]))
	}
}

func TestTokenBatcher_SplitIntoBatches_FitsInOneBatch(t *testing.T) {
	log := zerolog.Nop()
	batcher := NewTokenBatcher(DefaultTokenBatchConfig(), log)

	messages := make([]*entity.RawMessage, 5)
	for i := 0; i < 5; i++ {
		messages[i] = &entity.RawMessage{
			ID:      string(rune('1' + i)),
			Content: "عندي اوجمنتين متوفر",
		}
	}

	batches := batcher.SplitIntoBatches(messages)

	if len(batches) != 1 {
		t.Errorf("len(batches) = %d, want 1 (all fit in one batch)", len(batches))
	}
	if len(batches[0]) != 5 {
		t.Errorf("len(batches[0]) = %d, want 5", len(batches[0]))
	}
}

func TestTokenBatcher_SplitIntoBatches_ExceedsTokenLimit(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{
		MaxTokensPerBatch:   500, // Very low limit
		PromptOverhead:      200, // Leaves 300 for messages
		TokensPerMessage:    50,
		MaxMessagesPerBatch: 100, // High message limit (won't trigger)
	}
	batcher := NewTokenBatcher(cfg, log)

	// Create messages that will exceed token limit
	messages := make([]*entity.RawMessage, 10)
	for i := 0; i < 10; i++ {
		messages[i] = &entity.RawMessage{
			ID:      string(rune('A' + i)),
			Content: "عندي 5 علب اوجمنتين 1 جم ب 300 جنيه للعلبة متوفر للتسليم فوري في القاهرة",
		}
	}

	batches := batcher.SplitIntoBatches(messages)

	if len(batches) <= 1 {
		t.Errorf("len(batches) = %d, want > 1 (should split due to token limit)", len(batches))
	}

	// Verify all messages are accounted for
	totalMessages := 0
	for _, batch := range batches {
		totalMessages += len(batch)
	}
	if totalMessages != 10 {
		t.Errorf("total messages = %d, want 10", totalMessages)
	}

	// Verify stats
	stats := batcher.GetStats()
	if stats["split_batches"] == 0 {
		t.Error("split_batches should be > 0")
	}
}

func TestTokenBatcher_SplitIntoBatches_ExceedsMessageLimit(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{
		MaxTokensPerBatch:   100000, // Very high token limit (won't trigger)
		PromptOverhead:      2000,
		TokensPerMessage:    50,
		MaxMessagesPerBatch: 3, // Low message limit
	}
	batcher := NewTokenBatcher(cfg, log)

	messages := make([]*entity.RawMessage, 10)
	for i := 0; i < 10; i++ {
		messages[i] = &entity.RawMessage{
			ID:      string(rune('A' + i)),
			Content: "short",
		}
	}

	batches := batcher.SplitIntoBatches(messages)

	// Should split into ceil(10/3) = 4 batches
	if len(batches) != 4 {
		t.Errorf("len(batches) = %d, want 4 (split by message count)", len(batches))
	}
}

func TestTokenBatcher_SplitIntoBatches_OversizedMessage(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{
		MaxTokensPerBatch:   500,
		PromptOverhead:      400, // Only 100 tokens available
		TokensPerMessage:    50,
		MaxMessagesPerBatch: 10,
	}
	batcher := NewTokenBatcher(cfg, log)

	// Create one oversized message (will exceed available tokens)
	oversizedContent := strings.Repeat("عندي اوجمنتين متوفر للبيع ", 100)
	messages := []*entity.RawMessage{
		{ID: "1", Content: "short"},
		{ID: "2", Content: oversizedContent}, // Oversized
		{ID: "3", Content: "short"},
	}

	batches := batcher.SplitIntoBatches(messages)

	// Oversized message should be in its own batch
	if len(batches) < 2 {
		t.Errorf("len(batches) = %d, want >= 2 (oversized should be separate)", len(batches))
	}

	// Verify stats
	stats := batcher.GetStats()
	if stats["oversized_messages"] != 1 {
		t.Errorf("oversized_messages = %d, want 1", stats["oversized_messages"])
	}
}

// =============================================================================
// EstimateBatchTokens Tests
// =============================================================================

func TestTokenBatcher_EstimateBatchTokens(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{
		MaxTokensPerBatch:   6000,
		PromptOverhead:      2000,
		TokensPerMessage:    50,
		MaxMessagesPerBatch: 10,
	}
	batcher := NewTokenBatcher(cfg, log)

	messages := []*entity.RawMessage{
		{ID: "1", Content: "short message"},
		{ID: "2", Content: "another short message"},
	}

	tokens := batcher.EstimateBatchTokens(messages)

	// Should be at least prompt overhead + 2 * message overhead
	minExpected := cfg.PromptOverhead + 2*cfg.TokensPerMessage
	if tokens < minExpected {
		t.Errorf("EstimateBatchTokens() = %d, want >= %d", tokens, minExpected)
	}
}

// =============================================================================
// GetConfig and SetConfig Tests
// =============================================================================

func TestTokenBatcher_GetConfig(t *testing.T) {
	log := zerolog.Nop()
	cfg := TokenBatchConfig{
		MaxTokensPerBatch:   8000,
		PromptOverhead:      1500,
		TokensPerMessage:    40,
		MaxMessagesPerBatch: 20,
	}
	batcher := NewTokenBatcher(cfg, log)

	got := batcher.GetConfig()
	if got.MaxTokensPerBatch != cfg.MaxTokensPerBatch {
		t.Errorf("GetConfig().MaxTokensPerBatch = %d, want %d", got.MaxTokensPerBatch, cfg.MaxTokensPerBatch)
	}
}

func TestTokenBatcher_SetConfig(t *testing.T) {
	log := zerolog.Nop()
	batcher := NewTokenBatcher(DefaultTokenBatchConfig(), log)

	newCfg := TokenBatchConfig{
		MaxTokensPerBatch:   10000,
		PromptOverhead:      3000,
		TokensPerMessage:    60,
		MaxMessagesPerBatch: 25,
	}
	batcher.SetConfig(newCfg)

	got := batcher.GetConfig()
	if got.MaxTokensPerBatch != newCfg.MaxTokensPerBatch {
		t.Errorf("After SetConfig, MaxTokensPerBatch = %d, want %d", got.MaxTokensPerBatch, newCfg.MaxTokensPerBatch)
	}
}

// =============================================================================
// Token Batching Constants Tests
// =============================================================================

func TestTokenBatchingConstants(t *testing.T) {
	if DefaultMaxTokensPerBatch <= 0 {
		t.Error("DefaultMaxTokensPerBatch should be positive")
	}
	if DefaultPromptOverhead <= 0 {
		t.Error("DefaultPromptOverhead should be positive")
	}
	if DefaultTokensPerMessage <= 0 {
		t.Error("DefaultTokensPerMessage should be positive")
	}
	if DefaultMaxTokensPerBatch <= DefaultPromptOverhead {
		t.Error("DefaultMaxTokensPerBatch should be greater than DefaultPromptOverhead")
	}
}
