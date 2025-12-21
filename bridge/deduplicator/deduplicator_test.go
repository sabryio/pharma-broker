package deduplicator

import (
	"context"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func TestNew(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := DefaultConfig()

	d := New(ctx, cfg, logger)
	defer d.Close()

	if d == nil {
		t.Fatal("New returned nil")
	}

	stats := d.Stats()
	if stats.CacheSize != 0 {
		t.Errorf("Expected empty cache, got size %d", stats.CacheSize)
	}
}

func TestDeduplicator_IsDuplicate(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := Config{
		Window:          5 * time.Second,
		CacheSize:       100,
		CacheTTL:        30 * time.Second,
		CleanupInterval: time.Minute,
	}

	d := New(ctx, cfg, logger)
	defer d.Close()

	groupJID := "group@g.us"
	senderJID := "sender@s.whatsapp.net"
	content := "Hello, world!"
	timestamp := time.Now()

	// First message should not be duplicate
	if d.IsDuplicate(groupJID, senderJID, content, timestamp) {
		t.Error("First message should not be duplicate")
	}

	// Record the message
	d.RecordMessage(groupJID, senderJID, content, timestamp)

	// Same message within window should be duplicate
	if !d.IsDuplicate(groupJID, senderJID, content, timestamp.Add(1*time.Second)) {
		t.Error("Same message within window should be duplicate")
	}

	// Different content should not be duplicate
	if d.IsDuplicate(groupJID, senderJID, "Different content", timestamp) {
		t.Error("Different content should not be duplicate")
	}

	// Different sender should not be duplicate
	if d.IsDuplicate(groupJID, "other@s.whatsapp.net", content, timestamp) {
		t.Error("Different sender should not be duplicate")
	}

	// Different group should not be duplicate
	if d.IsDuplicate("other@g.us", senderJID, content, timestamp) {
		t.Error("Different group should not be duplicate")
	}
}

func TestDeduplicator_WindowExpiry(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := Config{
		Window:          100 * time.Millisecond, // Short window for testing
		CacheSize:       100,
		CacheTTL:        1 * time.Second,
		CleanupInterval: time.Minute,
	}

	d := New(ctx, cfg, logger)
	defer d.Close()

	groupJID := "group@g.us"
	senderJID := "sender@s.whatsapp.net"
	content := "Hello!"
	timestamp := time.Now()

	d.RecordMessage(groupJID, senderJID, content, timestamp)

	// Within window - should be duplicate
	if !d.IsDuplicate(groupJID, senderJID, content, timestamp.Add(50*time.Millisecond)) {
		t.Error("Should be duplicate within window")
	}

	// Outside window - should not be duplicate
	if d.IsDuplicate(groupJID, senderJID, content, timestamp.Add(200*time.Millisecond)) {
		t.Error("Should not be duplicate outside window")
	}
}

func TestDeduplicator_CacheTTL(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := Config{
		Window:          1 * time.Hour, // Long window
		CacheSize:       100,
		CacheTTL:        100 * time.Millisecond, // Short TTL for testing
		CleanupInterval: time.Minute,
	}

	d := New(ctx, cfg, logger)
	defer d.Close()

	groupJID := "group@g.us"
	senderJID := "sender@s.whatsapp.net"
	content := "Hello!"
	timestamp := time.Now()

	d.RecordMessage(groupJID, senderJID, content, timestamp)

	// Immediately - should be duplicate
	if !d.IsDuplicate(groupJID, senderJID, content, timestamp) {
		t.Error("Should be duplicate immediately")
	}

	// Wait for TTL to expire
	time.Sleep(150 * time.Millisecond)

	// After TTL - should not be duplicate (entry expired)
	if d.IsDuplicate(groupJID, senderJID, content, timestamp) {
		t.Error("Should not be duplicate after TTL expiry")
	}
}

func TestDeduplicator_CacheSizeLimit(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := Config{
		Window:          1 * time.Hour,
		CacheSize:       3, // Very small cache
		CacheTTL:        1 * time.Hour,
		CleanupInterval: time.Minute,
	}

	d := New(ctx, cfg, logger)
	defer d.Close()

	// Add more messages than cache size
	for i := 0; i < 10; i++ {
		d.RecordMessage("group@g.us", "sender"+string(rune('a'+i))+"@s.whatsapp.net", "content", time.Now())
	}

	stats := d.Stats()
	if stats.CacheSize > 3 {
		t.Errorf("Cache size should be <= 3, got %d", stats.CacheSize)
	}
}

func TestDeduplicator_Stats(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := DefaultConfig()

	d := New(ctx, cfg, logger)
	defer d.Close()

	groupJID := "group@g.us"
	senderJID := "sender@s.whatsapp.net"
	content := "Hello!"
	timestamp := time.Now()

	// Record and check for duplicates
	d.RecordMessage(groupJID, senderJID, content, timestamp)

	// Miss (different content)
	d.IsDuplicate(groupJID, senderJID, "different", timestamp)

	// Hit (same content)
	d.IsDuplicate(groupJID, senderJID, content, timestamp)

	// Another hit
	d.IsDuplicate(groupJID, senderJID, content, timestamp)

	stats := d.Stats()
	if stats.Hits != 2 {
		t.Errorf("Expected 2 hits, got %d", stats.Hits)
	}
	if stats.Misses != 1 {
		t.Errorf("Expected 1 miss, got %d", stats.Misses)
	}
	if stats.CacheSize != 1 {
		t.Errorf("Expected cache size 1, got %d", stats.CacheSize)
	}

	// Hit rate should be ~66.67%
	expectedHitRate := 66.67
	if stats.HitRate < expectedHitRate-1 || stats.HitRate > expectedHitRate+1 {
		t.Errorf("Expected hit rate ~%.2f%%, got %.2f%%", expectedHitRate, stats.HitRate)
	}
}

func TestDeduplicator_Cleanup(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := Config{
		Window:          1 * time.Hour,
		CacheSize:       100,
		CacheTTL:        50 * time.Millisecond, // Short TTL
		CleanupInterval: 25 * time.Millisecond, // Frequent cleanup
	}

	d := New(ctx, cfg, logger)
	defer d.Close()

	// Add some messages
	for i := 0; i < 5; i++ {
		d.RecordMessage("group@g.us", "sender"+string(rune('a'+i))+"@s.whatsapp.net", "content", time.Now())
	}

	stats := d.Stats()
	if stats.CacheSize != 5 {
		t.Errorf("Expected cache size 5, got %d", stats.CacheSize)
	}

	// Wait for TTL and cleanup
	time.Sleep(100 * time.Millisecond)

	stats = d.Stats()
	if stats.CacheSize != 0 {
		t.Errorf("Expected cache size 0 after cleanup, got %d", stats.CacheSize)
	}
}

func TestDeduplicator_Close(t *testing.T) {
	ctx := context.Background()
	logger := zerolog.Nop()
	cfg := DefaultConfig()

	d := New(ctx, cfg, logger)

	// Close should not panic
	d.Close()

	// Double close should not panic
	d.Close()
}

func TestCalculateHitRate(t *testing.T) {
	tests := []struct {
		hits     int64
		misses   int64
		expected float64
	}{
		{0, 0, 0},
		{1, 0, 100},
		{0, 1, 0},
		{1, 1, 50},
		{3, 1, 75},
		{1, 3, 25},
	}

	for _, tt := range tests {
		result := calculateHitRate(tt.hits, tt.misses)
		if result != tt.expected {
			t.Errorf("calculateHitRate(%d, %d) = %.2f, expected %.2f", tt.hits, tt.misses, result, tt.expected)
		}
	}
}
