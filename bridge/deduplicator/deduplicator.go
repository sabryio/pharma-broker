// Package deduplicator provides in-memory message deduplication.
package deduplicator

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
)

// Config holds configuration for the message deduplicator.
type Config struct {
	Window          time.Duration
	CacheSize       int
	CacheTTL        time.Duration
	CleanupInterval time.Duration
}

// cacheKey is a strongly-typed key for the dedup cache.
type cacheKey string

// makeCacheKey creates a key from sender JID and content hash.
// This makes deduplication group-agnostic, catching identical messages
// from the same sender across multiple groups.
func makeCacheKey(senderJID domain.JID, content string) cacheKey {
	contentHash := sha256.Sum256([]byte(content))
	return cacheKey(string(senderJID) + "|" + hex.EncodeToString(contentHash[:8]))
}

type cacheEntry struct {
	Content   string
	Timestamp time.Time
	ExpiresAt time.Time
}

// Deduplicator detects and filters duplicate messages using in-memory cache.
type Deduplicator struct {
	cfg     Config
	log     zerolog.Logger
	cache   map[cacheKey]cacheEntry
	cacheMu sync.RWMutex
	hits    atomic.Int64
	misses  atomic.Int64
	cancel  context.CancelFunc
	wg      sync.WaitGroup
	closed  atomic.Bool
}

// New creates a new message deduplicator.
func New(ctx context.Context, cfg Config, log zerolog.Logger) *Deduplicator {
	ctx, cancel := context.WithCancel(ctx)

	d := &Deduplicator{
		cfg:    cfg,
		log:    log.With().Str("component", "deduplicator").Logger(),
		cache:  make(map[cacheKey]cacheEntry, cfg.CacheSize),
		cancel: cancel,
	}

	d.wg.Add(1)
	go d.cleanupLoop(ctx)

	d.log.Info().
		Dur("window", cfg.Window).
		Int("cache_size", cfg.CacheSize).
		Dur("ttl", cfg.CacheTTL).
		Msg("Deduplicator started")

	return d
}

// Close stops background goroutines.
func (d *Deduplicator) Close() {
	if d.closed.Swap(true) {
		return // Already closed
	}
	if d.cancel != nil {
		d.cancel()
	}
	d.wg.Wait()
}

// IsDuplicate checks if a message is a duplicate.
func (d *Deduplicator) IsDuplicate(groupJID, senderJID domain.JID, content string, timestamp time.Time) bool {
	d.cacheMu.RLock()
	defer d.cacheMu.RUnlock()

	key := makeCacheKey(senderJID, content)
	entry, exists := d.cache[key]
	if !exists {
		d.misses.Add(1)
		return false
	}

	if time.Now().After(entry.ExpiresAt) {
		d.misses.Add(1)
		return false
	}

	timeDiff := absDuration(timestamp.Sub(entry.Timestamp))
	if timeDiff >= d.cfg.Window {
		d.misses.Add(1)
		return false
	}

	if entry.Content == content {
		d.hits.Add(1)
		d.log.Debug().
			Str("sender", string(senderJID)).
			Str("group", string(groupJID)).
			Dur("time_diff", timeDiff).
			Msg("Duplicate message detected (cross-group)")
		return true
	}

	d.misses.Add(1)
	return false
}

// Record stores the message for future deduplication checks.
func (d *Deduplicator) Record(groupJID, senderJID domain.JID, content string, timestamp time.Time) {
	key := makeCacheKey(senderJID, content)
	entry := cacheEntry{
		Content:   content,
		Timestamp: timestamp,
		ExpiresAt: time.Now().Add(d.cfg.CacheTTL),
	}

	d.cacheMu.Lock()
	defer d.cacheMu.Unlock()

	if len(d.cache) >= d.cfg.CacheSize {
		d.evictOne()
	}

	d.cache[key] = entry
}

func absDuration(d time.Duration) time.Duration {
	if d < 0 {
		return -d
	}
	return d
}

func (d *Deduplicator) evictOne() {
	for key := range d.cache {
		delete(d.cache, key)
		return
	}
}

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

var _ ports.Deduplicator = (*Deduplicator)(nil)
