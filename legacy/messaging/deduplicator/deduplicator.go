package deduplicator

import (
	"context"
	"errors"
	"reflect"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/pkg/metrics"
)

// isNil checks if an interface value holds a nil pointer.
// This is needed for generic types that are pointers.
func isNil[T any](v T) bool {
	val := reflect.ValueOf(v)
	if !val.IsValid() {
		return true
	}
	switch val.Kind() {
	case reflect.Pointer, reflect.Interface, reflect.Slice, reflect.Map, reflect.Chan, reflect.Func:
		return val.IsNil()
	default:
		return false
	}
}

// absDuration returns the absolute value of a duration.
func absDuration(d time.Duration) time.Duration {
	if d < 0 {
		return -d
	}
	return d
}

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
	// CleanupInterval is how often expired entries are removed from cache.
	CleanupInterval time.Duration
}

// Validate checks that the configuration values are valid.
func (cfg DeduplicatorConfig) Validate() error {
	if cfg.Window <= 0 {
		return errors.New("window must be positive")
	}
	if cfg.UseInMemoryCache {
		if cfg.CacheSize <= 0 {
			return errors.New("cache size must be positive when cache is enabled")
		}
		if cfg.CacheTTL <= 0 {
			return errors.New("cache TTL must be positive when cache is enabled")
		}
		if cfg.CleanupInterval <= 0 {
			return errors.New("cleanup interval must be positive when cache is enabled")
		}
	}
	return nil
}

// DefaultDeduplicatorConfig returns sensible defaults.
func DefaultDeduplicatorConfig() DeduplicatorConfig {
	return DeduplicatorConfig{
		Window:           10 * time.Second,
		UseInMemoryCache: true,
		CacheSize:        10000,
		CacheTTL:         30 * time.Second,
		CleanupInterval:  time.Minute,
	}
}

// DedupMessage is the constraint required by Deduplicator.
// Any message type must implement these accessors.
type DedupMessage interface {
	GetTimestamp() time.Time
	GetContent() string
}

// Lookup defines a generic interface for retrieving the last stored item
// for a given composite key (e.g., groupID + senderID).
// T is the type of the entity being returned.
type Lookup[T DedupMessage] interface {
	GetLast(
		ctx context.Context,
		groupID string,
		senderID string,
	) (T, error)
}

// Deduplicator detects and filters duplicate messages.
type Deduplicator[T DedupMessage] struct {
	cfg    DeduplicatorConfig
	log    zerolog.Logger
	lookup Lookup[T]

	// In-memory cache for fast lookups
	cache   map[string]cacheEntry
	cacheMu sync.RWMutex

	// Stats (atomic for thread safety)
	hits   atomic.Int64
	misses atomic.Int64

	// Lifecycle management
	cancel context.CancelFunc
	wg     sync.WaitGroup
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
// The provided context controls the lifecycle of background goroutines.
func NewDeduplicator[T DedupMessage](ctx context.Context, cfg DeduplicatorConfig, lookup Lookup[T], log zerolog.Logger) (*Deduplicator[T], error) {
	if err := cfg.Validate(); err != nil {
		return nil, err
	}

	ctx, cancel := context.WithCancel(ctx)

	d := &Deduplicator[T]{
		cfg:    cfg,
		log:    log.With().Str("component", "deduplicator").Logger(),
		lookup: lookup,
		cancel: cancel,
	}

	if cfg.UseInMemoryCache {
		d.cache = make(map[string]cacheEntry, cfg.CacheSize)
		d.wg.Add(1)
		go d.cleanupLoop(ctx)
	}

	return d, nil
}

// Close stops background goroutines and releases resources.
// It blocks until all goroutines have exited.
func (d *Deduplicator[T]) Close() {
	if d.cancel != nil {
		d.cancel()
	}
	d.wg.Wait()
}

// IsDuplicate checks if a message is a duplicate of a recent message.
// Returns true if the message should be filtered out.
func (d *Deduplicator[T]) IsDuplicate(ctx context.Context, groupJID, senderJID, content string, timestamp time.Time) bool {
	// Try in-memory cache first (fast path)
	if d.cfg.UseInMemoryCache {
		if d.checkCache(groupJID, senderJID, content, timestamp) {
			metrics.DeduplicatorHits.Inc()
			d.hits.Add(1)
			return true
		}
	}

	// Fall back to database lookup (slow path)
	if d.lookup != nil {
		lastMsg, err := d.lookup.GetLast(ctx, groupJID, senderJID)
		if err != nil {
			d.log.Debug().Err(err).Msg("Failed to lookup last message")
			return false
		}

		// Check if lastMsg is non-nil using reflection (needed for pointer types)
		if !isNil(lastMsg) && d.isDuplicateOf(lastMsg, content, timestamp) {
			metrics.DeduplicatorHits.Inc()
			d.hits.Add(1)
			return true
		}
	}

	metrics.DeduplicatorMisses.Inc()
	d.misses.Add(1)
	return false
}

// RecordMessage records a message in the cache for future deduplication checks.
func (d *Deduplicator[T]) RecordMessage(groupJID, senderJID, content string, timestamp time.Time) {
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
		d.evictOne()
	}

	d.cache[key] = entry
}

// checkCache checks the in-memory cache for duplicates.
func (d *Deduplicator[T]) checkCache(groupJID, senderJID, content string, timestamp time.Time) bool {
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
func (d *Deduplicator[T]) isDuplicateOf(lastMsg T, content string, timestamp time.Time) bool {
	timeDiff := absDuration(timestamp.Sub(lastMsg.GetTimestamp()))
	return timeDiff < d.cfg.Window && lastMsg.GetContent() == content
}

// evictOne removes one entry from the cache to make room.
// Uses random eviction for O(1) performance.
// Caller must hold the write lock.
func (d *Deduplicator[T]) evictOne() {
	for key := range d.cache {
		delete(d.cache, key)
		return
	}
}

// cleanupLoop periodically removes expired entries from the cache.
func (d *Deduplicator[T]) cleanupLoop(ctx context.Context) {
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
func (d *Deduplicator[T]) cleanup() {
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
func (d *Deduplicator[T]) Stats() DeduplicatorStats {
	d.cacheMu.RLock()
	cacheSize := len(d.cache)
	d.cacheMu.RUnlock()

	hits := d.hits.Load()
	misses := d.misses.Load()

	return DeduplicatorStats{
		Hits:      hits,
		Misses:    misses,
		CacheSize: cacheSize,
		HitRate:   calculateHitRate(hits, misses),
	}
}

// calculateHitRate returns the cache hit rate as a percentage.
func calculateHitRate(hits, misses int64) float64 {
	total := hits + misses
	if total == 0 {
		return 0
	}
	return float64(hits) / float64(total) * 100
}

// DeduplicatorStats holds deduplication statistics.
type DeduplicatorStats struct {
	Hits      int64   `json:"hits"`
	Misses    int64   `json:"misses"`
	CacheSize int     `json:"cache_size"`
	HitRate   float64 `json:"hit_rate_pct"`
}

// Clear clears the in-memory cache and resets stats.
func (d *Deduplicator[T]) Clear() {
	if d.cfg.UseInMemoryCache {
		d.cacheMu.Lock()
		d.cache = make(map[string]cacheEntry, d.cfg.CacheSize)
		d.cacheMu.Unlock()
	}
	d.hits.Store(0)
	d.misses.Store(0)
}
