package cache

import (
	"testing"
	"time"
)

func TestGroupCache(t *testing.T) {
	interval := 100 * time.Millisecond
	cache := NewGroupCache(interval)

	// Initial state
	if cache.IsMonitored("group1") {
		t.Error("Initial cache should not contain group1")
	}

	// Update cache
	cache.Update([]string{"group1", "group2"})

	if !cache.IsMonitored("group1") {
		t.Error("Cache should contain group1")
	}
	if !cache.IsMonitored("group2") {
		t.Error("Cache should contain group2")
	}
	if cache.IsMonitored("group3") {
		t.Error("Cache should not contain group3")
	}

	// Stale check
	if cache.IsStale() {
		t.Error("Cache should not be stale immediately after update")
	}

	time.Sleep(150 * time.Millisecond)

	if !cache.IsStale() {
		t.Error("Cache should be stale after interval")
	}

	// Update again
	cache.Update([]string{"group3"})
	if cache.IsMonitored("group1") {
		t.Error("Cache should no longer contain group1")
	}
	if !cache.IsMonitored("group3") {
		t.Error("Cache should contain group3")
	}
	if cache.IsStale() {
		t.Error("Cache should no longer be stale after update")
	}
}
