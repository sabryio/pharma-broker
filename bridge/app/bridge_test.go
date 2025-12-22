package app

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharma-bridge/domain"
)

// --- Mock implementations ---

type mockMessageSource struct {
	messages  chan domain.Message
	connected bool
	mu        sync.Mutex
}

func newMockMessageSource() *mockMessageSource {
	return &mockMessageSource{messages: make(chan domain.Message, 100)}
}

func (m *mockMessageSource) Connect(context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.connected = true
	return nil
}

func (m *mockMessageSource) Disconnect() {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.connected = false
	close(m.messages)
}

func (m *mockMessageSource) Messages() <-chan domain.Message { return m.messages }

func (m *mockMessageSource) IsConnected() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.connected
}

type mockMessageSink struct {
	messages []domain.Message
	mu       sync.Mutex
	err      error
}

func newMockMessageSink() *mockMessageSink {
	return &mockMessageSink{messages: make([]domain.Message, 0)}
}

func (m *mockMessageSink) Send(_ context.Context, msg domain.Message) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.err != nil {
		return m.err
	}
	m.messages = append(m.messages, msg)
	return nil
}

func (m *mockMessageSink) Close() error { return nil }

func (m *mockMessageSink) GetMessages() []domain.Message {
	m.mu.Lock()
	defer m.mu.Unlock()
	result := make([]domain.Message, len(m.messages))
	copy(result, m.messages)
	return result
}

type mockGroupCache struct {
	monitored map[domain.JID]bool
	mu        sync.RWMutex
}

func newMockGroupCache() *mockGroupCache {
	return &mockGroupCache{monitored: make(map[domain.JID]bool)}
}

func (m *mockGroupCache) IsMonitored(jid domain.JID) bool {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.monitored[jid]
}

func (m *mockGroupCache) Update(jids []domain.JID) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.monitored = make(map[domain.JID]bool)
	for _, jid := range jids {
		m.monitored[jid] = true
	}
}

type mockGroupRepository struct {
	jids []domain.JID
}

func (m *mockGroupRepository) GetMonitoredGroups(context.Context) ([]domain.JID, error) {
	return m.jids, nil
}

type mockDeduplicator struct {
	seen map[string]bool
	mu   sync.Mutex
}

func newMockDeduplicator() *mockDeduplicator {
	return &mockDeduplicator{seen: make(map[string]bool)}
}

func (m *mockDeduplicator) IsDuplicate(groupJID, senderJID domain.JID, content string, _ time.Time) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	key := string(groupJID) + "|" + string(senderJID) + "|" + content
	return m.seen[key]
}

func (m *mockDeduplicator) Record(groupJID, senderJID domain.JID, content string, _ time.Time) {
	m.mu.Lock()
	defer m.mu.Unlock()
	key := string(groupJID) + "|" + string(senderJID) + "|" + content
	m.seen[key] = true
}

func (m *mockDeduplicator) Close() {}

type mockRateLimiter struct{ allow bool }

func (m *mockRateLimiter) Allow() bool { return m.allow }

// --- Tests ---

func TestNewBridge(t *testing.T) {
	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        newMockMessageSink(),
		GroupCache:  newMockGroupCache(),
		GroupRepo:   &mockGroupRepository{jids: []domain.JID{"group1@g.us"}},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	if bridge == nil {
		t.Fatal("Expected bridge to be created")
	}
	if len(bridge.workers) != 20 {
		t.Errorf("Expected 20 workers, got %d", len(bridge.workers))
	}
}

func TestBridge_ProcessMessage(t *testing.T) {
	sink := newMockMessageSink()
	groupCache := newMockGroupCache()
	groupCache.Update([]domain.JID{"group1@g.us"})

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  groupCache,
		GroupRepo:   &mockGroupRepository{jids: []domain.JID{"group1@g.us"}},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	msg := domain.Message{
		ID:        "msg1",
		GroupJID:  "group1@g.us",
		SenderJID: "sender1@s.whatsapp.net",
		Content:   "Hello",
		Timestamp: domain.UnixTimestamp(time.Now().Unix()),
		IsGroup:   true,
		IsFromMe:  false,
	}

	bridge.processMessage(context.Background(), msg)
	time.Sleep(50 * time.Millisecond)

	messages := sink.GetMessages()
	if len(messages) != 1 {
		t.Errorf("Expected 1 message, got %d", len(messages))
	}
}

func TestBridge_SkipNonGroupMessages(t *testing.T) {
	sink := newMockMessageSink()

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  newMockGroupCache(),
		GroupRepo:   &mockGroupRepository{},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	msg := domain.Message{ID: "msg1", Content: "Hello", IsGroup: false}
	bridge.processMessage(context.Background(), msg)

	if len(sink.GetMessages()) != 0 {
		t.Error("Expected 0 messages for non-group message")
	}
}

func TestBridge_SkipOwnMessages(t *testing.T) {
	sink := newMockMessageSink()
	groupCache := newMockGroupCache()
	groupCache.Update([]domain.JID{"group1@g.us"})

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  groupCache,
		GroupRepo:   &mockGroupRepository{jids: []domain.JID{"group1@g.us"}},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	msg := domain.Message{ID: "msg1", GroupJID: "group1@g.us", Content: "Hello", IsGroup: true, IsFromMe: true}
	bridge.processMessage(context.Background(), msg)

	if len(sink.GetMessages()) != 0 {
		t.Error("Expected 0 messages for own message")
	}
}

func TestBridge_SkipUnmonitoredGroups(t *testing.T) {
	sink := newMockMessageSink()

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  newMockGroupCache(), // Empty cache
		GroupRepo:   &mockGroupRepository{},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	msg := domain.Message{ID: "msg1", GroupJID: "group1@g.us", Content: "Hello", IsGroup: true}
	bridge.processMessage(context.Background(), msg)

	if len(sink.GetMessages()) != 0 {
		t.Error("Expected 0 messages for unmonitored group")
	}
}

func TestBridge_SkipDuplicates(t *testing.T) {
	sink := newMockMessageSink()
	groupCache := newMockGroupCache()
	groupCache.Update([]domain.JID{"group1@g.us"})

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  groupCache,
		GroupRepo:   &mockGroupRepository{jids: []domain.JID{"group1@g.us"}},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	msg := domain.Message{
		ID:        "msg1",
		GroupJID:  "group1@g.us",
		SenderJID: "sender1@s.whatsapp.net",
		Content:   "Hello",
		Timestamp: domain.UnixTimestamp(time.Now().Unix()),
		IsGroup:   true,
	}

	ctx := context.Background()
	bridge.processMessage(ctx, msg)
	bridge.processMessage(ctx, msg) // Duplicate

	time.Sleep(50 * time.Millisecond)

	if len(sink.GetMessages()) != 1 {
		t.Errorf("Expected 1 message (duplicate skipped), got %d", len(sink.GetMessages()))
	}
}

func TestBridge_RateLimiting(t *testing.T) {
	sink := newMockMessageSink()
	groupCache := newMockGroupCache()
	groupCache.Update([]domain.JID{"group1@g.us"})

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  groupCache,
		GroupRepo:   &mockGroupRepository{jids: []domain.JID{"group1@g.us"}},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: false}, // Rate limited
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	msg := domain.Message{
		ID:        "msg1",
		GroupJID:  "group1@g.us",
		SenderJID: "sender1@s.whatsapp.net",
		Content:   "Hello",
		Timestamp: domain.UnixTimestamp(time.Now().Unix()),
		IsGroup:   true,
	}

	bridge.processMessage(context.Background(), msg)

	if len(sink.GetMessages()) != 0 {
		t.Error("Expected 0 messages (rate limited)")
	}
}

func TestBridge_MessagesForwarded(t *testing.T) {
	sink := newMockMessageSink()
	groupCache := newMockGroupCache()
	groupCache.Update([]domain.JID{"group1@g.us"})

	bridge := NewBridge(BridgeParams{
		Source:      newMockMessageSource(),
		Sink:        sink,
		GroupCache:  groupCache,
		GroupRepo:   &mockGroupRepository{jids: []domain.JID{"group1@g.us"}},
		Dedup:       newMockDeduplicator(),
		RateLimiter: &mockRateLimiter{allow: true},
		Logger:      zerolog.Nop(),
		Config:      DefaultBridgeConfig(),
	})

	ctx := context.Background()
	for i := 0; i < 5; i++ {
		msg := domain.Message{
			ID:        domain.MessageID("msg" + string(rune('0'+i))),
			GroupJID:  "group1@g.us",
			SenderJID: domain.JID("sender" + string(rune('0'+i)) + "@s.whatsapp.net"),
			Content:   "Hello " + string(rune('0'+i)),
			Timestamp: domain.UnixTimestamp(time.Now().Unix()),
			IsGroup:   true,
		}
		bridge.processMessage(ctx, msg)
	}

	time.Sleep(50 * time.Millisecond)

	if bridge.MessagesForwarded() != 5 {
		t.Errorf("Expected 5 messages forwarded, got %d", bridge.MessagesForwarded())
	}
}

func TestDefaultBridgeConfig(t *testing.T) {
	cfg := DefaultBridgeConfig()

	if !cfg.SkipOwnMessages {
		t.Error("Expected SkipOwnMessages to be true")
	}
	if cfg.WorkerCount != 20 {
		t.Errorf("Expected WorkerCount 20, got %d", cfg.WorkerCount)
	}
	if cfg.WorkerQueueSize != 100 {
		t.Errorf("Expected WorkerQueueSize 100, got %d", cfg.WorkerQueueSize)
	}
}

func TestHashJID(t *testing.T) {
	jid1 := domain.JID("group1@g.us")
	jid2 := domain.JID("group2@g.us")

	hash1 := hashJID(jid1)
	hash2 := hashJID(jid2)

	if hash1 < 0 {
		t.Error("Hash should be non-negative")
	}
	if hash1 == hash2 {
		t.Error("Different JIDs should have different hashes")
	}

	// Same JID should produce same hash
	if hashJID(jid1) != hash1 {
		t.Error("Same JID should produce same hash")
	}
}
