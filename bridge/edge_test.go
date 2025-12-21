// Package bridge provides edge case tests for panic prevention and race condition detection.
// Run with: go test -race -v ./... -run Edge
package bridge

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/rs/zerolog"

	resilienceadapter "pharma-bridge/adapters/resilience"
	"pharma-bridge/app"
	"pharma-bridge/deduplicator"
	"pharma-bridge/domain"
	"pharma-bridge/historysync"
	"pharma-bridge/ports"
	"pharma-bridge/qr"
	"pharma-bridge/reconnector"
	"pharma-bridge/resilience"
)

// --- QR Handler Edge Cases ---

func TestEdge_QRHandler_DoubleClose(t *testing.T) {
	cfg := qr.Config{RenderTerminal: false, QRTimeout: time.Minute, MaxRetries: 5}
	h := qr.New(cfg, zerolog.Nop())

	// First close should work
	h.Close()

	// Second close should not panic
	h.Close()
}

func TestEdge_QRHandler_CloseWhileHandling(t *testing.T) {
	cfg := qr.Config{RenderTerminal: false, QRTimeout: time.Minute, MaxRetries: 5}
	h := qr.New(cfg, zerolog.Nop())

	var wg sync.WaitGroup
	wg.Add(2)

	// Concurrent QR code handling
	go func() {
		defer wg.Done()
		for i := 0; i < 100; i++ {
			h.HandleQRCode("test-code", cfg)
		}
	}()

	// Concurrent close
	go func() {
		defer wg.Done()
		time.Sleep(10 * time.Millisecond)
		h.Close()
	}()

	wg.Wait()
}

func TestEdge_QRHandler_ConcurrentStateAccess(t *testing.T) {
	cfg := qr.Config{RenderTerminal: false, QRTimeout: time.Minute, MaxRetries: 5}
	h := qr.New(cfg, zerolog.Nop())
	defer h.Close()

	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(4)

		go func() {
			defer wg.Done()
			h.HandleQRCode("test", cfg)
		}()

		go func() {
			defer wg.Done()
			h.HandleEvent("success", cfg)
		}()

		go func() {
			defer wg.Done()
			h.GetState()
		}()

		go func() {
			defer wg.Done()
			h.IsPaired()
		}()
	}

	wg.Wait()
}

// --- RetrySender Edge Cases ---

type mockSink struct {
	sendErr error
	closed  bool
	mu      sync.Mutex
}

func (m *mockSink) Send(ctx context.Context, msg domain.Message) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.sendErr
}

func (m *mockSink) Close() error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.closed = true
	return nil
}

func TestEdge_RetrySender_DoubleClose(t *testing.T) {
	sink := &mockSink{}
	cfg := resilienceadapter.RetrySenderConfig{MaxSize: 10, FlushInterval: time.Second}
	rs := resilienceadapter.NewRetrySender(sink, cfg, zerolog.Nop())

	// First close
	rs.Close()

	// Second close should not panic
	rs.Close()
}

func TestEdge_RetrySender_CloseWhileSending(t *testing.T) {
	sink := &mockSink{}
	cfg := resilienceadapter.RetrySenderConfig{MaxSize: 100, FlushInterval: 10 * time.Millisecond}
	rs := resilienceadapter.NewRetrySender(sink, cfg, zerolog.Nop())

	ctx, cancel := context.WithCancel(context.Background())
	rs.Start(ctx, cfg)

	var wg sync.WaitGroup
	wg.Add(2)

	// Concurrent sends
	go func() {
		defer wg.Done()
		for i := 0; i < 100; i++ {
			rs.Send(ctx, domain.Message{ID: domain.MessageID("test")})
		}
	}()

	// Close while sending
	go func() {
		defer wg.Done()
		time.Sleep(5 * time.Millisecond)
		cancel()
		rs.Close()
	}()

	wg.Wait()
}

func TestEdge_RetrySender_ConcurrentSizeAccess(t *testing.T) {
	sink := &mockSink{}
	cfg := resilienceadapter.RetrySenderConfig{MaxSize: 100, FlushInterval: time.Second}
	rs := resilienceadapter.NewRetrySender(sink, cfg, zerolog.Nop())
	defer rs.Close()

	ctx := context.Background()
	rs.Start(ctx, cfg)

	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(2)

		go func() {
			defer wg.Done()
			rs.Send(ctx, domain.Message{ID: domain.MessageID("test")})
		}()

		go func() {
			defer wg.Done()
			rs.Size()
		}()
	}

	wg.Wait()
}

// --- RetryBuffer Edge Cases ---

func TestEdge_RetryBuffer_DoubleStop(t *testing.T) {
	rb := resilience.NewRetryBuffer(10, nil)

	// First stop
	rb.Stop()

	// Second stop should not panic
	rb.Stop()
}

// --- Deduplicator Edge Cases ---

func TestEdge_Deduplicator_DoubleClose(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cfg := deduplicator.Config{
		Window:          time.Second,
		CacheSize:       100,
		CacheTTL:        time.Second,
		CleanupInterval: time.Hour,
	}
	d := deduplicator.New(ctx, cfg, zerolog.Nop())

	// First close
	d.Close()

	// Second close should not panic
	d.Close()
}

func TestEdge_Deduplicator_ConcurrentAccess(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cfg := deduplicator.Config{
		Window:          time.Second,
		CacheSize:       100,
		CacheTTL:        time.Second,
		CleanupInterval: 10 * time.Millisecond,
	}
	d := deduplicator.New(ctx, cfg, zerolog.Nop())
	defer d.Close()

	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(3)

		go func(i int) {
			defer wg.Done()
			jid := domain.JID("group@g.us")
			sender := domain.JID("sender@s.whatsapp.net")
			d.IsDuplicate(jid, sender, "content", time.Now())
		}(i)

		go func(i int) {
			defer wg.Done()
			jid := domain.JID("group@g.us")
			sender := domain.JID("sender@s.whatsapp.net")
			d.Record(jid, sender, "content", time.Now())
		}(i)

		go func() {
			defer wg.Done()
			d.Stats()
		}()
	}

	wg.Wait()
}

// --- CircuitBreaker Edge Cases ---

func TestEdge_CircuitBreaker_ConcurrentAccess(t *testing.T) {
	cb := resilience.NewCircuitBreaker(3, time.Second)

	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(4)

		go func() {
			defer wg.Done()
			cb.Allow()
		}()

		go func() {
			defer wg.Done()
			cb.RecordSuccess()
		}()

		go func() {
			defer wg.Done()
			cb.RecordFailure()
		}()

		go func() {
			defer wg.Done()
			cb.State()
		}()
	}

	wg.Wait()
}

// --- RateLimiter Edge Cases ---

func TestEdge_RateLimiter_ConcurrentAccess(t *testing.T) {
	cfg := resilience.RateLimiterConfig{
		RatePerMinute: 1000,
		BurstSize:     100,
		Enabled:       true,
	}
	rl := resilience.NewRateLimiter(cfg)

	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(5)

		go func() {
			defer wg.Done()
			rl.Allow()
		}()

		go func() {
			defer wg.Done()
			ctx, cancel := context.WithTimeout(context.Background(), time.Millisecond)
			defer cancel()
			rl.Wait(ctx)
		}()

		go func() {
			defer wg.Done()
			rl.GetStats()
		}()

		go func() {
			defer wg.Done()
			rl.GetCurrentTokens()
		}()

		go func() {
			defer wg.Done()
			rl.SetEnabled(true)
		}()
	}

	wg.Wait()
}

func TestEdge_RateLimiter_ZeroConfig(t *testing.T) {
	// Zero values should not cause panic
	cfg := resilience.RateLimiterConfig{
		RatePerMinute: 0,
		BurstSize:     0,
		Enabled:       true,
	}
	rl := resilience.NewRateLimiter(cfg)

	// Should not panic
	rl.Allow()
	rl.GetCurrentTokens()
}

// --- Reconnector Edge Cases ---

func TestEdge_Reconnector_ConcurrentRun(t *testing.T) {
	cfg := reconnector.Config{
		InitialInterval:     time.Millisecond,
		MaxInterval:         10 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          2,
	}
	r := reconnector.New(cfg, zerolog.Nop())

	var wg sync.WaitGroup
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			r.Run(context.Background(), func(ctx context.Context) error {
				return nil
			})
		}()
	}

	wg.Wait()
}

func TestEdge_Reconnector_StopWhileRunning(t *testing.T) {
	cfg := reconnector.Config{
		InitialInterval:     100 * time.Millisecond,
		MaxInterval:         time.Second,
		Multiplier:          2,
		RandomizationFactor: 0,
		MaxRetries:          0, // Infinite
	}
	r := reconnector.New(cfg, zerolog.Nop())

	var wg sync.WaitGroup
	wg.Add(1)

	go func() {
		defer wg.Done()
		r.Run(context.Background(), func(ctx context.Context) error {
			return context.DeadlineExceeded // Always fail
		})
	}()

	time.Sleep(50 * time.Millisecond)
	r.Stop()

	wg.Wait()
}

// --- HistorySync Edge Cases ---

func TestEdge_HistorySync_ConcurrentAccess(t *testing.T) {
	cfg := historysync.Config{
		Cooldown:    time.Millisecond,
		MaxAge:      time.Hour,
		MaxMessages: 100,
		CacheSize:   100,
		CacheTTL:    time.Hour,
	}
	h := historysync.New(cfg, zerolog.Nop())

	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(4)

		go func() {
			defer wg.Done()
			h.ShouldProcess()
		}()

		go func(i int) {
			defer wg.Done()
			h.MarkMessageProcessed("msg-" + string(rune('a'+i%26)))
		}(i)

		go func(i int) {
			defer wg.Done()
			h.IsMessageProcessed("msg-" + string(rune('a'+i%26)))
		}(i)

		go func() {
			defer wg.Done()
			h.GetStats()
		}()
	}

	wg.Wait()
}

// --- Bridge Edge Cases ---

type mockSource struct {
	messages  chan domain.Message
	connected bool
}

func (m *mockSource) Connect(ctx context.Context) error { m.connected = true; return nil }
func (m *mockSource) Disconnect()                       { m.connected = false }
func (m *mockSource) IsConnected() bool                 { return m.connected }
func (m *mockSource) Messages() <-chan domain.Message   { return m.messages }

type mockGroupRepo struct{}

func (m *mockGroupRepo) GetMonitoredGroups(ctx context.Context) ([]domain.JID, error) {
	return []domain.JID{"group@g.us"}, nil
}

type mockGroupCache struct {
	monitored map[domain.JID]bool
}

func (m *mockGroupCache) IsMonitored(jid domain.JID) bool { return m.monitored[jid] }
func (m *mockGroupCache) Update(jids []domain.JID)        {}
func (m *mockGroupCache) IsStale() bool                   { return false }
func (m *mockGroupCache) LastSync() time.Time             { return time.Now() }

type mockDedup struct{}

func (m *mockDedup) IsDuplicate(groupJID, senderJID domain.JID, content string, timestamp time.Time) bool {
	return false
}
func (m *mockDedup) Record(groupJID, senderJID domain.JID, content string, timestamp time.Time) {}
func (m *mockDedup) Close()                                                                     {}

type mockRateLimiter struct{}

func (m *mockRateLimiter) Allow() bool                     { return true }
func (m *mockRateLimiter) Wait(ctx context.Context) error  { return nil }
func (m *mockRateLimiter) GetStats() map[string]int64      { return nil }
func (m *mockRateLimiter) SetEnabled(enabled bool)         {}
func (m *mockRateLimiter) IsEnabled() bool                 { return true }
func (m *mockRateLimiter) SetRate(rate float64, burst int) {}
func (m *mockRateLimiter) GetCurrentTokens() float64       { return 0 }
func (m *mockRateLimiter) Reset()                          {}

func TestEdge_Bridge_DoubleStop(t *testing.T) {
	source := &mockSource{messages: make(chan domain.Message)}
	sink := &mockSink{}

	b := app.NewBridge(app.BridgeParams{
		Source:      source,
		Sink:        sink,
		GroupCache:  &mockGroupCache{monitored: map[domain.JID]bool{"group@g.us": true}},
		GroupRepo:   &mockGroupRepo{},
		Dedup:       &mockDedup{},
		RateLimiter: &mockRateLimiter{},
		Logger:      zerolog.Nop(),
		Config: app.BridgeConfig{
			WorkerCount:     2,
			WorkerQueueSize: 10,
		},
	})

	// First stop
	b.Stop()

	// Second stop should not panic
	b.Stop()
}

func TestEdge_Bridge_StopWhileRunning(t *testing.T) {
	source := &mockSource{messages: make(chan domain.Message, 10)}
	sink := &mockSink{}

	b := app.NewBridge(app.BridgeParams{
		Source:      source,
		Sink:        sink,
		GroupCache:  &mockGroupCache{monitored: map[domain.JID]bool{"group@g.us": true}},
		GroupRepo:   &mockGroupRepo{},
		Dedup:       &mockDedup{},
		RateLimiter: &mockRateLimiter{},
		Logger:      zerolog.Nop(),
		Config: app.BridgeConfig{
			WorkerCount:     2,
			WorkerQueueSize: 10,
		},
	})

	ctx, cancel := context.WithCancel(context.Background())

	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		b.Run(ctx)
	}()

	// Send some messages
	for i := 0; i < 5; i++ {
		source.messages <- domain.Message{
			ID:       domain.MessageID("test"),
			IsGroup:  true,
			GroupJID: domain.JID("group@g.us"),
			Content:  "test",
		}
	}

	time.Sleep(10 * time.Millisecond)
	cancel()
	b.Stop()

	wg.Wait()
}

// Ensure mocks implement interfaces
var _ ports.MessageSource = (*mockSource)(nil)
var _ ports.MessageSink = (*mockSink)(nil)
var _ ports.GroupRepository = (*mockGroupRepo)(nil)
var _ ports.GroupCache = (*mockGroupCache)(nil)
var _ ports.Deduplicator = (*mockDedup)(nil)
var _ ports.RateLimiter = (*mockRateLimiter)(nil)
