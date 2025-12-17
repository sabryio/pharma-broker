package whatsapp

import (
	"context"
	"fmt"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"pharmabroker/messaging/reconnector"

	"github.com/rs/zerolog"
)

func TestConnectionState_String(t *testing.T) {
	tests := []struct {
		state    ConnectionState
		expected string
	}{
		{StateDisconnected, "DISCONNECTED"},
		{StateConnecting, "CONNECTING"},
		{StateConnected, "CONNECTED"},
		{StateReconnecting, "RECONNECTING"},
		{StateFailed, "FAILED"},
		{ConnectionState(99), "UNKNOWN"},
	}

	for _, tt := range tests {
		if got := tt.state.String(); got != tt.expected {
			t.Errorf("ConnectionState(%d).String() = %s, want %s", tt.state, got, tt.expected)
		}
	}
}

func TestDefaultReconnectorConfig(t *testing.T) {
	cfg := reconnector.DefaultReconnectorConfig()

	if cfg.MaxRetries != 0 {
		t.Errorf("MaxRetries = %d, want 0 (infinite)", cfg.MaxRetries)
	}
	if cfg.InitialInterval != 5*time.Second {
		t.Errorf("InitialInterval = %v, want 5s", cfg.InitialInterval)
	}
	if cfg.MaxInterval != 5*time.Minute {
		t.Errorf("MaxInterval = %v, want 5m", cfg.MaxInterval)
	}
	if cfg.RandomizationFactor != 0.1 {
		t.Errorf("RandomizationFactor = %f, want 0.1", cfg.RandomizationFactor)
	}
	if cfg.Multiplier != 2.0 {
		t.Errorf("Multiplier = %f, want 2.0", cfg.Multiplier)
	}
}

func TestConnectionStatus(t *testing.T) {
	status := ConnectionStatus{
		State:           StateConnected,
		ReconnectCount:  3,
		LastConnectedAt: time.Now(),
		UptimeSeconds:   3600,
	}

	if status.State != StateConnected {
		t.Errorf("State = %v, want CONNECTED", status.State)
	}
	if status.ReconnectCount != 3 {
		t.Errorf("ReconnectCount = %d, want 3", status.ReconnectCount)
	}
	if status.UptimeSeconds != 3600 {
		t.Errorf("UptimeSeconds = %d, want 3600", status.UptimeSeconds)
	}
}

type mockAlerter struct {
	called   bool
	severity string
	title    string
	message  string
}

func (m *mockAlerter) SendAlert(_ context.Context, severity, title, message string) error {
	m.called = true
	m.severity = severity
	m.title = title
	m.message = message
	return nil
}

func TestAlerterInterface(t *testing.T) {
	alerter := &mockAlerter{}

	// Verify it implements the interface
	var _ AlertNotifier = alerter

	// Test calling
	ctx := context.Background()
	err := alerter.SendAlert(ctx, "critical", "Test", "Test message")
	if err != nil {
		t.Errorf("SendAlert returned error: %v", err)
	}
	if !alerter.called {
		t.Error("SendAlert was not called")
	}
	if alerter.severity != "critical" {
		t.Errorf("severity = %s, want critical", alerter.severity)
	}
}

// =============================================================================
// History Sync Deduplication Tests
// =============================================================================

func TestHistorySyncStats(t *testing.T) {
	stats := HistorySyncStats{}

	// Test atomic operations
	stats.TotalSyncs.Add(5)
	stats.SkippedCooldown.Add(2)
	stats.MessagesReceived.Add(100)
	stats.MessagesSkipped.Add(30)
	stats.MessagesProcessed.Add(70)

	if got := stats.TotalSyncs.Load(); got != 5 {
		t.Errorf("TotalSyncs = %d, want 5", got)
	}
	if got := stats.SkippedCooldown.Load(); got != 2 {
		t.Errorf("SkippedCooldown = %d, want 2", got)
	}
	if got := stats.MessagesReceived.Load(); got != 100 {
		t.Errorf("MessagesReceived = %d, want 100", got)
	}
	if got := stats.MessagesSkipped.Load(); got != 30 {
		t.Errorf("MessagesSkipped = %d, want 30", got)
	}
	if got := stats.MessagesProcessed.Load(); got != 70 {
		t.Errorf("MessagesProcessed = %d, want 70", got)
	}
}

func TestIsMessageProcessed(t *testing.T) {
	m := &Manager{
		processedMsgIDs: make(map[string]int64),
	}

	// Initially not processed
	if m.isMessageProcessed("msg-123") {
		t.Error("Message should not be processed initially")
	}

	// Mark as processed
	m.markMessageProcessed("msg-123")

	// Now should be processed
	if !m.isMessageProcessed("msg-123") {
		t.Error("Message should be processed after marking")
	}

	// Different message should not be processed
	if m.isMessageProcessed("msg-456") {
		t.Error("Different message should not be processed")
	}
}

func TestMarkMessageProcessed(t *testing.T) {
	m := &Manager{
		processedMsgIDs: make(map[string]int64),
	}

	// Mark multiple messages
	m.markMessageProcessed("msg-1")
	m.markMessageProcessed("msg-2")
	m.markMessageProcessed("msg-3")

	// Verify all are tracked
	m.processedMsgIDsMu.RLock()
	count := len(m.processedMsgIDs)
	m.processedMsgIDsMu.RUnlock()

	if count != 3 {
		t.Errorf("Expected 3 processed messages, got %d", count)
	}

	// Verify timestamps are set
	m.processedMsgIDsMu.RLock()
	for id, ts := range m.processedMsgIDs {
		if ts <= 0 {
			t.Errorf("Message %s has invalid timestamp: %d", id, ts)
		}
	}
	m.processedMsgIDsMu.RUnlock()
}

func TestCleanupProcessedIDsCache_TTL(t *testing.T) {
	m := &Manager{
		processedMsgIDs: make(map[string]int64),
	}

	// Add some old entries (beyond TTL)
	oldTime := time.Now().Add(-2 * processedIDsCacheTTL).Unix()
	m.processedMsgIDsMu.Lock()
	m.processedMsgIDs["old-msg-1"] = oldTime
	m.processedMsgIDs["old-msg-2"] = oldTime
	m.processedMsgIDsMu.Unlock()

	// Add some recent entries
	m.markMessageProcessed("new-msg-1")
	m.markMessageProcessed("new-msg-2")

	// Run cleanup
	m.cleanupProcessedIDsCache()

	// Verify old entries are removed
	m.processedMsgIDsMu.RLock()
	defer m.processedMsgIDsMu.RUnlock()

	if _, exists := m.processedMsgIDs["old-msg-1"]; exists {
		t.Error("Old message 1 should have been cleaned up")
	}
	if _, exists := m.processedMsgIDs["old-msg-2"]; exists {
		t.Error("Old message 2 should have been cleaned up")
	}

	// Verify new entries remain
	if _, exists := m.processedMsgIDs["new-msg-1"]; !exists {
		t.Error("New message 1 should still exist")
	}
	if _, exists := m.processedMsgIDs["new-msg-2"]; !exists {
		t.Error("New message 2 should still exist")
	}
}

func TestCleanupProcessedIDsCache_Capacity(t *testing.T) {
	m := &Manager{
		processedMsgIDs: make(map[string]int64),
	}

	// Fill beyond capacity with recent entries
	now := time.Now().Unix()
	m.processedMsgIDsMu.Lock()
	for i := 0; i < processedIDsCacheSize+100; i++ {
		m.processedMsgIDs[string(rune(i))] = now
	}
	initialCount := len(m.processedMsgIDs)
	m.processedMsgIDsMu.Unlock()

	// Run cleanup
	m.cleanupProcessedIDsCache()

	// Verify size is reduced to capacity
	m.processedMsgIDsMu.RLock()
	finalCount := len(m.processedMsgIDs)
	m.processedMsgIDsMu.RUnlock()

	if finalCount > processedIDsCacheSize {
		t.Errorf("Cache size %d exceeds capacity %d after cleanup", finalCount, processedIDsCacheSize)
	}

	t.Logf("Cache reduced from %d to %d entries", initialCount, finalCount)
}

func TestResetHistorySyncState(t *testing.T) {
	m := &Manager{
		processedMsgIDs: make(map[string]int64),
	}

	// Set some state
	m.lastHistorySyncAt.Store(time.Now().Unix())
	m.markMessageProcessed("msg-1")
	m.markMessageProcessed("msg-2")

	// Verify state exists
	if m.lastHistorySyncAt.Load() == 0 {
		t.Error("lastHistorySyncAt should be set")
	}
	m.processedMsgIDsMu.RLock()
	if len(m.processedMsgIDs) != 2 {
		t.Errorf("Expected 2 processed messages, got %d", len(m.processedMsgIDs))
	}
	m.processedMsgIDsMu.RUnlock()

	// Reset state
	m.ResetHistorySyncState()

	// Verify state is cleared
	if m.lastHistorySyncAt.Load() != 0 {
		t.Error("lastHistorySyncAt should be 0 after reset")
	}
	m.processedMsgIDsMu.RLock()
	if len(m.processedMsgIDs) != 0 {
		t.Errorf("Expected 0 processed messages after reset, got %d", len(m.processedMsgIDs))
	}
	m.processedMsgIDsMu.RUnlock()
}

func TestHistorySyncCooldown(t *testing.T) {
	// Test that cooldown constant is reasonable
	if historySyncCooldown < time.Minute {
		t.Errorf("historySyncCooldown %v is too short, should be at least 1 minute", historySyncCooldown)
	}
	if historySyncCooldown > 30*time.Minute {
		t.Errorf("historySyncCooldown %v is too long, should be at most 30 minutes", historySyncCooldown)
	}
}

func TestHistorySyncMaxAge(t *testing.T) {
	// Test that max age constant is reasonable
	if historySyncMaxAge < time.Hour {
		t.Errorf("historySyncMaxAge %v is too short, should be at least 1 hour", historySyncMaxAge)
	}
	if historySyncMaxAge > 7*24*time.Hour {
		t.Errorf("historySyncMaxAge %v is too long, should be at most 7 days", historySyncMaxAge)
	}
}

func TestHistorySyncMaxMessages(t *testing.T) {
	// Test that max messages constant is reasonable
	if historySyncMaxMessages < 100 {
		t.Errorf("historySyncMaxMessages %d is too low, should be at least 100", historySyncMaxMessages)
	}
	if historySyncMaxMessages > 10000 {
		t.Errorf("historySyncMaxMessages %d is too high, should be at most 10000", historySyncMaxMessages)
	}
}

func TestProcessedIDsCacheSize(t *testing.T) {
	// Test that cache size constant is reasonable
	if processedIDsCacheSize < 1000 {
		t.Errorf("processedIDsCacheSize %d is too low, should be at least 1000", processedIDsCacheSize)
	}
	if processedIDsCacheSize > 100000 {
		t.Errorf("processedIDsCacheSize %d is too high, should be at most 100000", processedIDsCacheSize)
	}
}

func TestConcurrentMessageProcessing(t *testing.T) {
	m := &Manager{
		processedMsgIDs: make(map[string]int64),
	}

	// Simulate concurrent access
	done := make(chan bool)
	for i := 0; i < 10; i++ {
		go func(id int) {
			for j := 0; j < 100; j++ {
				msgID := string(rune(id*100 + j))
				m.markMessageProcessed(msgID)
				m.isMessageProcessed(msgID)
			}
			done <- true
		}(i)
	}

	// Wait for all goroutines
	for i := 0; i < 10; i++ {
		<-done
	}

	// Verify no race conditions (test passes if no panic)
	m.processedMsgIDsMu.RLock()
	count := len(m.processedMsgIDs)
	m.processedMsgIDsMu.RUnlock()

	if count != 1000 {
		t.Errorf("Expected 1000 processed messages, got %d", count)
	}
}

func TestHistorySyncStatsAtomicity(t *testing.T) {
	stats := HistorySyncStats{}

	// Simulate concurrent updates
	done := make(chan bool)
	for i := 0; i < 10; i++ {
		go func() {
			for j := 0; j < 100; j++ {
				stats.TotalSyncs.Add(1)
				stats.MessagesReceived.Add(10)
				stats.MessagesProcessed.Add(5)
			}
			done <- true
		}()
	}

	// Wait for all goroutines
	for i := 0; i < 10; i++ {
		<-done
	}

	// Verify counts
	if got := stats.TotalSyncs.Load(); got != 1000 {
		t.Errorf("TotalSyncs = %d, want 1000", got)
	}
	if got := stats.MessagesReceived.Load(); got != 10000 {
		t.Errorf("MessagesReceived = %d, want 10000", got)
	}
	if got := stats.MessagesProcessed.Load(); got != 5000 {
		t.Errorf("MessagesProcessed = %d, want 5000", got)
	}
}

// =============================================================================
// Group Info Cache Tests
// =============================================================================

func TestGroupInfoCache_GetSet(t *testing.T) {
	cache := NewGroupInfoCache()

	// Initially empty
	if _, found := cache.Get("group1"); found {
		t.Error("Cache should be empty initially")
	}

	// Set and get
	cache.Set("group1", "Test Group 1")
	name, found := cache.Get("group1")
	if !found {
		t.Error("Should find cached entry")
	}
	if name != "Test Group 1" {
		t.Errorf("Expected 'Test Group 1', got '%s'", name)
	}
}

func TestGroupInfoCache_TTL(t *testing.T) {
	cache := &GroupInfoCache{
		entries: make(map[string]*groupInfoEntry),
	}

	// Add an expired entry
	cache.entries["expired"] = &groupInfoEntry{
		name:      "Expired Group",
		fetchedAt: time.Now().Add(-2 * groupInfoCacheTTL),
	}

	// Should not find expired entry
	if _, found := cache.Get("expired"); found {
		t.Error("Should not find expired entry")
	}
}

func TestGroupInfoCache_Capacity(t *testing.T) {
	cache := NewGroupInfoCache()

	// Fill to capacity
	for i := 0; i < groupInfoCacheSize+10; i++ {
		cache.Set(fmt.Sprintf("group%d", i), fmt.Sprintf("Group %d", i))
	}

	// Should not exceed capacity
	if cache.Size() > groupInfoCacheSize {
		t.Errorf("Cache size %d exceeds capacity %d", cache.Size(), groupInfoCacheSize)
	}
}

func TestGroupInfoCache_Clear(t *testing.T) {
	cache := NewGroupInfoCache()

	cache.Set("group1", "Group 1")
	cache.Set("group2", "Group 2")

	if cache.Size() != 2 {
		t.Errorf("Expected size 2, got %d", cache.Size())
	}

	cache.Clear()

	if cache.Size() != 0 {
		t.Errorf("Expected size 0 after clear, got %d", cache.Size())
	}
}

func TestGroupInfoCache_Concurrent(t *testing.T) {
	cache := NewGroupInfoCache()
	done := make(chan bool)

	// Concurrent writes
	for i := 0; i < 10; i++ {
		go func(id int) {
			for j := 0; j < 100; j++ {
				cache.Set(fmt.Sprintf("group%d_%d", id, j), fmt.Sprintf("Group %d-%d", id, j))
				cache.Get(fmt.Sprintf("group%d_%d", id, j))
			}
			done <- true
		}(i)
	}

	// Wait for all goroutines
	for i := 0; i < 10; i++ {
		<-done
	}

	// Test passes if no race conditions
	t.Logf("Cache size after concurrent access: %d", cache.Size())
}

// =============================================================================
// Message Truncation Tests
// =============================================================================

func TestTruncateContent_NoTruncation(t *testing.T) {
	content := "Short message"
	result, truncated := TruncateContent(content, 100)

	if truncated {
		t.Error("Should not truncate short content")
	}
	if result != content {
		t.Errorf("Content should be unchanged, got '%s'", result)
	}
}

func TestTruncateContent_Truncation(t *testing.T) {
	content := "This is a very long message that should be truncated because it exceeds the maximum allowed size"
	result, truncated := TruncateContent(content, 50)

	if !truncated {
		t.Error("Should truncate long content")
	}
	if len(result) > 50 {
		t.Errorf("Result length %d exceeds max size 50", len(result))
	}
	if !strings.HasSuffix(result, truncatedMessageSuffix) {
		t.Errorf("Result should end with truncation suffix, got '%s'", result)
	}
}

func TestTruncateContent_WordBoundary(t *testing.T) {
	content := "Hello world this is a test message"
	result, truncated := TruncateContent(content, 20)

	if !truncated {
		t.Error("Should truncate")
	}
	// Should try to truncate at word boundary
	if strings.Contains(result[:len(result)-len(truncatedMessageSuffix)], "worl") {
		t.Error("Should truncate at word boundary, not mid-word")
	}
}

func TestTruncateContent_DefaultMaxSize(t *testing.T) {
	// Create content larger than default max
	content := strings.Repeat("a", maxMessageContentSize+100)
	result, truncated := TruncateContent(content, 0) // 0 means use default

	if !truncated {
		t.Error("Should truncate content exceeding default max")
	}
	if len(result) > maxMessageContentSize {
		t.Errorf("Result length %d exceeds default max %d", len(result), maxMessageContentSize)
	}
}

func TestFindLastSpace(t *testing.T) {
	tests := []struct {
		input    string
		expected int
	}{
		{"hello world", 5},
		{"hello\nworld", 5},
		{"hello\tworld", 5},
		{"helloworld", -1},
		{"", -1},
		{"   ", 2},
	}

	for _, tt := range tests {
		result := findLastSpace(tt.input)
		if result != tt.expected {
			t.Errorf("findLastSpace(%q) = %d, want %d", tt.input, result, tt.expected)
		}
	}
}

// =============================================================================
// Ordered Message Queue Tests
// =============================================================================

type mockHandler struct {
	messages []*IncomingMessage
	mu       sync.Mutex
}

func (h *mockHandler) HandleMessage(msg *IncomingMessage) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.messages = append(h.messages, msg)
}

func (h *mockHandler) HandleGroupJoined(group *GroupInfo) {}

func (h *mockHandler) GetMessages() []*IncomingMessage {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.messages
}

func TestOrderedMessageQueue_Basic(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	queue := NewOrderedMessageQueue(log)

	handler := &mockHandler{}
	queue.SetHandlers([]EventHandler{handler})

	// Enqueue messages
	for i := 0; i < 5; i++ {
		queue.Enqueue(&IncomingMessage{
			ID:       fmt.Sprintf("msg-%d", i),
			GroupJID: "group1",
			Content:  fmt.Sprintf("Message %d", i),
		})
	}

	// Wait for processing
	time.Sleep(100 * time.Millisecond)

	// Verify messages were processed
	messages := handler.GetMessages()
	if len(messages) != 5 {
		t.Errorf("Expected 5 messages, got %d", len(messages))
	}

	queue.Stop()
}

func TestOrderedMessageQueue_MultipleGroups(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	queue := NewOrderedMessageQueue(log)

	handler := &mockHandler{}
	queue.SetHandlers([]EventHandler{handler})

	// Enqueue messages to different groups
	for i := 0; i < 10; i++ {
		queue.Enqueue(&IncomingMessage{
			ID:       fmt.Sprintf("msg-%d", i),
			GroupJID: fmt.Sprintf("group%d", i%3),
			Content:  fmt.Sprintf("Message %d", i),
		})
	}

	// Wait for processing
	time.Sleep(100 * time.Millisecond)

	// Verify all messages were processed
	messages := handler.GetMessages()
	if len(messages) != 10 {
		t.Errorf("Expected 10 messages, got %d", len(messages))
	}

	// Check stats
	stats := queue.Stats()
	if stats.ActiveGroups != 3 {
		t.Errorf("Expected 3 active groups, got %d", stats.ActiveGroups)
	}

	queue.Stop()
}

func TestOrderedMessageQueue_Ordering(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	queue := NewOrderedMessageQueue(log)

	handler := &mockHandler{}
	queue.SetHandlers([]EventHandler{handler})

	// Enqueue messages to same group
	for i := 0; i < 10; i++ {
		queue.Enqueue(&IncomingMessage{
			ID:       fmt.Sprintf("msg-%d", i),
			GroupJID: "group1",
			Content:  fmt.Sprintf("Message %d", i),
		})
	}

	// Wait for processing
	time.Sleep(200 * time.Millisecond)

	// Verify order is preserved
	messages := handler.GetMessages()
	for i, msg := range messages {
		expected := fmt.Sprintf("msg-%d", i)
		if msg.ID != expected {
			t.Errorf("Message %d: expected ID %s, got %s", i, expected, msg.ID)
		}
	}

	queue.Stop()
}

func TestOrderedMessageQueue_Stop(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	queue := NewOrderedMessageQueue(log)

	handler := &mockHandler{}
	queue.SetHandlers([]EventHandler{handler})

	// Enqueue some messages
	queue.Enqueue(&IncomingMessage{ID: "msg-1", GroupJID: "group1"})

	// Stop should not hang
	done := make(chan bool)
	go func() {
		queue.Stop()
		done <- true
	}()

	select {
	case <-done:
		// Success
	case <-time.After(2 * time.Second):
		t.Error("Stop() timed out")
	}
}

// =============================================================================
// Message Processing Stats Tests
// =============================================================================

func TestMessageProcessingStats(t *testing.T) {
	stats := MessageProcessingStats{}

	stats.TotalReceived.Add(100)
	stats.TotalProcessed.Add(95)
	stats.TotalTruncated.Add(3)
	stats.TotalDropped.Add(2)

	if stats.TotalReceived.Load() != 100 {
		t.Errorf("TotalReceived = %d, want 100", stats.TotalReceived.Load())
	}
	if stats.TotalProcessed.Load() != 95 {
		t.Errorf("TotalProcessed = %d, want 95", stats.TotalProcessed.Load())
	}
	if stats.TotalTruncated.Load() != 3 {
		t.Errorf("TotalTruncated = %d, want 3", stats.TotalTruncated.Load())
	}
	if stats.TotalDropped.Load() != 2 {
		t.Errorf("TotalDropped = %d, want 2", stats.TotalDropped.Load())
	}
}

// =============================================================================
// Constants Validation Tests
// =============================================================================

func TestGroupInfoCacheConstants(t *testing.T) {
	if groupInfoCacheSize < 100 {
		t.Errorf("groupInfoCacheSize %d is too small", groupInfoCacheSize)
	}
	if groupInfoCacheTTL < time.Minute {
		t.Errorf("groupInfoCacheTTL %v is too short", groupInfoCacheTTL)
	}
}

func TestMessageSizeConstants(t *testing.T) {
	if maxMessageContentSize < 1000 {
		t.Errorf("maxMessageContentSize %d is too small", maxMessageContentSize)
	}
	if maxMessageContentSize > 100000 {
		t.Errorf("maxMessageContentSize %d is too large", maxMessageContentSize)
	}
}

func TestOrderedQueueConstants(t *testing.T) {
	if orderedQueueBufferSize < 10 {
		t.Errorf("orderedQueueBufferSize %d is too small", orderedQueueBufferSize)
	}
	if orderedQueueBufferSize > 1000 {
		t.Errorf("orderedQueueBufferSize %d is too large", orderedQueueBufferSize)
	}
}

// =============================================================================
// Outbound Rate Limiter Tests
// =============================================================================

func TestOutboundRateLimiter_DefaultConfig(t *testing.T) {
	cfg := DefaultOutboundRateLimiterConfig()

	if cfg.RatePerMinute != defaultOutboundRatePerMinute {
		t.Errorf("RatePerMinute = %f, want %f", cfg.RatePerMinute, float64(defaultOutboundRatePerMinute))
	}
	if cfg.BurstSize != defaultOutboundBurstSize {
		t.Errorf("BurstSize = %d, want %d", cfg.BurstSize, defaultOutboundBurstSize)
	}
	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
}

func TestOutboundRateLimiter_AllowBurst(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 60, // 1 per second
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Should allow burst of 5 immediately
	for i := 0; i < 5; i++ {
		if !rl.Allow() {
			t.Errorf("Request %d should be allowed (within burst)", i)
		}
	}

	// 6th request should be denied
	if rl.Allow() {
		t.Error("Request 6 should be denied (burst exhausted)")
	}
}

func TestOutboundRateLimiter_Wait(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 600, // 10 per second for faster test
		BurstSize:     2,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	ctx := context.Background()

	// Exhaust burst
	rl.Wait(ctx)
	rl.Wait(ctx)

	// Third request should wait
	start := time.Now()
	err := rl.Wait(ctx)
	elapsed := time.Since(start)

	if err != nil {
		t.Errorf("Wait returned error: %v", err)
	}

	// Should have waited approximately 100ms (1/10 second)
	if elapsed < 50*time.Millisecond {
		t.Errorf("Wait was too fast: %v", elapsed)
	}
}

func TestOutboundRateLimiter_WaitTimeout(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 1, // Very slow: 1 per minute
		BurstSize:     1,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Exhaust the single token
	rl.Allow()

	// Wait with short timeout should fail
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	err := rl.Wait(ctx)
	if err == nil {
		t.Error("Wait should have timed out")
	}
}

func TestOutboundRateLimiter_Disabled(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 1, // Very slow
		BurstSize:     1,
		Enabled:       false, // Disabled
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Should allow all requests when disabled
	for i := 0; i < 100; i++ {
		if !rl.Allow() {
			t.Errorf("Request %d should be allowed when disabled", i)
		}
	}
}

func TestOutboundRateLimiter_SetEnabled(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 1,
		BurstSize:     1,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Exhaust token
	rl.Allow()

	// Should be rate limited
	if rl.Allow() {
		t.Error("Should be rate limited")
	}

	// Disable
	rl.SetEnabled(false)

	// Should now allow
	if !rl.Allow() {
		t.Error("Should allow when disabled")
	}

	// Re-enable
	rl.SetEnabled(true)

	// Should be rate limited again (tokens still exhausted)
	if rl.Allow() {
		t.Error("Should be rate limited after re-enabling")
	}
}

func TestOutboundRateLimiter_SetRate(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 600, // 10 per second for faster test
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Exhaust burst
	for i := 0; i < 5; i++ {
		rl.Allow()
	}

	// Should be rate limited now
	if rl.Allow() {
		t.Error("Should be rate limited after exhausting burst")
	}

	// Update to higher rate and larger burst
	rl.SetRate(1200, 10) // 20 per second

	// Wait for refill (should get ~2 tokens in 100ms at 20/sec)
	time.Sleep(150 * time.Millisecond)

	// Should have some tokens now
	if !rl.Allow() {
		t.Error("Should have tokens after rate change and refill")
	}
}

func TestOutboundRateLimiter_GetStats(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 60,
		BurstSize:     3,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Make some requests
	rl.Allow() // Allowed
	rl.Allow() // Allowed
	rl.Allow() // Allowed
	rl.Allow() // Dropped

	stats := rl.GetStats()

	if stats["total_requests"] != 4 {
		t.Errorf("total_requests = %d, want 4", stats["total_requests"])
	}
	if stats["total_allowed"] != 3 {
		t.Errorf("total_allowed = %d, want 3", stats["total_allowed"])
	}
	if stats["total_dropped"] != 1 {
		t.Errorf("total_dropped = %d, want 1", stats["total_dropped"])
	}
}

func TestOutboundRateLimiter_GetCurrentTokens(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 60,
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Initially should have burst size tokens
	tokens := rl.GetCurrentTokens()
	if tokens != 5.0 {
		t.Errorf("Initial tokens = %f, want 5.0", tokens)
	}

	// Use some tokens
	rl.Allow()
	rl.Allow()

	tokens = rl.GetCurrentTokens()
	if tokens < 2.9 || tokens > 3.1 {
		t.Errorf("After 2 requests, tokens = %f, want ~3.0", tokens)
	}
}

func TestOutboundRateLimiter_Reset(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 60,
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Exhaust all tokens
	for i := 0; i < 5; i++ {
		rl.Allow()
	}

	// Should be empty
	if rl.Allow() {
		t.Error("Should be rate limited")
	}

	// Reset
	rl.Reset()

	// Should have full burst again
	for i := 0; i < 5; i++ {
		if !rl.Allow() {
			t.Errorf("Request %d should be allowed after reset", i)
		}
	}
}

func TestOutboundRateLimiter_TokenRefill(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 600, // 10 per second
		BurstSize:     2,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	// Exhaust tokens
	rl.Allow()
	rl.Allow()

	// Wait for refill (should get ~1 token in 100ms)
	time.Sleep(150 * time.Millisecond)

	// Should have at least 1 token now
	if !rl.Allow() {
		t.Error("Should have refilled at least 1 token")
	}
}

func TestOutboundRateLimiter_Concurrent(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 6000, // 100 per second
		BurstSize:     50,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	var wg sync.WaitGroup
	allowed := atomic.Int64{}
	dropped := atomic.Int64{}

	// Concurrent requests
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 20; j++ {
				if rl.Allow() {
					allowed.Add(1)
				} else {
					dropped.Add(1)
				}
			}
		}()
	}

	wg.Wait()

	total := allowed.Load() + dropped.Load()
	if total != 200 {
		t.Errorf("Total requests = %d, want 200", total)
	}

	// Should have allowed at least burst size
	if allowed.Load() < int64(cfg.BurstSize) {
		t.Errorf("Allowed = %d, should be at least %d", allowed.Load(), cfg.BurstSize)
	}

	t.Logf("Concurrent test: allowed=%d, dropped=%d", allowed.Load(), dropped.Load())
}

func TestOutboundRateLimiter_WaitConcurrent(t *testing.T) {
	log := zerolog.New(zerolog.NewTestWriter(t))
	cfg := OutboundRateLimiterConfig{
		RatePerMinute: 600, // 10 per second
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewOutboundRateLimiter(cfg, log)

	var wg sync.WaitGroup
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	success := atomic.Int64{}
	failed := atomic.Int64{}

	// 10 concurrent waiters
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if err := rl.Wait(ctx); err != nil {
				failed.Add(1)
			} else {
				success.Add(1)
			}
		}()
	}

	wg.Wait()

	// All should eventually succeed within 2 seconds
	if success.Load() != 10 {
		t.Errorf("Success = %d, want 10 (failed = %d)", success.Load(), failed.Load())
	}
}

func TestOutboundRateLimiterStats_Atomicity(t *testing.T) {
	stats := OutboundRateLimiterStats{}

	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 100; j++ {
				stats.TotalRequests.Add(1)
				stats.TotalAllowed.Add(1)
			}
		}()
	}

	wg.Wait()

	if stats.TotalRequests.Load() != 1000 {
		t.Errorf("TotalRequests = %d, want 1000", stats.TotalRequests.Load())
	}
	if stats.TotalAllowed.Load() != 1000 {
		t.Errorf("TotalAllowed = %d, want 1000", stats.TotalAllowed.Load())
	}
}

// =============================================================================
// Rate Limiter Constants Tests
// =============================================================================

func TestRateLimiterConstants(t *testing.T) {
	if defaultOutboundRatePerMinute < 10 {
		t.Errorf("defaultOutboundRatePerMinute %d is too low", defaultOutboundRatePerMinute)
	}
	if defaultOutboundRatePerMinute > 100 {
		t.Errorf("defaultOutboundRatePerMinute %d is too high (risk of ban)", defaultOutboundRatePerMinute)
	}
	if defaultOutboundBurstSize < 1 {
		t.Errorf("defaultOutboundBurstSize %d is too low", defaultOutboundBurstSize)
	}
	if defaultOutboundBurstSize > 20 {
		t.Errorf("defaultOutboundBurstSize %d is too high", defaultOutboundBurstSize)
	}
	if rateLimitWaitTimeout < 5*time.Second {
		t.Errorf("rateLimitWaitTimeout %v is too short", rateLimitWaitTimeout)
	}
}
