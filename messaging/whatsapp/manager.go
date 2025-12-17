package whatsapp

import (
	"context"
	"fmt"
	"os"
	"slices"
	"sync"
	"sync/atomic"
	"time"

	"github.com/google/uuid"
	_ "github.com/lib/pq" // PostgreSQL driver for sqlstore
	"github.com/mdp/qrterminal/v3"
	"github.com/rs/zerolog"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/proto/waE2E"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"
	"google.golang.org/protobuf/proto"

	"pharmabroker/bot/core"
	whatsappbot "pharmabroker/bot/whatsapp"
	"pharmabroker/messaging/reconnector"
	"pharmabroker/pkg/config"
	"pharmabroker/pkg/metrics"
)

// Constants for timeouts and limits
const (
	defaultConnectTimeout = 30 * time.Second
	groupInfoTimeout      = 5 * time.Second
	botResponseTimeout    = 30 * time.Second
	qrChannelBufferSize   = 1

	// History sync deduplication constants
	historySyncCooldown    = 5 * time.Minute // Minimum time between processing history syncs
	historySyncMaxAge      = 24 * time.Hour  // Only process messages newer than this
	historySyncMaxMessages = 1000            // Maximum messages to process per sync
	processedIDsCacheSize  = 10000           // Size of processed message IDs cache
	processedIDsCacheTTL   = 1 * time.Hour   // TTL for processed IDs cache entries

	// Group info cache constants
	groupInfoCacheSize = 500              // Maximum cached group names
	groupInfoCacheTTL  = 30 * time.Minute // TTL for cached group info

	// Message size limits
	maxMessageContentSize  = 10000 // Maximum message content size in bytes (10KB)
	truncatedMessageSuffix = "... [truncated]"

	// Ordered queue constants
	orderedQueueBufferSize = 100 // Buffer size per group queue

	// Outbound rate limiter constants
	defaultOutboundRatePerMinute = 20               // Default: 20 messages per minute
	defaultOutboundBurstSize     = 5                // Allow burst of 5 messages
	rateLimitWaitTimeout         = 30 * time.Second // Max wait time for rate limit
)

// =============================================================================
// Outbound Rate Limiter
// =============================================================================

// OutboundRateLimiter controls the rate of outgoing messages to prevent WhatsApp bans.
// Uses a token bucket algorithm with configurable rate and burst size.
type OutboundRateLimiter struct {
	// Configuration
	ratePerMinute float64
	burstSize     int
	enabled       atomic.Bool

	// Token bucket state
	tokens     float64
	lastRefill time.Time
	mu         sync.Mutex

	// Statistics
	stats OutboundRateLimiterStats

	// Logging
	log zerolog.Logger
}

// OutboundRateLimiterStats tracks rate limiter statistics.
type OutboundRateLimiterStats struct {
	TotalRequests   atomic.Int64 // Total send requests
	TotalAllowed    atomic.Int64 // Requests allowed immediately
	TotalWaited     atomic.Int64 // Requests that had to wait
	TotalDropped    atomic.Int64 // Requests dropped due to timeout
	TotalWaitTimeMs atomic.Int64 // Cumulative wait time in milliseconds
}

// OutboundRateLimiterConfig holds configuration for the rate limiter.
type OutboundRateLimiterConfig struct {
	RatePerMinute float64 // Messages allowed per minute
	BurstSize     int     // Maximum burst size
	Enabled       bool    // Whether rate limiting is enabled
}

// DefaultOutboundRateLimiterConfig returns sensible defaults.
func DefaultOutboundRateLimiterConfig() OutboundRateLimiterConfig {
	return OutboundRateLimiterConfig{
		RatePerMinute: defaultOutboundRatePerMinute,
		BurstSize:     defaultOutboundBurstSize,
		Enabled:       true,
	}
}

// NewOutboundRateLimiter creates a new rate limiter with the given configuration.
func NewOutboundRateLimiter(cfg OutboundRateLimiterConfig, log zerolog.Logger) *OutboundRateLimiter {
	if cfg.RatePerMinute <= 0 {
		cfg.RatePerMinute = defaultOutboundRatePerMinute
	}
	if cfg.BurstSize <= 0 {
		cfg.BurstSize = defaultOutboundBurstSize
	}

	rl := &OutboundRateLimiter{
		ratePerMinute: cfg.RatePerMinute,
		burstSize:     cfg.BurstSize,
		tokens:        float64(cfg.BurstSize), // Start with full bucket
		lastRefill:    time.Now(),
		log:           log.With().Str("component", "rate-limiter").Logger(),
	}
	rl.enabled.Store(cfg.Enabled)

	rl.log.Info().
		Float64("rate_per_minute", cfg.RatePerMinute).
		Int("burst_size", cfg.BurstSize).
		Bool("enabled", cfg.Enabled).
		Msg("Outbound rate limiter initialized")

	return rl
}

// Wait blocks until a token is available or the context is cancelled.
// Returns nil if a token was acquired, or an error if the wait was cancelled/timed out.
func (rl *OutboundRateLimiter) Wait(ctx context.Context) error {
	rl.stats.TotalRequests.Add(1)

	// If disabled, allow immediately
	if !rl.enabled.Load() {
		rl.stats.TotalAllowed.Add(1)
		return nil
	}

	startWait := time.Now()

	for {
		// Try to acquire a token
		if rl.tryAcquire() {
			waitTime := time.Since(startWait)
			if waitTime > time.Millisecond {
				rl.stats.TotalWaited.Add(1)
				rl.stats.TotalWaitTimeMs.Add(waitTime.Milliseconds())
				rl.log.Debug().
					Dur("wait_time", waitTime).
					Msg("Rate limit wait completed")
			} else {
				rl.stats.TotalAllowed.Add(1)
			}
			return nil
		}

		// Calculate time until next token
		waitDuration := rl.timeUntilNextToken()

		// Check context before waiting
		select {
		case <-ctx.Done():
			rl.stats.TotalDropped.Add(1)
			rl.log.Warn().
				Dur("waited", time.Since(startWait)).
				Msg("Rate limit wait cancelled")
			return ctx.Err()
		case <-time.After(waitDuration):
			// Continue loop to try acquiring again
		}
	}
}

// Allow checks if a message can be sent immediately without waiting.
// Returns true if allowed, false if rate limited.
func (rl *OutboundRateLimiter) Allow() bool {
	rl.stats.TotalRequests.Add(1)

	if !rl.enabled.Load() {
		rl.stats.TotalAllowed.Add(1)
		return true
	}

	if rl.tryAcquire() {
		rl.stats.TotalAllowed.Add(1)
		return true
	}

	rl.stats.TotalDropped.Add(1)
	return false
}

// tryAcquire attempts to acquire a token from the bucket.
// Returns true if successful, false if no tokens available.
func (rl *OutboundRateLimiter) tryAcquire() bool {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	rl.refillTokens()

	if rl.tokens >= 1.0 {
		rl.tokens -= 1.0
		return true
	}

	return false
}

// refillTokens adds tokens based on elapsed time since last refill.
// Must be called with mutex held.
func (rl *OutboundRateLimiter) refillTokens() {
	now := time.Now()
	elapsed := now.Sub(rl.lastRefill)
	rl.lastRefill = now

	// Calculate tokens to add: (elapsed_minutes * rate_per_minute)
	tokensToAdd := elapsed.Minutes() * rl.ratePerMinute

	// Add tokens, capped at burst size
	rl.tokens = min(rl.tokens+tokensToAdd, float64(rl.burstSize))
}

// timeUntilNextToken calculates how long until the next token is available.
func (rl *OutboundRateLimiter) timeUntilNextToken() time.Duration {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	if rl.tokens >= 1.0 {
		return 0
	}

	// Time for one token: 60 seconds / rate_per_minute
	tokenInterval := time.Duration(60.0/rl.ratePerMinute*1000) * time.Millisecond
	tokensNeeded := 1.0 - rl.tokens

	return time.Duration(float64(tokenInterval) * tokensNeeded)
}

// SetEnabled enables or disables the rate limiter.
func (rl *OutboundRateLimiter) SetEnabled(enabled bool) {
	rl.enabled.Store(enabled)
	rl.log.Info().Bool("enabled", enabled).Msg("Rate limiter state changed")
}

// IsEnabled returns whether the rate limiter is enabled.
func (rl *OutboundRateLimiter) IsEnabled() bool {
	return rl.enabled.Load()
}

// SetRate updates the rate limit configuration.
func (rl *OutboundRateLimiter) SetRate(ratePerMinute float64, burstSize int) {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	if ratePerMinute > 0 {
		rl.ratePerMinute = ratePerMinute
	}
	if burstSize > 0 {
		rl.burstSize = burstSize
		// Cap current tokens at new burst size
		rl.tokens = min(rl.tokens, float64(burstSize))
	}

	rl.log.Info().
		Float64("rate_per_minute", rl.ratePerMinute).
		Int("burst_size", rl.burstSize).
		Msg("Rate limiter configuration updated")
}

// GetStats returns a snapshot of rate limiter statistics.
func (rl *OutboundRateLimiter) GetStats() map[string]int64 {
	return map[string]int64{
		"total_requests":     rl.stats.TotalRequests.Load(),
		"total_allowed":      rl.stats.TotalAllowed.Load(),
		"total_waited":       rl.stats.TotalWaited.Load(),
		"total_dropped":      rl.stats.TotalDropped.Load(),
		"total_wait_time_ms": rl.stats.TotalWaitTimeMs.Load(),
	}
}

// GetCurrentTokens returns the current number of available tokens.
func (rl *OutboundRateLimiter) GetCurrentTokens() float64 {
	rl.mu.Lock()
	defer rl.mu.Unlock()
	rl.refillTokens()
	return rl.tokens
}

// Reset resets the rate limiter to its initial state.
func (rl *OutboundRateLimiter) Reset() {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	rl.tokens = float64(rl.burstSize)
	rl.lastRefill = time.Now()

	rl.log.Info().Msg("Rate limiter reset")
}

// GroupInfoCache caches group names to reduce API calls.
type GroupInfoCache struct {
	entries map[string]*groupInfoEntry
	mu      sync.RWMutex
}

type groupInfoEntry struct {
	name      string
	fetchedAt time.Time
}

// NewGroupInfoCache creates a new group info cache.
func NewGroupInfoCache() *GroupInfoCache {
	return &GroupInfoCache{
		entries: make(map[string]*groupInfoEntry, groupInfoCacheSize),
	}
}

// Get retrieves a cached group name, returns empty string if not found or expired.
func (c *GroupInfoCache) Get(jid string) (string, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	entry, exists := c.entries[jid]
	if !exists {
		return "", false
	}

	// Check TTL
	if time.Since(entry.fetchedAt) > groupInfoCacheTTL {
		return "", false
	}

	return entry.name, true
}

// Set stores a group name in the cache.
func (c *GroupInfoCache) Set(jid, name string) {
	c.mu.Lock()
	defer c.mu.Unlock()

	// Evict oldest entries if at capacity
	if len(c.entries) >= groupInfoCacheSize {
		c.evictOldest()
	}

	c.entries[jid] = &groupInfoEntry{
		name:      name,
		fetchedAt: time.Now(),
	}
}

// evictOldest removes the oldest entry from the cache (must be called with lock held).
func (c *GroupInfoCache) evictOldest() {
	var oldestJID string
	var oldestTime time.Time

	for jid, entry := range c.entries {
		if oldestJID == "" || entry.fetchedAt.Before(oldestTime) {
			oldestJID = jid
			oldestTime = entry.fetchedAt
		}
	}

	if oldestJID != "" {
		delete(c.entries, oldestJID)
	}
}

// Size returns the number of cached entries.
func (c *GroupInfoCache) Size() int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.entries)
}

// Clear removes all entries from the cache.
func (c *GroupInfoCache) Clear() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.entries = make(map[string]*groupInfoEntry, groupInfoCacheSize)
}

// OrderedMessageQueue provides per-group ordered message processing.
type OrderedMessageQueue struct {
	queues   map[string]chan *IncomingMessage
	handlers []EventHandler
	mu       sync.RWMutex
	wg       sync.WaitGroup
	done     chan struct{}
	log      zerolog.Logger
}

// NewOrderedMessageQueue creates a new ordered message queue.
func NewOrderedMessageQueue(log zerolog.Logger) *OrderedMessageQueue {
	return &OrderedMessageQueue{
		queues: make(map[string]chan *IncomingMessage),
		done:   make(chan struct{}),
		log:    log.With().Str("component", "ordered-queue").Logger(),
	}
}

// SetHandlers sets the event handlers for message processing.
func (q *OrderedMessageQueue) SetHandlers(handlers []EventHandler) {
	q.mu.Lock()
	defer q.mu.Unlock()
	q.handlers = handlers
}

// Enqueue adds a message to the appropriate group queue for ordered processing.
func (q *OrderedMessageQueue) Enqueue(msg *IncomingMessage) {
	q.mu.Lock()
	queue, exists := q.queues[msg.GroupJID]
	if !exists {
		queue = make(chan *IncomingMessage, orderedQueueBufferSize)
		q.queues[msg.GroupJID] = queue
		q.wg.Add(1)
		go q.processGroup(msg.GroupJID, queue)
	}
	q.mu.Unlock()

	select {
	case queue <- msg:
		// Message queued
	default:
		q.log.Warn().
			Str("group_jid", msg.GroupJID).
			Str("msg_id", msg.ID).
			Msg("Ordered queue full, message dropped")
	}
}

// processGroup processes messages for a single group in order.
func (q *OrderedMessageQueue) processGroup(_ string, queue chan *IncomingMessage) {
	defer q.wg.Done()

	for {
		select {
		case <-q.done:
			return
		case msg, ok := <-queue:
			if !ok {
				return
			}
			q.deliverToHandlers(msg)
		}
	}
}

// deliverToHandlers sends the message to all handlers with panic recovery.
func (q *OrderedMessageQueue) deliverToHandlers(msg *IncomingMessage) {
	q.mu.RLock()
	handlers := q.handlers
	q.mu.RUnlock()

	for _, h := range handlers {
		func(handler EventHandler) {
			defer func() {
				if r := recover(); r != nil {
					q.log.Error().
						Interface("panic", r).
						Str("message_id", msg.ID).
						Str("group", msg.GroupName).
						Msg("Handler panic recovered in ordered queue")
				}
			}()
			handler.HandleMessage(msg)
		}(h)
	}
}

// Stop gracefully shuts down the ordered queue.
func (q *OrderedMessageQueue) Stop() {
	close(q.done)
	q.wg.Wait()

	q.mu.Lock()
	for _, queue := range q.queues {
		close(queue)
	}
	q.queues = make(map[string]chan *IncomingMessage)
	q.mu.Unlock()
}

// Stats returns queue statistics.
func (q *OrderedMessageQueue) Stats() OrderedQueueStats {
	q.mu.RLock()
	defer q.mu.RUnlock()

	stats := OrderedQueueStats{
		ActiveGroups: len(q.queues),
		QueueSizes:   make(map[string]int),
	}

	for jid, queue := range q.queues {
		stats.QueueSizes[jid] = len(queue)
		stats.TotalPending += len(queue)
	}

	return stats
}

// OrderedQueueStats contains statistics about the ordered queue.
type OrderedQueueStats struct {
	ActiveGroups int            `json:"active_groups"`
	TotalPending int            `json:"total_pending"`
	QueueSizes   map[string]int `json:"queue_sizes,omitempty"`
}

// ConnectionState represents the current state of the WhatsApp connection.
type ConnectionState int32

const (
	StateDisconnected ConnectionState = iota
	StateConnecting
	StateConnected
	StateReconnecting
	StateFailed // Max attempts reached
)

// String returns the string representation of the connection state.
func (s ConnectionState) String() string {
	switch s {
	case StateDisconnected:
		return "DISCONNECTED"
	case StateConnecting:
		return "CONNECTING"
	case StateConnected:
		return "CONNECTED"
	case StateReconnecting:
		return "RECONNECTING"
	case StateFailed:
		return "FAILED"
	default:
		return "UNKNOWN"
	}
}

// AlertNotifier sends alerts to administrators.
type AlertNotifier interface {
	SendAlert(ctx context.Context, severity, title, message string) error
}

// Manager manages WhatsApp client connections with resilient reconnection.
type Manager struct {
	cfg      *config.WhatsAppConfig
	client   *whatsmeow.Client
	store    *sqlstore.Container
	log      zerolog.Logger
	mu       sync.RWMutex
	handlers []EventHandler

	// Bot command handler (optional)
	botHandler *whatsappbot.Bot

	// Admin alerter (optional)
	alerter AlertNotifier

	// Reconnection (uses standalone reconnector package)
	reconnector *reconnector.Reconnector

	// State (atomic for lock-free reads)
	state           atomic.Int32 // ConnectionState
	reconnectCount  atomic.Int32
	lastConnectedAt atomic.Int64 // Unix timestamp

	// History sync deduplication
	lastHistorySyncAt atomic.Int64     // Unix timestamp of last history sync
	processedMsgIDs   map[string]int64 // Message ID -> timestamp (for dedup)
	processedMsgIDsMu sync.RWMutex     // Mutex for processedMsgIDs
	historySyncStats  HistorySyncStats // Statistics for monitoring

	// Group info cache (reduces API calls)
	groupInfoCache *GroupInfoCache

	// Ordered message queue (optional, for strict ordering)
	orderedQueue    *OrderedMessageQueue
	useOrderedQueue bool

	// Message processing stats
	messageStats MessageProcessingStats

	// Outbound rate limiter (prevents WhatsApp bans)
	outboundRateLimiter *OutboundRateLimiter

	// Channels
	qrChannel     chan string
	stopChan      chan struct{}
	reconnectChan chan struct{} // Signal to trigger reconnect
}

// MessageProcessingStats tracks message processing statistics.
type MessageProcessingStats struct {
	TotalReceived  atomic.Int64 // Total messages received
	TotalProcessed atomic.Int64 // Messages successfully processed
	TotalTruncated atomic.Int64 // Messages truncated due to size
	TotalDropped   atomic.Int64 // Messages dropped (queue full, etc.)
}

// HistorySyncStats tracks history sync processing statistics.
type HistorySyncStats struct {
	TotalSyncs        atomic.Int64 // Total history sync events received
	SkippedCooldown   atomic.Int64 // Syncs skipped due to cooldown
	MessagesReceived  atomic.Int64 // Total messages in sync events
	MessagesSkipped   atomic.Int64 // Messages skipped (old, duplicate, etc.)
	MessagesProcessed atomic.Int64 // Messages actually processed
}

// EventHandler processes WhatsApp events
type EventHandler interface {
	HandleMessage(msg *IncomingMessage)
	HandleGroupJoined(group *GroupInfo)
}

// IncomingMessage represents a received WhatsApp message
type IncomingMessage struct {
	ID string // External WhatsApp ID

	GroupJID    string
	GroupName   string
	SenderJID   string
	SenderPhone string
	SenderName  string
	Content     string
	Timestamp   time.Time
	IsFromMe    bool

	// Reply context (for messages that are replies to other messages)
	ReplyToID      string // WhatsApp ID of the quoted message
	ReplyToContent string // Text content of the quoted message
	ReplyToSender  string // JID of the sender of the quoted message
}

// GroupInfo represents WhatsApp group information
type GroupInfo struct {
	JID         string
	Name        string
	Description string
}

// NewManager creates a new WhatsApp manager with default reconnection config.
func NewManager(ctx context.Context, cfg *config.WhatsAppConfig, log zerolog.Logger) (*Manager, error) {
	return NewManagerWithConfig(ctx, cfg, reconnector.DefaultReconnectorConfig(), log)
}

// NewManagerWithConfig creates a new WhatsApp manager with custom reconnection config.
func NewManagerWithConfig(ctx context.Context, cfg *config.WhatsAppConfig, reconnectorCfg reconnector.ReconnectorConfig, log zerolog.Logger) (*Manager, error) {
	// Ensure session directory exists (for temporary files even when using PostgreSQL)
	if err := os.MkdirAll(cfg.SessionDir, 0755); err != nil {
		return nil, fmt.Errorf("create session directory: %w", err)
	}

	// Initialize session store (PostgreSQL only - SQLite removed)
	if cfg.SessionDBDSN == "" {
		return nil, fmt.Errorf("SessionDBDSN is required: PostgreSQL is the only supported session store")
	}

	container, err := sqlstore.New(ctx, "postgres", cfg.SessionDBDSN, waLog.Noop)
	if err != nil {
		return nil, fmt.Errorf("create PostgreSQL session store: %w", err)
	}
	log.Info().Msg("WhatsApp session store: PostgreSQL")

	m := &Manager{
		cfg:             cfg,
		store:           container,
		log:             log.With().Str("component", "whatsapp").Logger(),
		qrChannel:       make(chan string, qrChannelBufferSize),
		stopChan:        make(chan struct{}),
		reconnectChan:   make(chan struct{}, 1),
		processedMsgIDs: make(map[string]int64, processedIDsCacheSize),
		groupInfoCache:  NewGroupInfoCache(),
		outboundRateLimiter: NewOutboundRateLimiter(
			DefaultOutboundRateLimiterConfig(),
			log,
		),
	}

	// Initialize reconnector with callbacks
	m.reconnector = reconnector.NewReconnector(reconnectorCfg, log)
	m.setupReconnectorCallbacks()

	m.setState(StateDisconnected)
	return m, nil
}

// setupReconnectorCallbacks configures reconnector callbacks for state management.
func (m *Manager) setupReconnectorCallbacks() {
	m.reconnector.SetOnRetry(func(attempt int, delay time.Duration, err error) {
		m.reconnectCount.Store(int32(attempt))
		m.log.Info().
			Int("attempt", attempt).
			Dur("next_delay", delay).
			Err(err).
			Msg("Reconnection attempt scheduled")
	})

	m.reconnector.SetOnSuccess(func(attempt int, elapsed time.Duration) {
		m.log.Info().
			Int("attempts", attempt).
			Dur("elapsed", elapsed).
			Msg("Reconnection successful")
	})

	m.reconnector.SetOnFailure(func(attempt int, elapsed time.Duration, err error) {
		m.onMaxAttemptsReached(attempt)
	})
}

// RegisterHandler adds an event handler
func (m *Manager) RegisterHandler(h EventHandler) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.handlers = append(m.handlers, h)
}

// Connect establishes WhatsApp connection
func (m *Manager) Connect(ctx context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Get or create device
	device, err := m.store.GetFirstDevice(ctx)
	if err != nil {
		return fmt.Errorf("get device: %w", err)
	}

	if device == nil {
		device = m.store.NewDevice()
	}

	// Create client
	m.client = whatsmeow.NewClient(device, waLog.Noop)
	m.client.AddEventHandler(m.handleEvent)

	// Connect
	if m.client.Store.ID == nil {
		// Need to pair - get QR code
		qrChan, _ := m.client.GetQRChannel(ctx)
		if err := m.client.Connect(); err != nil {
			return fmt.Errorf("connect: %w", err)
		}

		// Wait for QR or success
		for evt := range qrChan {
			switch evt.Event {
			case "code":
				m.log.Info().Msg("QR code received, waiting for scan...")
				qrterminal.GenerateHalfBlock(evt.Code, qrterminal.L, os.Stdout)
				select {
				case m.qrChannel <- evt.Code:
				default:
				}
			case "success":
				m.log.Info().Msg("Successfully paired!")
				m.onConnected()
				return nil
			case "timeout":
				return fmt.Errorf("QR code timeout")
			}
		}
	} else {
		// Already paired, just connect
		if err := m.client.Connect(); err != nil {
			return fmt.Errorf("connect: %w", err)
		}
		m.onConnected()
		m.log.Info().Msg("Connected to existing session")
	}

	return nil
}

// GetQRChannel returns channel that receives QR codes for pairing
func (m *Manager) GetQRChannel() <-chan string {
	return m.qrChannel
}

// IsConnected returns connection status.
func (m *Manager) IsConnected() bool {
	return m.State() == StateConnected
}

// State returns the current connection state.
func (m *Manager) State() ConnectionState {
	return ConnectionState(m.state.Load())
}

// setState updates the state and triggers callback if configured.
func (m *Manager) setState(newState ConnectionState) {
	oldState := ConnectionState(m.state.Swap(int32(newState)))
	if oldState != newState {
		m.log.Info().
			Str("from", oldState.String()).
			Str("to", newState.String()).
			Msg("Connection state changed")

		metrics.WhatsAppConnectionState.Set(float64(newState))
	}
}

// onConnected handles successful connection.
func (m *Manager) onConnected() {
	m.setState(StateConnected)
	m.lastConnectedAt.Store(time.Now().Unix())
	m.reconnectCount.Store(0)
	metrics.WhatsAppReconnectAttempts.Add(0) // Initialize if needed
}

// Disconnect closes the WhatsApp connection.
func (m *Manager) Disconnect() {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.client != nil {
		m.client.Disconnect()
		m.setState(StateDisconnected)
	}

	close(m.stopChan)
}

// GetJoinedGroups returns all groups the client is a member of
func (m *Manager) GetJoinedGroups(ctx context.Context) ([]*GroupInfo, error) {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		return nil, fmt.Errorf("not connected")
	}

	groups, err := client.GetJoinedGroups(ctx)
	if err != nil {
		return nil, fmt.Errorf("get groups: %w", err)
	}

	var result []*GroupInfo
	for _, g := range groups {
		result = append(result, &GroupInfo{
			JID:         g.JID.String(),
			Name:        g.Name,
			Description: g.Topic,
		})
	}

	return result, nil
}

// SyncGroups fetches all joined groups and saves them using the provided save function
func (m *Manager) SyncGroups(ctx context.Context, saveFunc func(jid, name, description string) error) error {
	groups, err := m.GetJoinedGroups(ctx)
	if err != nil {
		return fmt.Errorf("get joined groups: %w", err)
	}

	m.log.Info().Int("count", len(groups)).Msg("Syncing groups to database")

	for _, g := range groups {
		if err := saveFunc(g.JID, g.Name, g.Description); err != nil {
			m.log.Warn().Err(err).Str("jid", g.JID).Msg("Failed to save group")
		}
	}

	return nil
}

// handleEvent processes WhatsApp events
func (m *Manager) handleEvent(evt any) {
	switch v := evt.(type) {
	case *events.Message:
		m.handleMessageEvent(v)
	case *events.Connected:
		m.onConnected()
		m.log.Info().Msg("WhatsApp connected")
	case *events.Disconnected:
		if m.State() != StateReconnecting {
			m.setState(StateReconnecting)
			m.log.Warn().Msg("WhatsApp disconnected, starting reconnection")
			go m.reconnectWithBackoff()
		}
	case *events.HistorySync:
		m.handleHistorySync(v)
	}
}

// handleHistorySync processes history sync events with deduplication.
// It prevents duplicate processing by:
// 1. Enforcing a cooldown period between syncs
// 2. Filtering out old messages beyond the max age
// 3. Tracking processed message IDs to prevent duplicates
// 4. Limiting the number of messages processed per sync
func (m *Manager) handleHistorySync(v *events.HistorySync) {
	m.historySyncStats.TotalSyncs.Add(1)

	// Check cooldown - skip if we processed a sync recently
	lastSync := m.lastHistorySyncAt.Load()
	now := time.Now().Unix()
	if lastSync > 0 && now-lastSync < int64(historySyncCooldown.Seconds()) {
		m.historySyncStats.SkippedCooldown.Add(1)
		m.log.Info().
			Int64("last_sync_ago_seconds", now-lastSync).
			Int64("cooldown_seconds", int64(historySyncCooldown.Seconds())).
			Msg("Skipping history sync - cooldown active")
		return
	}

	// Update last sync timestamp
	m.lastHistorySyncAt.Store(now)

	// Calculate cutoff time for old messages
	cutoffTime := time.Now().Add(-historySyncMaxAge)

	// Count messages for logging
	totalMessages := 0
	for _, conv := range v.Data.Conversations {
		totalMessages += len(conv.Messages)
	}
	m.historySyncStats.MessagesReceived.Add(int64(totalMessages))

	m.log.Info().
		Str("type", fmt.Sprintf("%v", v.Data.SyncType)).
		Int("conversations", len(v.Data.Conversations)).
		Int("total_messages", totalMessages).
		Time("cutoff_time", cutoffTime).
		Msg("Processing History Sync with deduplication")

	// Clean up old entries from processed IDs cache
	m.cleanupProcessedIDsCache()

	processedCount := 0
	skippedOld := 0
	skippedDuplicate := 0

	for _, conv := range v.Data.Conversations {
		for _, waMsg := range conv.Messages {
			// Check message limit
			if processedCount >= historySyncMaxMessages {
				m.log.Warn().
					Int("limit", historySyncMaxMessages).
					Msg("History sync message limit reached")
				goto done
			}

			if waMsg.Message == nil || waMsg.Message.Key == nil {
				continue
			}

			key := waMsg.Message.Key
			msgID := key.GetID()

			// Get timestamp
			ts := int64(0)
			if waMsg.Message.MessageTimestamp != nil {
				ts = int64(*waMsg.Message.MessageTimestamp)
			}
			msgTime := time.Unix(ts, 0)

			// Skip old messages
			if msgTime.Before(cutoffTime) {
				skippedOld++
				continue
			}

			// Skip already processed messages
			if m.isMessageProcessed(msgID) {
				skippedDuplicate++
				continue
			}

			// Mark as processed
			m.markMessageProcessed(msgID)

			pushName := ""
			if waMsg.Message.PushName != nil {
				pushName = *waMsg.Message.PushName
			}

			info := types.MessageInfo{
				ID:        msgID,
				Timestamp: msgTime,
				PushName:  pushName,
			}
			info.IsFromMe = key.GetFromMe()

			// Parse chat JID
			if key.RemoteJID != nil {
				if chatJID, err := types.ParseJID(*key.RemoteJID); err == nil {
					info.Chat = chatJID
				}
			}

			// Parse sender JID
			if key.Participant != nil {
				if senderJID, err := types.ParseJID(*key.Participant); err == nil {
					info.Sender = senderJID
				}
			} else if !info.IsFromMe {
				info.Sender = info.Chat
			}

			// Mark as group if chat server is g.us
			if info.Chat.Server == "g.us" {
				info.IsGroup = true
			}

			// Only process group messages
			if info.IsGroup {
				msgEvt := &events.Message{
					Info:    info,
					Message: waMsg.Message.Message,
				}
				m.handleMessageEvent(msgEvt)
				processedCount++
			}
		}
	}

done:
	m.historySyncStats.MessagesSkipped.Add(int64(skippedOld + skippedDuplicate))
	m.historySyncStats.MessagesProcessed.Add(int64(processedCount))

	m.log.Info().
		Int("processed", processedCount).
		Int("skipped_old", skippedOld).
		Int("skipped_duplicate", skippedDuplicate).
		Int("total", totalMessages).
		Msg("History sync completed")
}

// isMessageProcessed checks if a message ID has already been processed.
func (m *Manager) isMessageProcessed(msgID string) bool {
	m.processedMsgIDsMu.RLock()
	defer m.processedMsgIDsMu.RUnlock()
	_, exists := m.processedMsgIDs[msgID]
	return exists
}

// markMessageProcessed marks a message ID as processed.
func (m *Manager) markMessageProcessed(msgID string) {
	m.processedMsgIDsMu.Lock()
	defer m.processedMsgIDsMu.Unlock()
	m.processedMsgIDs[msgID] = time.Now().Unix()
}

// cleanupProcessedIDsCache removes old entries from the processed IDs cache.
func (m *Manager) cleanupProcessedIDsCache() {
	m.processedMsgIDsMu.Lock()
	defer m.processedMsgIDsMu.Unlock()

	cutoff := time.Now().Add(-processedIDsCacheTTL).Unix()
	deleted := 0

	for id, ts := range m.processedMsgIDs {
		if ts < cutoff {
			delete(m.processedMsgIDs, id)
			deleted++
		}
	}

	// If still over capacity, remove oldest entries
	if len(m.processedMsgIDs) > processedIDsCacheSize {
		// Find and remove oldest entries until under capacity
		for id := range m.processedMsgIDs {
			if len(m.processedMsgIDs) <= processedIDsCacheSize {
				break
			}
			delete(m.processedMsgIDs, id)
			deleted++
		}
	}

	if deleted > 0 {
		m.log.Debug().
			Int("deleted", deleted).
			Int("remaining", len(m.processedMsgIDs)).
			Msg("Cleaned up processed message IDs cache")
	}
}

// GetHistorySyncStats returns the current history sync statistics.
func (m *Manager) GetHistorySyncStats() HistorySyncStats {
	return HistorySyncStats{
		TotalSyncs:        atomic.Int64{},
		SkippedCooldown:   atomic.Int64{},
		MessagesReceived:  atomic.Int64{},
		MessagesSkipped:   atomic.Int64{},
		MessagesProcessed: atomic.Int64{},
	}
}

// ResetHistorySyncState resets the history sync deduplication state.
// Useful for testing or manual intervention.
func (m *Manager) ResetHistorySyncState() {
	m.lastHistorySyncAt.Store(0)
	m.processedMsgIDsMu.Lock()
	m.processedMsgIDs = make(map[string]int64, processedIDsCacheSize)
	m.processedMsgIDsMu.Unlock()
	m.log.Info().Msg("History sync state reset")
}

// EnableOrderedQueue enables per-group ordered message processing.
// When enabled, messages from the same group are processed sequentially.
func (m *Manager) EnableOrderedQueue() {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.orderedQueue == nil {
		m.orderedQueue = NewOrderedMessageQueue(m.log)
	}
	m.orderedQueue.SetHandlers(m.handlers)
	m.useOrderedQueue = true
	m.log.Info().Msg("Ordered message queue enabled")
}

// DisableOrderedQueue disables ordered message processing.
func (m *Manager) DisableOrderedQueue() {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.orderedQueue != nil {
		m.orderedQueue.Stop()
	}
	m.useOrderedQueue = false
	m.log.Info().Msg("Ordered message queue disabled")
}

// IsOrderedQueueEnabled returns whether ordered queue is enabled.
func (m *Manager) IsOrderedQueueEnabled() bool {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.useOrderedQueue
}

// GetOrderedQueueStats returns statistics about the ordered queue.
func (m *Manager) GetOrderedQueueStats() *OrderedQueueStats {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if m.orderedQueue == nil {
		return nil
	}
	stats := m.orderedQueue.Stats()
	return &stats
}

// GetMessageStats returns message processing statistics.
func (m *Manager) GetMessageStats() MessageProcessingStats {
	return MessageProcessingStats{
		TotalReceived:  atomic.Int64{},
		TotalProcessed: atomic.Int64{},
		TotalTruncated: atomic.Int64{},
		TotalDropped:   atomic.Int64{},
	}
}

// GetMessageStatsSnapshot returns a snapshot of message processing statistics.
func (m *Manager) GetMessageStatsSnapshot() map[string]int64 {
	return map[string]int64{
		"total_received":  m.messageStats.TotalReceived.Load(),
		"total_processed": m.messageStats.TotalProcessed.Load(),
		"total_truncated": m.messageStats.TotalTruncated.Load(),
		"total_dropped":   m.messageStats.TotalDropped.Load(),
	}
}

// GetGroupInfoCacheStats returns statistics about the group info cache.
func (m *Manager) GetGroupInfoCacheStats() map[string]int {
	if m.groupInfoCache == nil {
		return nil
	}
	return map[string]int{
		"size":     m.groupInfoCache.Size(),
		"capacity": groupInfoCacheSize,
	}
}

// ClearGroupInfoCache clears the group info cache.
func (m *Manager) ClearGroupInfoCache() {
	if m.groupInfoCache != nil {
		m.groupInfoCache.Clear()
		m.log.Info().Msg("Group info cache cleared")
	}
}

// =============================================================================
// Rate Limiter Management
// =============================================================================

// GetOutboundRateLimiter returns the outbound rate limiter for direct access.
func (m *Manager) GetOutboundRateLimiter() *OutboundRateLimiter {
	return m.outboundRateLimiter
}

// SetOutboundRateLimit updates the outbound rate limit configuration.
func (m *Manager) SetOutboundRateLimit(ratePerMinute float64, burstSize int) {
	if m.outboundRateLimiter != nil {
		m.outboundRateLimiter.SetRate(ratePerMinute, burstSize)
	}
}

// EnableOutboundRateLimit enables the outbound rate limiter.
func (m *Manager) EnableOutboundRateLimit() {
	if m.outboundRateLimiter != nil {
		m.outboundRateLimiter.SetEnabled(true)
	}
}

// DisableOutboundRateLimit disables the outbound rate limiter.
func (m *Manager) DisableOutboundRateLimit() {
	if m.outboundRateLimiter != nil {
		m.outboundRateLimiter.SetEnabled(false)
	}
}

// IsOutboundRateLimitEnabled returns whether the outbound rate limiter is enabled.
func (m *Manager) IsOutboundRateLimitEnabled() bool {
	if m.outboundRateLimiter != nil {
		return m.outboundRateLimiter.IsEnabled()
	}
	return false
}

// GetOutboundRateLimitStats returns statistics about the outbound rate limiter.
func (m *Manager) GetOutboundRateLimitStats() map[string]int64 {
	if m.outboundRateLimiter != nil {
		return m.outboundRateLimiter.GetStats()
	}
	return nil
}

func (m *Manager) handleMessageEvent(evt *events.Message) {
	// Only process group messages
	if !evt.Info.IsGroup {
		return
	}

	m.messageStats.TotalReceived.Add(1)

	// Extract message content with size limit
	content, wasTruncated := extractTextContentWithLimit(evt.Message, maxMessageContentSize)
	if content == "" {
		return // Skip non-text messages
	}

	if wasTruncated {
		m.messageStats.TotalTruncated.Add(1)
		m.log.Debug().
			Str("msg_id", evt.Info.ID).
			Int("original_size", len(extractTextContent(evt.Message))).
			Int("truncated_size", len(content)).
			Msg("Message content truncated due to size limit")
	}

	// Get group info with caching
	groupJID := evt.Info.Chat.String()
	groupName := m.fetchGroupName(evt.Info.Chat, groupJID)

	// Get bot handler under lock
	m.mu.RLock()
	botHandler := m.botHandler
	m.mu.RUnlock()

	// Build incoming message
	msg := &IncomingMessage{
		ID:          evt.Info.ID,
		GroupJID:    groupJID,
		GroupName:   groupName,
		SenderJID:   evt.Info.Sender.String(),
		SenderPhone: extractPhoneNumber(evt.Info.Sender),
		SenderName:  evt.Info.PushName,
		Content:     content,
		Timestamp:   evt.Info.Timestamp,
		IsFromMe:    evt.Info.IsFromMe,
	}

	// Extract reply context if this is a reply to another message
	m.extractReplyContext(evt.Message, msg)

	// Check if this is a bot command (before group monitoring check)
	if botHandler != nil && core.IsCommand(content) {
		response := botHandler.HandleIncoming(context.Background(), evt.Info.Sender.String(), groupJID, content, evt.Info.Timestamp)
		if response != nil && response.Text != "" {
			go m.sendBotResponse(evt.Info.Chat, response.Text)
		}
		return // Don't process as regular message
	}

	// Check if group is monitored
	if !m.isGroupMonitored(groupJID) {
		return
	}

	// Use ordered queue if enabled, otherwise notify handlers directly
	if m.useOrderedQueue && m.orderedQueue != nil {
		m.orderedQueue.Enqueue(msg)
	} else {
		m.notifyHandlers(msg)
	}

	m.messageStats.TotalProcessed.Add(1)
}

// extractTextContent extracts text content from a WhatsApp message.
// Returns the content and whether it was truncated.
func extractTextContent(msg *waE2E.Message) string {
	if msg == nil {
		return ""
	}

	var content string
	if msg.Conversation != nil {
		content = *msg.Conversation
	} else if msg.ExtendedTextMessage != nil && msg.ExtendedTextMessage.Text != nil {
		content = *msg.ExtendedTextMessage.Text
	}

	return content
}

// extractTextContentWithLimit extracts text content with size limit enforcement.
// Returns the content (possibly truncated) and whether truncation occurred.
func extractTextContentWithLimit(msg *waE2E.Message, maxSize int) (string, bool) {
	content := extractTextContent(msg)
	if content == "" {
		return "", false
	}

	return TruncateContent(content, maxSize)
}

// TruncateContent truncates content to the specified maximum size.
// Returns the content (possibly truncated) and whether truncation occurred.
func TruncateContent(content string, maxSize int) (string, bool) {
	if maxSize <= 0 {
		maxSize = maxMessageContentSize
	}

	if len(content) <= maxSize {
		return content, false
	}

	// Truncate and add suffix
	truncateAt := maxSize - len(truncatedMessageSuffix)
	if truncateAt < 0 {
		truncateAt = 0
	}

	// Try to truncate at a word boundary
	truncated := content[:truncateAt]
	lastSpace := findLastSpace(truncated)
	if lastSpace > truncateAt/2 {
		truncated = truncated[:lastSpace]
	}

	return truncated + truncatedMessageSuffix, true
}

// findLastSpace finds the last space character in a string.
func findLastSpace(s string) int {
	for i := len(s) - 1; i >= 0; i-- {
		if s[i] == ' ' || s[i] == '\n' || s[i] == '\t' {
			return i
		}
	}
	return -1
}

// fetchGroupName retrieves the group name with caching and timeout, falling back to JID.
// Uses a cache to reduce API calls and improve performance.
func (m *Manager) fetchGroupName(chat types.JID, fallback string) string {
	jidStr := chat.String()

	// Check cache first
	if m.groupInfoCache != nil {
		if name, found := m.groupInfoCache.Get(jidStr); found {
			return name
		}
	}

	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		return fallback
	}

	ctx, cancel := context.WithTimeout(context.Background(), groupInfoTimeout)
	defer cancel()

	groupInfo, err := client.GetGroupInfo(ctx, chat)
	if err != nil {
		m.log.Debug().Err(err).Str("jid", fallback).Msg("Failed to get group info")
		// Cache the fallback to avoid repeated failed lookups
		if m.groupInfoCache != nil {
			m.groupInfoCache.Set(jidStr, fallback)
		}
		return fallback
	}

	// Cache the successful result
	if m.groupInfoCache != nil {
		m.groupInfoCache.Set(jidStr, groupInfo.Name)
	}

	return groupInfo.Name
}

// extractReplyContext extracts reply context from an extended text message
func (m *Manager) extractReplyContext(waMsg *waE2E.Message, msg *IncomingMessage) {
	if waMsg.ExtendedTextMessage == nil {
		return
	}

	ctxInfo := waMsg.ExtendedTextMessage.ContextInfo
	if ctxInfo == nil {
		return
	}

	if ctxInfo.StanzaID != nil {
		msg.ReplyToID = *ctxInfo.StanzaID
	}
	if ctxInfo.Participant != nil {
		msg.ReplyToSender = *ctxInfo.Participant
	}
	if ctxInfo.QuotedMessage != nil {
		msg.ReplyToContent = extractTextContent(ctxInfo.QuotedMessage)
	}
}

// notifyHandlers sends the message to all registered handlers with panic recovery
func (m *Manager) notifyHandlers(msg *IncomingMessage) {
	m.mu.RLock()
	handlers := m.handlers
	m.mu.RUnlock()

	for _, h := range handlers {
		go func(handler EventHandler) {
			defer func() {
				if r := recover(); r != nil {
					m.log.Error().
						Interface("panic", r).
						Str("message_id", msg.ID).
						Str("group", msg.GroupName).
						Msg("Handler panic recovered - message processing failed")
				}
			}()
			handler.HandleMessage(msg)
		}(h)
	}
}

func (m *Manager) isGroupMonitored(jid string) bool {
	// If no specific groups configured, monitor all
	if len(m.cfg.MonitoredGroups) == 0 {
		return true
	}

	return slices.Contains(m.cfg.MonitoredGroups, jid)
}

// reconnectWithBackoff attempts to reconnect using the battle-tested reconnector.
func (m *Manager) reconnectWithBackoff() {
	// Create a cancellable context that stops when the manager stops
	ctx, cancel := context.WithCancel(context.Background())
	go func() {
		<-m.stopChan
		cancel()
	}()

	// Use the reconnector package for exponential backoff with jitter
	err := m.reconnector.Run(ctx, func(ctx context.Context) error {
		// Check if already connected (early exit)
		if m.State() == StateConnected {
			return nil
		}

		// Attempt connection with timeout
		connectCtx, connectCancel := context.WithTimeout(ctx, defaultConnectTimeout)
		defer connectCancel()

		return m.Connect(connectCtx)
	})

	if err != nil {
		m.log.Error().Err(err).Msg("Reconnection loop ended with error")
	}
}

// onMaxAttemptsReached handles when max reconnection attempts are exhausted.
func (m *Manager) onMaxAttemptsReached(attempts int) {
	m.setState(StateFailed)
	metrics.WhatsAppReconnectFailures.Inc()

	m.log.Error().
		Int("attempts", attempts).
		Msg("Max reconnection attempts reached")

	// Send admin alert
	if m.alerter != nil {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		err := m.alerter.SendAlert(ctx, "critical", "WhatsApp Disconnected",
			fmt.Sprintf("WhatsApp connection failed after %d attempts. Manual intervention required.", attempts))
		if err != nil {
			m.log.Error().Err(err).Msg("Failed to send admin alert")
		}
	}
}

// ForceReconnect triggers an immediate reconnection attempt.
func (m *Manager) ForceReconnect() {
	select {
	case m.reconnectChan <- struct{}{}:
		m.log.Info().Msg("Force reconnect signal sent")
	default:
		m.log.Warn().Msg("Reconnect already in progress")
	}
}

// SetAlerter sets the alert notifier for admin notifications.
func (m *Manager) SetAlerter(alerter AlertNotifier) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.alerter = alerter
}

// GetConnectionStatus returns detailed connection status.
func (m *Manager) GetConnectionStatus() ConnectionStatus {
	return ConnectionStatus{
		State:           m.State(),
		ReconnectCount:  int(m.reconnectCount.Load()),
		LastConnectedAt: time.Unix(m.lastConnectedAt.Load(), 0),
		UptimeSeconds:   m.getUptimeSeconds(),
	}
}

// ConnectionStatus represents detailed connection status.
type ConnectionStatus struct {
	State           ConnectionState `json:"state"`
	ReconnectCount  int             `json:"reconnect_count"`
	LastConnectedAt time.Time       `json:"last_connected_at"`
	UptimeSeconds   int64           `json:"uptime_seconds"`
}

func (m *Manager) getUptimeSeconds() int64 {
	if m.State() != StateConnected {
		return 0
	}
	lastConn := m.lastConnectedAt.Load()
	if lastConn == 0 {
		return 0
	}
	return time.Now().Unix() - lastConn
}

func extractPhoneNumber(jid types.JID) string {
	return jid.User
}

// SendMessage sends a text message to the specified JID with rate limiting.
func (m *Manager) SendMessage(ctx context.Context, jidStr, content string) error {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		return fmt.Errorf("not connected")
	}

	// Apply rate limiting
	if m.outboundRateLimiter != nil {
		if err := m.outboundRateLimiter.Wait(ctx); err != nil {
			return fmt.Errorf("rate limit exceeded: %w", err)
		}
	}

	jid, err := types.ParseJID(jidStr)
	if err != nil {
		return fmt.Errorf("invalid JID: %w", err)
	}

	msg := &waE2E.Message{
		ExtendedTextMessage: &waE2E.ExtendedTextMessage{
			Text: proto.String(content),
		},
	}

	_, err = client.SendMessage(ctx, jid, msg)
	return err
}

// GenerateMessageID creates a unique message ID
func GenerateMessageID() string {
	return uuid.New().String()
}

// SetBotHandler sets the bot command handler
func (m *Manager) SetBotHandler(handler *whatsappbot.Bot) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.botHandler = handler
}

// sendBotResponse sends a response message back to the chat with rate limiting.
func (m *Manager) sendBotResponse(chat types.JID, response string) {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		m.log.Warn().Msg("Cannot send bot response - not connected")
		return
	}

	// Apply rate limiting
	if m.outboundRateLimiter != nil {
		ctx, cancel := context.WithTimeout(context.Background(), rateLimitWaitTimeout)
		defer cancel()

		if err := m.outboundRateLimiter.Wait(ctx); err != nil {
			m.log.Warn().
				Err(err).
				Str("chat", chat.String()).
				Msg("Bot response dropped due to rate limit")
			return
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), botResponseTimeout)
	defer cancel()

	msg := &waE2E.Message{
		ExtendedTextMessage: &waE2E.ExtendedTextMessage{
			Text: proto.String(response),
		},
	}

	_, err := client.SendMessage(ctx, chat, msg)
	if err != nil {
		m.log.Error().Err(err).Str("chat", chat.String()).Msg("Failed to send bot response")
	} else {
		m.log.Debug().Str("chat", chat.String()).Msg("Bot response sent")
	}
}
