package whatsapp

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/rs/zerolog"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

type MockMessage struct {
	GroupJID  string
	SenderJID string
	Content   string
	Timestamp time.Time
}

func (rm *MockMessage) GetTimestamp() time.Time {
	return rm.Timestamp
}

func (rm *MockMessage) GetContent() string {
	return rm.Content
}

// mockLookup is a mock implementation of Lookup[*entity.RawMessage] for testing.
type mockLookup struct {
	mu       sync.Mutex
	messages map[string]*MockMessage
}

func newMockLookup() *mockLookup {
	return &mockLookup{
		messages: make(map[string]*MockMessage),
	}
}

// GetLast implements Lookup[*entity.RawMessage].
func (m *mockLookup) GetLast(ctx context.Context, groupID, senderID string) (*MockMessage, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	key := groupID + "|" + senderID
	return m.messages[key], nil
}

func (m *mockLookup) SetLastMessage(groupJID, senderJID string, msg *MockMessage) {
	m.mu.Lock()
	defer m.mu.Unlock()

	key := groupJID + "|" + senderJID
	m.messages[key] = msg
}

func TestNewDeduplicator(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	d := NewDeduplicator[*MockMessage](cfg, nil, zerolog.Nop())

	require.NotNil(t, d)
	assert.NotNil(t, d.cache)
	assert.Equal(t, cfg.Window, d.cfg.Window)
}

func TestDeduplicator_NoDuplicate_NewMessage(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	// New message should not be duplicate
	isDupe := d.IsDuplicate(context.Background(), "group1", "sender1", "Hello", time.Now())
	assert.False(t, isDupe)
}

func TestDeduplicator_Duplicate_SameContentWithinWindow(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	cfg.Window = 10 * time.Second
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"
	content := "Test message"

	// Record first message
	d.RecordMessage(groupJID, senderJID, content, now)

	// Same content within window should be duplicate
	isDupe := d.IsDuplicate(context.Background(), groupJID, senderJID, content, now.Add(5*time.Second))
	assert.True(t, isDupe)
}

func TestDeduplicator_NoDuplicate_SameContentOutsideWindow(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	cfg.Window = 10 * time.Second
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"
	content := "Test message"

	// Record first message
	d.RecordMessage(groupJID, senderJID, content, now)

	// Same content outside window should NOT be duplicate
	isDupe := d.IsDuplicate(context.Background(), groupJID, senderJID, content, now.Add(15*time.Second))
	assert.False(t, isDupe)
}

func TestDeduplicator_NoDuplicate_DifferentContent(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"

	// Record first message
	d.RecordMessage(groupJID, senderJID, "Message 1", now)

	// Different content should NOT be duplicate
	isDupe := d.IsDuplicate(context.Background(), groupJID, senderJID, "Message 2", now.Add(5*time.Second))
	assert.False(t, isDupe)
}

func TestDeduplicator_NoDuplicate_DifferentSender(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	content := "Same message"

	// Record message from sender1
	d.RecordMessage(groupJID, "sender1@s.whatsapp.net", content, now)

	// Same content from different sender should NOT be duplicate
	isDupe := d.IsDuplicate(context.Background(), groupJID, "sender2@s.whatsapp.net", content, now.Add(2*time.Second))
	assert.False(t, isDupe)
}

func TestDeduplicator_NoDuplicate_DifferentGroup(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	senderJID := "sender@s.whatsapp.net"
	content := "Same message"

	// Record message in group1
	d.RecordMessage("group1@s.whatsapp.net", senderJID, content, now)

	// Same content in different group should NOT be duplicate
	isDupe := d.IsDuplicate(context.Background(), "group2@s.whatsapp.net", senderJID, content, now.Add(2*time.Second))
	assert.False(t, isDupe)
}

func TestDeduplicator_DBFallback(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	cfg.UseInMemoryCache = false // Disable cache
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"
	content := "Test message"

	// Set up DB with previous message
	lookup.SetLastMessage(groupJID, senderJID, &MockMessage{
		GroupJID:  groupJID,
		SenderJID: senderJID,
		Content:   content,
		Timestamp: now,
	})

	// Should find duplicate via DB lookup
	isDupe := d.IsDuplicate(context.Background(), groupJID, senderJID, content, now.Add(5*time.Second))
	assert.True(t, isDupe)
}

func TestDeduplicator_DBFallback_NoDuplicate(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	cfg.UseInMemoryCache = false // Disable cache
	lookup := newMockLookup()
	d := NewDeduplicator(cfg, lookup, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"

	// Set up DB with previous message
	lookup.SetLastMessage(groupJID, senderJID, &MockMessage{
		GroupJID:  groupJID,
		SenderJID: senderJID,
		Content:   "Old message",
		Timestamp: now,
	})

	// Different content should NOT be duplicate
	isDupe := d.IsDuplicate(context.Background(), groupJID, senderJID, "New message", now.Add(5*time.Second))
	assert.False(t, isDupe)
}

func TestDeduplicator_CacheEviction(t *testing.T) {
	cfg := DeduplicatorConfig{
		Window:           10 * time.Second,
		UseInMemoryCache: true,
		CacheSize:        3, // Small cache
		CacheTTL:         30 * time.Second,
	}
	d := NewDeduplicator[*MockMessage](cfg, nil, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"

	// Fill cache beyond capacity
	for i := 0; i < 5; i++ {
		senderJID := "sender" + string(rune('a'+i)) + "@s.whatsapp.net"
		d.RecordMessage(groupJID, senderJID, "Message", now.Add(time.Duration(i)*time.Second))
	}

	// Cache should be capped at size
	stats := d.Stats()
	assert.LessOrEqual(t, stats.CacheSize, cfg.CacheSize)
}

func TestDeduplicator_Stats(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	d := NewDeduplicator[*MockMessage](cfg, nil, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"

	// Record a message
	d.RecordMessage(groupJID, senderJID, "Test", now)

	// Check for same message (hit)
	d.IsDuplicate(context.Background(), groupJID, senderJID, "Test", now.Add(time.Second))

	// Check for different message (miss)
	d.IsDuplicate(context.Background(), groupJID, senderJID, "Different", now.Add(2*time.Second))

	stats := d.Stats()
	assert.Equal(t, int64(1), stats.Hits)
	assert.Equal(t, int64(1), stats.Misses)
	assert.Equal(t, 50.0, stats.HitRate)
}

func TestDeduplicator_Clear(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	d := NewDeduplicator[*MockMessage](cfg, nil, zerolog.Nop())

	// Add some entries
	d.RecordMessage("group", "sender", "content", time.Now())
	d.IsDuplicate(context.Background(), "group", "sender", "content", time.Now())

	// Clear
	d.Clear()

	stats := d.Stats()
	assert.Equal(t, 0, stats.CacheSize)
	assert.Equal(t, int64(0), stats.Hits)
	assert.Equal(t, int64(0), stats.Misses)
}

func TestDeduplicator_ConcurrentAccess(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()
	d := NewDeduplicator[*MockMessage](cfg, nil, zerolog.Nop())

	now := time.Now()
	var wg sync.WaitGroup

	// Concurrent writes
	for i := range 100 {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			groupJID := "group@s.whatsapp.net"
			senderJID := "sender" + string(rune('a'+idx%26)) + "@s.whatsapp.net"
			d.RecordMessage(groupJID, senderJID, "Message", now)
		}(i)
	}

	// Concurrent reads
	for i := range 100 {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			groupJID := "group@s.whatsapp.net"
			senderJID := "sender" + string(rune('a'+idx%26)) + "@s.whatsapp.net"
			d.IsDuplicate(context.Background(), groupJID, senderJID, "Message", now.Add(time.Second))
		}(i)
	}

	wg.Wait()
	// Should not panic or deadlock
}

func TestDeduplicator_ExpiredCache(t *testing.T) {
	cfg := DeduplicatorConfig{
		Window:           10 * time.Second,
		UseInMemoryCache: true,
		CacheSize:        100,
		CacheTTL:         100 * time.Millisecond, // Very short TTL for testing
	}
	d := NewDeduplicator[*MockMessage](cfg, nil, zerolog.Nop())

	now := time.Now()
	groupJID := "group@s.whatsapp.net"
	senderJID := "sender@s.whatsapp.net"
	content := "Test message"

	// Record message
	d.RecordMessage(groupJID, senderJID, content, now)

	// Wait for TTL to expire
	time.Sleep(150 * time.Millisecond)

	// Should NOT be duplicate because cache entry expired
	isDupe := d.IsDuplicate(context.Background(), groupJID, senderJID, content, now.Add(50*time.Millisecond))
	assert.False(t, isDupe)
}

func TestDefaultDeduplicatorConfig(t *testing.T) {
	cfg := DefaultDeduplicatorConfig()

	assert.Equal(t, 10*time.Second, cfg.Window)
	assert.True(t, cfg.UseInMemoryCache)
	assert.Equal(t, 10000, cfg.CacheSize)
	assert.Equal(t, 30*time.Second, cfg.CacheTTL)
}

func TestAbsDuration(t *testing.T) {
	tests := []struct {
		name     string
		input    time.Duration
		expected time.Duration
	}{
		{"positive", 5 * time.Second, 5 * time.Second},
		{"negative", -5 * time.Second, 5 * time.Second},
		{"zero", 0, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := absDuration(tt.input)
			assert.Equal(t, tt.expected, result)
		})
	}
}
