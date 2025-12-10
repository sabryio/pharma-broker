package whatsapp

import (
	"context"
	"sync"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/metrics"
)

// DeduplicatorConfig holds configuration for the message deduplicator.
type DeduplicatorConfig struct {
	// Window is the time window for detecting duplicates.
	Window time.Duration
	// UseInMemoryCache enables in-memory caching for faster lookups.
	UseInMemoryCache bool
	// CacheSize is the maximum number of entries in the in-memory cache.
	CacheSize int
	// CacheTTL is how long entries stay in cache.
	CacheTTL time.Duration
}

// DefaultDeduplicatorConfig returns sensible defaults.
func DefaultDeduplicatorConfig() DeduplicatorConfig {
	return DeduplicatorConfig{
		Window:           10 * time.Second,
		UseInMemoryCache: true,
		CacheSize:        10000,
		CacheTTL:         30 * time.Second,
	}
}

// MessageLookup defines the interface for looking up previous messages.
// This abstraction allows for testing without a real database.
type MessageLookup interface {
	GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*entity.RawMessage, error)
}

// Deduplicator detects and filters duplicate messages.
type Deduplicator struct {
	cfg    DeduplicatorConfig
	log    zerolog.Logger
	lookup MessageLookup

	// In-memory cache for fast lookups
	cache   map[string]cacheEntry
	cacheMu sync.RWMutex

	// Stats
	hits   int64
	misses int64
}

// cacheEntry represents a cached message for deduplication.
type cacheEntry struct {
	Content   string
	Timestamp time.Time
	ExpiresAt time.Time
}

// cacheKey generates a unique key for sender+group combination.
func cacheKey(groupJID, senderJID string) string {
	return groupJID + "|" + senderJID
}

// NewDeduplicator creates a new message deduplicator.
func NewDeduplicator(cfg DeduplicatorConfig, lookup MessageLookup, log zerolog.Logger) *Deduplicator {
	d := &Deduplicator{
		cfg:    cfg,
		log:    log.With().Str("component", "deduplicator").Logger(),
		lookup: lookup,
	}

	if cfg.UseInMemoryCache {
		d.cache = make(map[string]cacheEntry, cfg.CacheSize)
		go d.cleanupLoop()
	}

	return d
}

// IsDuplicate checks if a message is a duplicate of a recent message.
// Returns true if the message should be filtered out.
func (d *Deduplicator) IsDuplicate(ctx context.Context, groupJID, senderJID, content string, timestamp time.Time) bool {
	// Try in-memory cache first (fast path)
	if d.cfg.UseInMemoryCache {
		if d.checkCache(groupJID, senderJID, content, timestamp) {
			metrics.DeduplicatorHits.Inc()
			d.hits++
			return true
		}
	}

	// Fall back to database lookup (slow path)
	if d.lookup != nil {
		lastMsg, err := d.lookup.GetLastMessageBySender(ctx, groupJID, senderJID)
		if err != nil {
			d.log.Debug().Err(err).Msg("Failed to lookup last message")
			return false
		}

		if lastMsg != nil && d.isDuplicateOf(lastMsg, content, timestamp) {
			metrics.DeduplicatorHits.Inc()
			d.hits++
			return true
		}
	}

	metrics.DeduplicatorMisses.Inc()
	d.misses++
	return false
}

// RecordMessage records a message in the cache for future deduplication checks.
func (d *Deduplicator) RecordMessage(groupJID, senderJID, content string, timestamp time.Time) {
	if !d.cfg.UseInMemoryCache {
		return
	}

	key := cacheKey(groupJID, senderJID)
	entry := cacheEntry{
		Content:   content,
		Timestamp: timestamp,
		ExpiresAt: time.Now().Add(d.cfg.CacheTTL),
	}

	d.cacheMu.Lock()
	defer d.cacheMu.Unlock()

	// Evict if cache is full
	if len(d.cache) >= d.cfg.CacheSize {
		d.evictOldest()
	}

	d.cache[key] = entry
}

// checkCache checks the in-memory cache for duplicates.
func (d *Deduplicator) checkCache(groupJID, senderJID, content string, timestamp time.Time) bool {
	d.cacheMu.RLock()
	defer d.cacheMu.RUnlock()

	key := cacheKey(groupJID, senderJID)
	entry, exists := d.cache[key]
	if !exists {
		return false
	}

	// Check if entry is expired
	if time.Now().After(entry.ExpiresAt) {
		return false
	}

	// Check if within deduplication window
	timeDiff := absDuration(timestamp.Sub(entry.Timestamp))
	if timeDiff >= d.cfg.Window {
		return false
	}

	// Check content match
	return entry.Content == content
}

// isDuplicateOf checks if a message is a duplicate of the given previous message.
func (d *Deduplicator) isDuplicateOf(lastMsg *entity.RawMessage, content string, timestamp time.Time) bool {
	timeDiff := absDuration(timestamp.Sub(lastMsg.Timestamp))
	return timeDiff < d.cfg.Window && lastMsg.Content == content
}

// evictOldest removes the oldest entry from the cache.
// Caller must hold the write lock.
func (d *Deduplicator) evictOldest() {
	var oldestKey string
	var oldestTime time.Time

	for key, entry := range d.cache {
		if oldestKey == "" || entry.Timestamp.Before(oldestTime) {
			oldestKey = key
			oldestTime = entry.Timestamp
		}
	}

	if oldestKey != "" {
		delete(d.cache, oldestKey)
	}
}

// cleanupLoop periodically removes expired entries from the cache.
func (d *Deduplicator) cleanupLoop() {
	ticker := time.NewTicker(time.Minute)
	defer ticker.Stop()

	for range ticker.C {
		d.cleanup()
	}
}

// cleanup removes expired entries from the cache.
func (d *Deduplicator) cleanup() {
	d.cacheMu.Lock()
	defer d.cacheMu.Unlock()

	now := time.Now()
	for key, entry := range d.cache {
		if now.After(entry.ExpiresAt) {
			delete(d.cache, key)
		}
	}
}

// Stats returns deduplication statistics.
func (d *Deduplicator) Stats() DeduplicatorStats {
	d.cacheMu.RLock()
	cacheSize := len(d.cache)
	d.cacheMu.RUnlock()

	return DeduplicatorStats{
		Hits:      d.hits,
		Misses:    d.misses,
		CacheSize: cacheSize,
		HitRate:   d.calculateHitRate(),
	}
}

// calculateHitRate returns the cache hit rate as a percentage.
func (d *Deduplicator) calculateHitRate() float64 {
	total := d.hits + d.misses
	if total == 0 {
		return 0
	}
	return float64(d.hits) / float64(total) * 100
}

// DeduplicatorStats holds deduplication statistics.
type DeduplicatorStats struct {
	Hits      int64   `json:"hits"`
	Misses    int64   `json:"misses"`
	CacheSize int     `json:"cache_size"`
	HitRate   float64 `json:"hit_rate_pct"`
}

// Clear clears the in-memory cache.
func (d *Deduplicator) Clear() {
	if d.cfg.UseInMemoryCache {
		d.cacheMu.Lock()
		d.cache = make(map[string]cacheEntry, d.cfg.CacheSize)
		d.cacheMu.Unlock()
	}
	d.hits = 0
	d.misses = 0
}
