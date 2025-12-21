package cache

import (
	"sync"
	"time"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
)

// GroupCache stores monitored group JIDs with a TTL.
// Implements ports.GroupCache interface.
type GroupCache struct {
	mu           sync.RWMutex
	jids         map[domain.JID]struct{}
	lastSync     time.Time
	syncInterval time.Duration
}

// NewGroupCache creates a new group cache with the specified sync interval.
func NewGroupCache(syncInterval time.Duration) *GroupCache {
	return &GroupCache{
		jids:         make(map[domain.JID]struct{}),
		syncInterval: syncInterval,
	}
}

// Ensure GroupCache implements the interface
var _ ports.GroupCache = (*GroupCache)(nil)

// IsMonitored checks if a group JID is in the cache.
func (c *GroupCache) IsMonitored(jid domain.JID) bool {
	c.mu.RLock()
	defer c.mu.RUnlock()

	// If cache is empty and never synced, we might want to return true to be safe,
	// but the requirement is to ignore if not monitored after sync.
	// For now, if empty, we assume no groups are monitored yet.
	_, exists := c.jids[jid]
	return exists
}

// Update sets the monitored JIDs and updates the last sync time.
func (c *GroupCache) Update(jids []domain.JID) {
	c.mu.Lock()
	defer c.mu.Unlock()

	newJids := make(map[domain.JID]struct{}, len(jids))
	for _, jid := range jids {
		newJids[jid] = struct{}{}
	}
	c.jids = newJids
	c.lastSync = time.Now()
}

// IsStale returns true if the cache has not been synced within the sync interval.
func (c *GroupCache) IsStale() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return time.Since(c.lastSync) > c.syncInterval
}

// LastSync returns the last successful sync time.
func (c *GroupCache) LastSync() time.Time {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.lastSync
}
