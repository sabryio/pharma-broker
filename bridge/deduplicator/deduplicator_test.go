package deduplicator

import (
	"context"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharma-bridge/domain"
)

func testConfig() Config {
	return Config{
		Window:          10 * time.Second,
		CacheSize:       10000,
		CacheTTL:        30 * time.Second,
		CleanupInterval: time.Minute,
	}
}

func TestNew(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	d := New(ctx, testConfig(), zerolog.Nop())
	defer d.Close()

	if d == nil {
		t.Fatal("Expected deduplicator to be created")
	}
}

func TestDeduplicator_IsDuplicate(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	d := New(ctx, testConfig(), zerolog.Nop())
	defer d.Close()

	groupJID := domain.JID("group@g.us")
	senderJID := domain.JID("sender@s.whatsapp.net")
	content := "Hello"
	ts := time.Now()

	// First message should not be duplicate
	if d.IsDuplicate(groupJID, senderJID, content, ts) {
		t.Error("First message should not be duplicate")
	}

	// Record the message
	d.Record(groupJID, senderJID, content, ts)

	// Same message should be duplicate
	if !d.IsDuplicate(groupJID, senderJID, content, ts) {
		t.Error("Same message should be duplicate")
	}

	// Different content should not be duplicate
	if d.IsDuplicate(groupJID, senderJID, "Different", ts) {
		t.Error("Different content should not be duplicate")
	}
}

func TestDeduplicator_CrossGroupDedup(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	d := New(ctx, testConfig(), zerolog.Nop())
	defer d.Close()

	group1 := domain.JID("group1@g.us")
	group2 := domain.JID("group2@g.us")
	senderJID := domain.JID("sender@s.whatsapp.net")
	content := "Hello"
	ts := time.Now()

	// Record in group 1
	d.Record(group1, senderJID, content, ts)

	// Same message from same sender in group 2 should now be duplicate
	if !d.IsDuplicate(group2, senderJID, content, ts) {
		t.Error("Same message from same sender in different group should be duplicate")
	}
}

func TestDeduplicator_WindowExpiry(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cfg := Config{
		Window:          100 * time.Millisecond,
		CacheSize:       100,
		CacheTTL:        time.Hour,
		CleanupInterval: time.Hour,
	}

	d := New(ctx, cfg, zerolog.Nop())
	defer d.Close()

	groupJID := domain.JID("group@g.us")
	senderJID := domain.JID("sender@s.whatsapp.net")
	content := "Hello"
	ts := time.Now()

	d.Record(groupJID, senderJID, content, ts)

	// Within window - should be duplicate
	if !d.IsDuplicate(groupJID, senderJID, content, ts.Add(50*time.Millisecond)) {
		t.Error("Message within window should be duplicate")
	}

	// Outside window - should not be duplicate
	if d.IsDuplicate(groupJID, senderJID, content, ts.Add(200*time.Millisecond)) {
		t.Error("Message outside window should not be duplicate")
	}
}

func TestDeduplicator_CacheTTL(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cfg := Config{
		Window:          time.Hour,
		CacheSize:       100,
		CacheTTL:        100 * time.Millisecond,
		CleanupInterval: time.Hour,
	}

	d := New(ctx, cfg, zerolog.Nop())
	defer d.Close()

	groupJID := domain.JID("group@g.us")
	senderJID := domain.JID("sender@s.whatsapp.net")
	content := "Hello"
	ts := time.Now()

	d.Record(groupJID, senderJID, content, ts)

	// Wait for TTL to expire
	time.Sleep(150 * time.Millisecond)

	// Should not be duplicate after TTL
	if d.IsDuplicate(groupJID, senderJID, content, ts) {
		t.Error("Message should not be duplicate after TTL")
	}
}

func TestDeduplicator_CacheSizeLimit(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cfg := Config{
		Window:          time.Hour,
		CacheSize:       3,
		CacheTTL:        time.Hour,
		CleanupInterval: time.Hour,
	}

	d := New(ctx, cfg, zerolog.Nop())
	defer d.Close()

	ts := time.Now()

	// Fill cache
	for i := 0; i < 5; i++ {
		d.Record(domain.JID("group@g.us"), domain.JID("sender"+string(rune('0'+i))+"@s.whatsapp.net"), "Hello", ts)
	}

	stats := d.Stats()
	if stats.CacheSize > 3 {
		t.Errorf("Cache size should not exceed limit, got %d", stats.CacheSize)
	}
}

func TestDeduplicator_Stats(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	d := New(ctx, testConfig(), zerolog.Nop())
	defer d.Close()

	groupJID := domain.JID("group@g.us")
	senderJID := domain.JID("sender@s.whatsapp.net")
	ts := time.Now()

	// Miss
	d.IsDuplicate(groupJID, senderJID, "Hello", ts)
	d.Record(groupJID, senderJID, "Hello", ts)

	// Hit
	d.IsDuplicate(groupJID, senderJID, "Hello", ts)

	stats := d.Stats()
	if stats.Hits != 1 {
		t.Errorf("Expected 1 hit, got %d", stats.Hits)
	}
	if stats.Misses != 1 {
		t.Errorf("Expected 1 miss, got %d", stats.Misses)
	}
}

func TestDeduplicator_Cleanup(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cfg := Config{
		Window:          time.Hour,
		CacheSize:       100,
		CacheTTL:        50 * time.Millisecond,
		CleanupInterval: 50 * time.Millisecond,
	}

	d := New(ctx, cfg, zerolog.Nop())
	defer d.Close()

	d.Record(domain.JID("group@g.us"), domain.JID("sender@s.whatsapp.net"), "Hello", time.Now())

	// Wait for cleanup
	time.Sleep(150 * time.Millisecond)

	stats := d.Stats()
	if stats.CacheSize != 0 {
		t.Errorf("Expected cache to be empty after cleanup, got %d", stats.CacheSize)
	}
}

func TestDeduplicator_Close(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	d := New(ctx, testConfig(), zerolog.Nop())
	d.Close()

	// Should not panic
}

func TestCalculateHitRate(t *testing.T) {
	tests := []struct {
		hits, misses int64
		expected     float64
	}{
		{0, 0, 0},
		{1, 0, 100},
		{0, 1, 0},
		{1, 1, 50},
		{3, 1, 75},
	}

	for _, tt := range tests {
		result := calculateHitRate(tt.hits, tt.misses)
		if result != tt.expected {
			t.Errorf("calculateHitRate(%d, %d) = %f, expected %f", tt.hits, tt.misses, result, tt.expected)
		}
	}
}
