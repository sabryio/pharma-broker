// Package historysync provides deduplication and processing for WhatsApp history sync events.
package historysync

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// Config holds configuration for history sync handling.
type Config struct {
	Cooldown    time.Duration // Minimum time between processing history syncs
	MaxAge      time.Duration // Only process messages newer than this
	MaxMessages int           // Maximum messages to process per sync
	CacheSize   int           // Size of processed message IDs cache
	CacheTTL    time.Duration // TTL for processed IDs cache entries
}

// Stats tracks history sync processing statistics.
type Stats struct {
	TotalSyncs        atomic.Int64 // Total history sync events received
	SkippedCooldown   atomic.Int64 // Syncs skipped due to cooldown
	MessagesReceived  atomic.Int64 // Total messages in sync events
	MessagesSkipped   atomic.Int64 // Messages skipped (old, duplicate, etc.)
	MessagesProcessed atomic.Int64 // Messages actually processed
}

// Handler manages history sync deduplication and processing.
type Handler struct {
	cfg    Config
	logger zerolog.Logger

	// State
	lastSyncAt     atomic.Int64     // Unix timestamp of last history sync
	processedIDs   map[string]int64 // Message ID -> timestamp (for dedup)
	processedIDsMu sync.RWMutex     // Mutex for processedIDs

	// Statistics
	stats Stats
}

// New creates a new history sync handler.
func New(cfg Config, logger zerolog.Logger) *Handler {
	return &Handler{
		cfg:          cfg,
		logger:       logger.With().Str("component", "historysync").Logger(),
		processedIDs: make(map[string]int64, cfg.CacheSize),
	}
}

// ShouldProcess checks if a history sync should be processed based on cooldown.
// Returns true if processing should proceed, false if in cooldown.
func (h *Handler) ShouldProcess() bool {
	h.stats.TotalSyncs.Add(1)

	lastSync := h.lastSyncAt.Load()
	now := time.Now().UnixNano()

	if lastSync > 0 && now-lastSync < h.cfg.Cooldown.Nanoseconds() {
		h.stats.SkippedCooldown.Add(1)
		h.logger.Debug().
			Int64("last_sync_ago_ns", now-lastSync).
			Int64("cooldown_ns", h.cfg.Cooldown.Nanoseconds()).
			Msg("Skipping history sync - cooldown active")
		return false
	}

	// Update last sync timestamp
	h.lastSyncAt.Store(now)
	return true
}

// IsMessageTooOld checks if a message timestamp is older than the max age.
func (h *Handler) IsMessageTooOld(timestamp time.Time) bool {
	cutoff := time.Now().Add(-h.cfg.MaxAge)
	return timestamp.Before(cutoff)
}

// IsMessageProcessed checks if a message ID has already been processed.
func (h *Handler) IsMessageProcessed(msgID string) bool {
	h.processedIDsMu.RLock()
	defer h.processedIDsMu.RUnlock()
	_, exists := h.processedIDs[msgID]
	return exists
}

// MarkMessageProcessed marks a message ID as processed.
func (h *Handler) MarkMessageProcessed(msgID string) {
	h.processedIDsMu.Lock()
	defer h.processedIDsMu.Unlock()
	h.processedIDs[msgID] = time.Now().UnixNano()
}

// CleanupCache removes old entries from the processed IDs cache.
func (h *Handler) CleanupCache() {
	h.processedIDsMu.Lock()
	defer h.processedIDsMu.Unlock()

	cutoff := time.Now().Add(-h.cfg.CacheTTL).UnixNano()
	deleted := 0

	for id, ts := range h.processedIDs {
		if ts < cutoff {
			delete(h.processedIDs, id)
			deleted++
		}
	}

	// If still over capacity, remove oldest entries
	if len(h.processedIDs) > h.cfg.CacheSize {
		for id := range h.processedIDs {
			if len(h.processedIDs) <= h.cfg.CacheSize {
				break
			}
			delete(h.processedIDs, id)
			deleted++
		}
	}

	if deleted > 0 {
		h.logger.Debug().
			Int("deleted", deleted).
			Int("remaining", len(h.processedIDs)).
			Msg("Cleaned up processed message IDs cache")
	}
}

// RecordReceived records that messages were received in a sync.
func (h *Handler) RecordReceived(count int) {
	h.stats.MessagesReceived.Add(int64(count))
}

// RecordSkipped records that messages were skipped.
func (h *Handler) RecordSkipped(count int) {
	h.stats.MessagesSkipped.Add(int64(count))
}

// RecordProcessed records that messages were processed.
func (h *Handler) RecordProcessed(count int) {
	h.stats.MessagesProcessed.Add(int64(count))
}

// MaxMessages returns the maximum messages to process per sync.
func (h *Handler) MaxMessages() int {
	return h.cfg.MaxMessages
}

// GetStats returns a snapshot of history sync statistics.
func (h *Handler) GetStats() map[string]int64 {
	return map[string]int64{
		"total_syncs":        h.stats.TotalSyncs.Load(),
		"skipped_cooldown":   h.stats.SkippedCooldown.Load(),
		"messages_received":  h.stats.MessagesReceived.Load(),
		"messages_skipped":   h.stats.MessagesSkipped.Load(),
		"messages_processed": h.stats.MessagesProcessed.Load(),
	}
}

// Reset resets the handler state (useful for testing).
func (h *Handler) Reset() {
	h.lastSyncAt.Store(0)
	h.processedIDsMu.Lock()
	h.processedIDs = make(map[string]int64, h.cfg.CacheSize)
	h.processedIDsMu.Unlock()
	h.logger.Info().Msg("History sync state reset")
}

// CacheSize returns the current number of cached message IDs.
func (h *Handler) CacheSize() int {
	h.processedIDsMu.RLock()
	defer h.processedIDsMu.RUnlock()
	return len(h.processedIDs)
}
