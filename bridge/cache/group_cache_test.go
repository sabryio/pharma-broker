package cache

import (
	"testing"
	"time"

	"pharma-bridge/domain"
)

func TestGroupCache(t *testing.T) {
	cache := NewGroupCache(5 * time.Minute)

	// Empty cache
	if cache.IsMonitored("group1@g.us") {
		t.Error("Empty cache should not have any monitored groups")
	}

	// Update cache
	cache.Update([]domain.JID{"group1@g.us", "group2@g.us"})

	if !cache.IsMonitored("group1@g.us") {
		t.Error("group1 should be monitored")
	}
	if !cache.IsMonitored("group2@g.us") {
		t.Error("group2 should be monitored")
	}
	if cache.IsMonitored("group3@g.us") {
		t.Error("group3 should not be monitored")
	}

	// Update replaces all
	cache.Update([]domain.JID{"group3@g.us"})

	if cache.IsMonitored("group1@g.us") {
		t.Error("group1 should no longer be monitored")
	}
	if !cache.IsMonitored("group3@g.us") {
		t.Error("group3 should be monitored")
	}

	// Check stale
	if cache.IsStale() {
		t.Error("Cache should not be stale immediately after update")
	}

	// Check last sync
	if cache.LastSync().IsZero() {
		t.Error("LastSync should not be zero after update")
	}
}

func TestGroupCache_Stale(t *testing.T) {
	cache := NewGroupCache(50 * time.Millisecond)
	cache.Update([]domain.JID{"group1@g.us"})

	if cache.IsStale() {
		t.Error("Cache should not be stale immediately")
	}

	time.Sleep(100 * time.Millisecond)

	if !cache.IsStale() {
		t.Error("Cache should be stale after interval")
	}
}
