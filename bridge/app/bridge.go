// Package app contains the application orchestration logic.
// Pure business logic with no infrastructure dependencies (Hexagonal Architecture).
package app

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
)

// Bridge orchestrates message flow from source to sink.
// Depends only on interfaces (ports), not concrete implementations (DIP).
type Bridge struct {
	source        ports.MessageSource
	sink          ports.MessageSink
	groupCache    ports.GroupCache
	groupRepo     ports.GroupRepository
	groupSyncer   ports.GroupSyncer
	groupProvider ports.GroupProvider
	dedup         ports.Deduplicator
	rateLimiter   ports.RateLimiter
	logger        zerolog.Logger

	skipOwnMessages   bool
	workers           []chan domain.Message
	messagesForwarded atomic.Int64
	stopped           atomic.Bool
	stopOnce          sync.Once
}

// BridgeConfig holds configuration for the bridge.
type BridgeConfig struct {
	SkipOwnMessages   bool
	WorkerCount       int
	WorkerQueueSize   int
	GroupSyncInterval time.Duration
}

// DefaultBridgeConfig returns sensible defaults.
func DefaultBridgeConfig() BridgeConfig {
	return BridgeConfig{
		SkipOwnMessages:   true,
		WorkerCount:       20,
		WorkerQueueSize:   100,
		GroupSyncInterval: 5 * time.Minute,
	}
}

// BridgeParams holds dependencies for creating a Bridge.
type BridgeParams struct {
	Source        ports.MessageSource
	Sink          ports.MessageSink
	GroupCache    ports.GroupCache
	GroupRepo     ports.GroupRepository
	GroupSyncer   ports.GroupSyncer
	GroupProvider ports.GroupProvider
	Dedup         ports.Deduplicator
	RateLimiter   ports.RateLimiter
	Logger        zerolog.Logger
	Config        BridgeConfig
}

// NewBridge creates a new Bridge with the given dependencies.
func NewBridge(params BridgeParams) *Bridge {
	cfg := params.Config
	if cfg.WorkerCount <= 0 {
		cfg.WorkerCount = 20
	}
	if cfg.WorkerQueueSize <= 0 {
		cfg.WorkerQueueSize = 100
	}

	b := &Bridge{
		source:          params.Source,
		sink:            params.Sink,
		groupCache:      params.GroupCache,
		groupRepo:       params.GroupRepo,
		groupSyncer:     params.GroupSyncer,
		groupProvider:   params.GroupProvider,
		dedup:           params.Dedup,
		rateLimiter:     params.RateLimiter,
		logger:          params.Logger.With().Str("component", "bridge").Logger(),
		skipOwnMessages: cfg.SkipOwnMessages,
		workers:         make([]chan domain.Message, cfg.WorkerCount),
	}

	for i := range cfg.WorkerCount {
		b.workers[i] = make(chan domain.Message, cfg.WorkerQueueSize)
	}

	return b
}

// Run starts the bridge message processing loop.
func (b *Bridge) Run(ctx context.Context) error {
	for i, ch := range b.workers {
		go b.workerLoop(ctx, i, ch)
	}

	// Sync WhatsApp groups to Core on startup
	go b.syncWhatsAppGroupsOnConnect(ctx)

	go b.syncGroupsWorker(ctx)
	b.syncGroups(ctx)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case msg, ok := <-b.source.Messages():
			if !ok {
				return nil
			}
			b.routeMessage(msg)
		}
	}
}

func (b *Bridge) routeMessage(msg domain.Message) {
	if !msg.IsGroup {
		return
	}

	// Shard by Group JID for ordered processing per group
	hash := hashJID(msg.GroupJID)
	workerIdx := hash % len(b.workers)

	select {
	case b.workers[workerIdx] <- msg:
	default:
		b.logger.Warn().
			Str("group", msg.GroupJID.String()).
			Int("worker", workerIdx).
			Msg("Worker queue full, message dropped")
	}
}

func hashJID(jid domain.JID) int {
	s := string(jid)
	hash := 0
	for i := 0; i < len(s); i++ {
		hash = 31*hash + int(s[i])
	}
	return hash & 0x7fffffff
}

func (b *Bridge) workerLoop(ctx context.Context, id int, ch chan domain.Message) {
	b.logger.Debug().Int("worker_id", id).Msg("Worker started")
	for {
		select {
		case <-ctx.Done():
			return
		case msg, ok := <-ch:
			if !ok {
				return
			}
			b.processMessage(ctx, msg)
		}
	}
}

func (b *Bridge) processMessage(ctx context.Context, msg domain.Message) {
	if !msg.IsGroup {
		return
	}

	if msg.IsFromMe && b.skipOwnMessages {
		b.logger.Debug().Msg("Skipping own message")
		return
	}

	if msg.Content == "" {
		return
	}

	if !b.groupCache.IsMonitored(msg.GroupJID) {
		return
	}

	if !b.rateLimiter.Allow() {
		b.logger.Debug().Msg("Rate limited")
		return
	}

	msgTime := time.Unix(msg.Timestamp.Int64(), 0)
	if b.dedup.IsDuplicate(msg.GroupJID, msg.SenderJID, msg.Content, msgTime) {
		b.logger.Debug().Str("sender", msg.SenderJID.String()).Msg("Duplicate message ignored")
		return
	}

	b.dedup.Record(msg.GroupJID, msg.SenderJID, msg.Content, msgTime)

	b.logger.Debug().
		Str("group", msg.GroupJID.String()).
		Int("content_len", len(msg.Content)).
		Msg("Processing monitored message")

	if err := b.sink.Send(ctx, msg); err != nil {
		b.logger.Warn().Err(err).Msg("Failed to forward message")
		return
	}

	b.messagesForwarded.Add(1)
}

func (b *Bridge) syncGroups(ctx context.Context) {
	b.logger.Debug().Msg("Syncing monitored groups...")

	jids, err := b.groupRepo.GetMonitoredGroups(ctx)
	if err != nil {
		b.logger.Warn().Err(err).Msg("Failed to sync monitored groups")
		return
	}

	b.groupCache.Update(jids)
	b.logger.Info().Int("count", len(jids)).Msg("✅ Monitored groups synced")
}

// syncWhatsAppGroupsOnConnect fetches groups from WhatsApp and syncs them to Core.
func (b *Bridge) syncWhatsAppGroupsOnConnect(ctx context.Context) {
	if b.groupSyncer == nil || b.groupProvider == nil {
		b.logger.Debug().Msg("Group syncer or provider not configured, skipping WhatsApp group sync")
		return
	}

	// Wait for WhatsApp connection
	b.logger.Info().Msg("⏳ Waiting for WhatsApp connection to sync groups...")
	for !b.source.IsConnected() {
		select {
		case <-ctx.Done():
			return
		case <-time.After(time.Second):
		}
	}

	// Small delay to ensure connection is stable
	time.Sleep(2 * time.Second)

	// Fetch groups from WhatsApp
	groups, err := b.groupProvider.GetJoinedGroups(ctx)
	if err != nil {
		b.logger.Error().Err(err).Msg("Failed to get WhatsApp groups")
		return
	}

	if len(groups) == 0 {
		b.logger.Warn().Msg("No WhatsApp groups found")
		return
	}

	// Sync to Core
	added, updated, err := b.groupSyncer.SyncGroups(ctx, groups)
	if err != nil {
		b.logger.Error().Err(err).Msg("Failed to sync groups to Core")
		return
	}

	b.logger.Info().
		Int32("added", added).
		Int32("updated", updated).
		Int("total", len(groups)).
		Msg("📱 WhatsApp groups synced to Core")

	// Refresh monitored groups cache
	b.syncGroups(ctx)
}

func (b *Bridge) syncGroupsWorker(ctx context.Context) {
	ticker := time.NewTicker(5 * time.Minute)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			b.syncGroups(ctx)
		}
	}
}

// Stop gracefully shuts down the bridge.
func (b *Bridge) Stop() {
	b.stopOnce.Do(func() {
		b.stopped.Store(true)

		for _, ch := range b.workers {
			close(ch)
		}

		b.logger.Info().Msg("Bridge stopped")
	})
}

// MessagesForwarded returns the total number of messages forwarded.
func (b *Bridge) MessagesForwarded() int64 {
	return b.messagesForwarded.Load()
}

// IsConnected returns true if the message source is connected.
func (b *Bridge) IsConnected() bool {
	return b.source != nil && b.source.IsConnected()
}
