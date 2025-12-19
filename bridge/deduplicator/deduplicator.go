// Package deduplicator provides in-memory message deduplication.
// Simplified port from legacy/messaging/deduplicator for the minimal bridge.
package deduplicator

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// Config holds configuration for the message deduplicator.
type Config struct {
	// Window is the time window for detecting duplicates.
	Window time.Duration
	// CacheSize is the maximum number of entries in the cache.
	CacheSize int
	// CacheTTL is how long entries stay in cache.
	CacheTTL time.Duration
	// CleanupInterval is how often expired entries are removed.
	CleanupInterval time.Duration
}

// DefaultConfig returns sensible defaults.
func DefaultConfig() Config {
	return Config{
		Window:          10 * time.Second,
		CacheSize:       10000,
		CacheTTL:        30 * time.Second,
		CleanupInterval: time.Minute,
	}
}

// Deduplicator detects and filters duplicate messages using in-memory cache.
type Deduplicator struct {
	cfg Config
	log zerolog.Logger

	// In-memory cache: key (group|sender) -> entry
	cache   map[string]cacheEntry
	cacheMu sync.RWMutex

	// Stats (atomic for thread safety)
	hits   atomic.Int64
	misses atomic.Int64

	// Lifecycle
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// cacheEntry represents a cached message for deduplication.
type cacheEntry struct {
	Content   string
	Timestamp time.Time
	ExpiresAt time.Time
}

// New creates a new message deduplicator.
func New(ctx context.Context, cfg Config, log zerolog.Logger) *Deduplicator {
	ctx, cancel := context.WithCancel(ctx)

	d := &Deduplicator{
		cfg:    cfg,
		log:    log.With().Str("component", "deduplicator").Logger(),
		cache:  make(map[string]cacheEntry, cfg.CacheSize),
		cancel: cancel,
	}

	// Start cleanup goroutine
	d.wg.Add(1)
	go d.cleanupLoop(ctx)

	d.log.Info().
		Dur("window", cfg.Window).
		Int("cache_size", cfg.CacheSize).
		Dur("ttl", cfg.CacheTTL).
		Msg("Deduplicator started")

	return d
}

// Close stops background goroutines and releases resources.
func (d *Deduplicator) Close() {
	if d.cancel != nil {
		d.cancel()
	}
	d.wg.Wait()
}

// IsDuplicate checks if a message is a duplicate of a recent message.
// Returns true if the message should be filtered out.
func (d *Deduplicator) IsDuplicate(groupJID, senderJID, content string, timestamp time.Time) bool {
	d.cacheMu.RLock()
	defer d.cacheMu.RUnlock()

	key := cacheKey(groupJID, senderJID)
	entry, exists := d.cache[key]
	if !exists {
		d.misses.Add(1)
		return false
	}

	// Check if entry is expired
	if time.Now().After(entry.ExpiresAt) {
		d.misses.Add(1)
		return false
	}

	// Check if within deduplication window
	timeDiff := absDuration(timestamp.Sub(entry.Timestamp))
	if timeDiff >= d.cfg.Window {
		d.misses.Add(1)
		return false
	}

	// Check content match
	if entry.Content == content {
		d.hits.Add(1)
		d.log.Debug().
			Str("sender", senderJID).
			Dur("time_diff", timeDiff).
			Msg("Duplicate message detected")
		return true
	}

	d.misses.Add(1)
	return false
}

// RecordMessage records a message in the cache for future deduplication checks.
func (d *Deduplicator) RecordMessage(groupJID, senderJID, content string, timestamp time.Time) {
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
		d.evictOne()
	}

	d.cache[key] = entry
}

// cacheKey generates a unique key for sender+group combination.
func cacheKey(groupJID, senderJID string) string {
	return groupJID + "|" + senderJID
}

// absDuration returns the absolute value of a duration.
func absDuration(d time.Duration) time.Duration {
	if d < 0 {
		return -d
	}
	return d
}

// evictOne removes one entry from the cache to make room.
// Caller must hold the write lock.
func (d *Deduplicator) evictOne() {
	for key := range d.cache {
		delete(d.cache, key)
		return
	}
}

// cleanupLoop periodically removes expired entries from the cache.
func (d *Deduplicator) cleanupLoop(ctx context.Context) {
	defer d.wg.Done()

	ticker := time.NewTicker(d.cfg.CleanupInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			d.cleanup()
		}
	}
}

// cleanup removes expired entries from the cache.
func (d *Deduplicator) cleanup() {
	d.cacheMu.Lock()
	defer d.cacheMu.Unlock()

	now := time.Now()
	count := 0
	for key, entry := range d.cache {
		if now.After(entry.ExpiresAt) {
			delete(d.cache, key)
			count++
		}
	}

	if count > 0 {
		d.log.Debug().Int("evicted", count).Int("remaining", len(d.cache)).Msg("Cache cleanup")
	}
}

// Stats returns deduplication statistics.
func (d *Deduplicator) Stats() Stats {
	d.cacheMu.RLock()
	cacheSize := len(d.cache)
	d.cacheMu.RUnlock()

	hits := d.hits.Load()
	misses := d.misses.Load()

	return Stats{
		Hits:      hits,
		Misses:    misses,
		CacheSize: cacheSize,
		HitRate:   calculateHitRate(hits, misses),
	}
}

// Stats holds deduplication statistics.
type Stats struct {
	Hits      int64   `json:"hits"`
	Misses    int64   `json:"misses"`
	CacheSize int     `json:"cache_size"`
	HitRate   float64 `json:"hit_rate_pct"`
}

func calculateHitRate(hits, misses int64) float64 {
	total := hits + misses
	if total == 0 {
		return 0
	}
	return float64(hits) / float64(total) * 100
}
