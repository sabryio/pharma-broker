package historysync

import (
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func TestNew(t *testing.T) {
	logger := zerolog.Nop()
	cfg := DefaultConfig()
	h := New(cfg, logger)

	if h == nil {
		t.Fatal("New returned nil")
	}

	if h.cfg.Cooldown != DefaultCooldown {
		t.Errorf("Expected cooldown %v, got %v", DefaultCooldown, h.cfg.Cooldown)
	}

	if h.cfg.MaxAge != DefaultMaxAge {
		t.Errorf("Expected max age %v, got %v", DefaultMaxAge, h.cfg.MaxAge)
	}

	if h.cfg.MaxMessages != DefaultMaxMessages {
		t.Errorf("Expected max messages %v, got %v", DefaultMaxMessages, h.cfg.MaxMessages)
	}
}

func TestHandler_ShouldProcess(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		Cooldown:    100 * time.Millisecond, // Short cooldown for testing
		MaxAge:      DefaultMaxAge,
		MaxMessages: DefaultMaxMessages,
		CacheSize:   DefaultCacheSize,
		CacheTTL:    DefaultCacheTTL,
	}
	h := New(cfg, logger)

	// First call should always process
	if !h.ShouldProcess() {
		t.Error("First call should process")
	}

	// Immediate second call should be blocked by cooldown
	if h.ShouldProcess() {
		t.Error("Second call should be blocked by cooldown")
	}

	// Wait for cooldown
	time.Sleep(150 * time.Millisecond)

	// Should process again
	if !h.ShouldProcess() {
		t.Error("Should process after cooldown")
	}

	stats := h.GetStats()
	if stats["total_syncs"] != 3 {
		t.Errorf("Expected 3 total syncs, got %d", stats["total_syncs"])
	}
	if stats["skipped_cooldown"] != 1 {
		t.Errorf("Expected 1 skipped, got %d", stats["skipped_cooldown"])
	}
}

func TestHandler_IsMessageTooOld(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		Cooldown:    DefaultCooldown,
		MaxAge:      1 * time.Hour, // 1 hour max age
		MaxMessages: DefaultMaxMessages,
		CacheSize:   DefaultCacheSize,
		CacheTTL:    DefaultCacheTTL,
	}
	h := New(cfg, logger)

	// Recent message should not be too old
	recent := time.Now().Add(-30 * time.Minute)
	if h.IsMessageTooOld(recent) {
		t.Error("30 minute old message should not be too old")
	}

	// Old message should be too old
	old := time.Now().Add(-2 * time.Hour)
	if !h.IsMessageTooOld(old) {
		t.Error("2 hour old message should be too old")
	}
}

func TestHandler_MessageProcessing(t *testing.T) {
	logger := zerolog.Nop()
	cfg := DefaultConfig()
	h := New(cfg, logger)

	msgID := "test-message-123"

	// Should not be processed initially
	if h.IsMessageProcessed(msgID) {
		t.Error("Message should not be processed initially")
	}

	// Mark as processed
	h.MarkMessageProcessed(msgID)

	// Should now be processed
	if !h.IsMessageProcessed(msgID) {
		t.Error("Message should be processed after marking")
	}

	// Different message should not be processed
	if h.IsMessageProcessed("different-message") {
		t.Error("Different message should not be processed")
	}
}

func TestHandler_CleanupCache(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		Cooldown:    DefaultCooldown,
		MaxAge:      DefaultMaxAge,
		MaxMessages: DefaultMaxMessages,
		CacheSize:   5,                     // Small cache for testing
		CacheTTL:    50 * time.Millisecond, // Short TTL for testing
	}
	h := New(cfg, logger)

	// Add some messages
	for i := 0; i < 3; i++ {
		h.MarkMessageProcessed("msg-" + string(rune('a'+i)))
	}

	if h.CacheSize() != 3 {
		t.Errorf("Expected cache size 3, got %d", h.CacheSize())
	}

	// Wait for TTL to expire
	time.Sleep(100 * time.Millisecond)

	// Cleanup should remove expired entries
	h.CleanupCache()

	if h.CacheSize() != 0 {
		t.Errorf("Expected cache size 0 after cleanup, got %d", h.CacheSize())
	}
}

func TestHandler_CacheSizeLimit(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		Cooldown:    DefaultCooldown,
		MaxAge:      DefaultMaxAge,
		MaxMessages: DefaultMaxMessages,
		CacheSize:   3, // Very small cache
		CacheTTL:    1 * time.Hour,
	}
	h := New(cfg, logger)

	// Add more messages than cache size
	for i := 0; i < 10; i++ {
		h.MarkMessageProcessed("msg-" + string(rune('a'+i)))
	}

	// Cleanup should enforce size limit
	h.CleanupCache()

	if h.CacheSize() > 3 {
		t.Errorf("Cache size should be <= 3, got %d", h.CacheSize())
	}
}

func TestHandler_Reset(t *testing.T) {
	logger := zerolog.Nop()
	cfg := DefaultConfig()
	h := New(cfg, logger)

	// Process some messages
	h.ShouldProcess()
	h.MarkMessageProcessed("msg-1")
	h.MarkMessageProcessed("msg-2")

	if h.CacheSize() != 2 {
		t.Errorf("Expected cache size 2, got %d", h.CacheSize())
	}

	// Reset
	h.Reset()

	// Cache should be empty
	if h.CacheSize() != 0 {
		t.Errorf("Expected cache size 0 after reset, got %d", h.CacheSize())
	}

	// Should be able to process again immediately
	if !h.ShouldProcess() {
		t.Error("Should be able to process after reset")
	}
}

func TestHandler_RecordStats(t *testing.T) {
	logger := zerolog.Nop()
	cfg := DefaultConfig()
	h := New(cfg, logger)

	h.RecordReceived(100)
	h.RecordSkipped(30)
	h.RecordProcessed(70)

	stats := h.GetStats()
	if stats["messages_received"] != 100 {
		t.Errorf("Expected 100 received, got %d", stats["messages_received"])
	}
	if stats["messages_skipped"] != 30 {
		t.Errorf("Expected 30 skipped, got %d", stats["messages_skipped"])
	}
	if stats["messages_processed"] != 70 {
		t.Errorf("Expected 70 processed, got %d", stats["messages_processed"])
	}
}

func TestHandler_MaxMessages(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		Cooldown:    DefaultCooldown,
		MaxAge:      DefaultMaxAge,
		MaxMessages: 500,
		CacheSize:   DefaultCacheSize,
		CacheTTL:    DefaultCacheTTL,
	}
	h := New(cfg, logger)

	if h.MaxMessages() != 500 {
		t.Errorf("Expected max messages 500, got %d", h.MaxMessages())
	}
}
