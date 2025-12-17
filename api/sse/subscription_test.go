package sse

import (
	"testing"

	"github.com/rs/zerolog"
)

// =============================================================================
// Subscription Tests
// =============================================================================

func TestNewSubscription(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")

	if sub.ClientID != "client-1" {
		t.Errorf("ClientID = %s, want client-1", sub.ClientID)
	}
	if sub.UserID != "user-1" {
		t.Errorf("UserID = %s, want user-1", sub.UserID)
	}
}

func TestSubscription_SubscribeToEvents(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")

	sub.SubscribeToEvents("new_offer", "new_match")

	if !sub.EventTypes["new_offer"] {
		t.Error("Should be subscribed to new_offer")
	}
	if !sub.EventTypes["new_match"] {
		t.Error("Should be subscribed to new_match")
	}
}

func TestSubscription_SubscribeToGroups(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")

	sub.SubscribeToGroups("group-A", "group-B")

	if !sub.GroupIDs["group-A"] {
		t.Error("Should be subscribed to group-A")
	}
	if !sub.GroupIDs["group-B"] {
		t.Error("Should be subscribed to group-B")
	}
}

func TestSubscription_UnsubscribeFromEvents(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")

	sub.SubscribeToEvents("new_offer", "new_match")
	sub.UnsubscribeFromEvents("new_offer")

	if sub.EventTypes["new_offer"] {
		t.Error("Should not be subscribed to new_offer after unsubscribe")
	}
	if !sub.EventTypes["new_match"] {
		t.Error("Should still be subscribed to new_match")
	}
}

func TestSubscription_UnsubscribeFromGroups(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")

	sub.SubscribeToGroups("group-A", "group-B")
	sub.UnsubscribeFromGroups("group-A")

	if sub.GroupIDs["group-A"] {
		t.Error("Should not be subscribed to group-A after unsubscribe")
	}
	if !sub.GroupIDs["group-B"] {
		t.Error("Should still be subscribed to group-B")
	}
}

func TestSubscription_ShouldReceive_NoFilters(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")

	// No filters = receive all
	event := SSEEvent{Type: "any_event", Data: nil}
	if !sub.ShouldReceive(event, "") {
		t.Error("Should receive event with no filters")
	}
}

func TestSubscription_ShouldReceive_EventTypeFilter(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")
	sub.SubscribeToEvents("new_offer")

	// Should receive subscribed event
	if !sub.ShouldReceive(SSEEvent{Type: "new_offer"}, "") {
		t.Error("Should receive new_offer event")
	}

	// Should not receive unsubscribed event
	if sub.ShouldReceive(SSEEvent{Type: "new_match"}, "") {
		t.Error("Should not receive new_match event")
	}
}

func TestSubscription_ShouldReceive_GroupFilter(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")
	sub.SubscribeToGroups("group-A")

	// Should receive event for subscribed group
	if !sub.ShouldReceive(SSEEvent{Type: "test"}, "group-A") {
		t.Error("Should receive event for group-A")
	}

	// Should not receive event for unsubscribed group
	if sub.ShouldReceive(SSEEvent{Type: "test"}, "group-B") {
		t.Error("Should not receive event for group-B")
	}
}

func TestSubscription_ShouldReceive_CombinedFilters(t *testing.T) {
	clientChan := make(chan SSEEvent, 10)
	sub := NewSubscription(clientChan, "client-1", "user-1")
	sub.SubscribeToEvents("new_offer")
	sub.SubscribeToGroups("group-A")

	// Should receive: correct event type AND correct group
	if !sub.ShouldReceive(SSEEvent{Type: "new_offer"}, "group-A") {
		t.Error("Should receive new_offer for group-A")
	}

	// Should not receive: wrong event type
	if sub.ShouldReceive(SSEEvent{Type: "new_match"}, "group-A") {
		t.Error("Should not receive new_match")
	}

	// Should not receive: wrong group
	if sub.ShouldReceive(SSEEvent{Type: "new_offer"}, "group-B") {
		t.Error("Should not receive for group-B")
	}
}

// =============================================================================
// SubscriptionManager Tests
// =============================================================================

func TestNewSubscriptionManager(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	if manager == nil {
		t.Fatal("NewSubscriptionManager returned nil")
	}
}

func TestSubscriptionManager_Subscribe(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	clientChan := make(chan SSEEvent, 10)
	sub := manager.Subscribe(clientChan, "client-1", "user-1", []string{"new_offer"}, []string{"group-A"})

	if sub == nil {
		t.Fatal("Subscribe returned nil")
	}
	if manager.GetSubscriberCount() != 1 {
		t.Errorf("GetSubscriberCount() = %d, want 1", manager.GetSubscriberCount())
	}
}

func TestSubscriptionManager_Unsubscribe(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	clientChan := make(chan SSEEvent, 10)
	manager.Subscribe(clientChan, "client-1", "user-1", nil, nil)
	manager.Unsubscribe(clientChan)

	if manager.GetSubscriberCount() != 0 {
		t.Errorf("GetSubscriberCount() after unsubscribe = %d, want 0", manager.GetSubscriberCount())
	}
}

func TestSubscriptionManager_GetSubscription(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	clientChan := make(chan SSEEvent, 10)
	manager.Subscribe(clientChan, "client-1", "user-1", nil, nil)

	sub := manager.GetSubscription(clientChan)
	if sub == nil {
		t.Fatal("GetSubscription returned nil")
	}
	if sub.ClientID != "client-1" {
		t.Errorf("ClientID = %s, want client-1", sub.ClientID)
	}
}

func TestSubscriptionManager_UpdateSubscription(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	clientChan := make(chan SSEEvent, 10)
	manager.Subscribe(clientChan, "client-1", "user-1", []string{"old_event"}, nil)

	manager.UpdateSubscription(clientChan, []string{"new_event"}, []string{"group-A"})

	sub := manager.GetSubscription(clientChan)
	if sub.EventTypes["old_event"] {
		t.Error("Should not have old_event after update")
	}
	if !sub.EventTypes["new_event"] {
		t.Error("Should have new_event after update")
	}
	if !sub.GroupIDs["group-A"] {
		t.Error("Should have group-A after update")
	}
}

func TestSubscriptionManager_BroadcastFiltered(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	// Create two clients with different subscriptions
	client1 := make(chan SSEEvent, 10)
	client2 := make(chan SSEEvent, 10)

	manager.Subscribe(client1, "client-1", "user-1", []string{"new_offer"}, nil)
	manager.Subscribe(client2, "client-2", "user-2", []string{"new_match"}, nil)

	// Broadcast new_offer - only client1 should receive
	sent := manager.BroadcastFiltered(SSEEvent{Type: "new_offer", Data: "test"}, "")

	if sent != 1 {
		t.Errorf("BroadcastFiltered sent to %d clients, want 1", sent)
	}

	// Check client1 received
	select {
	case <-client1:
		// Good
	default:
		t.Error("client1 should have received the event")
	}

	// Check client2 did not receive
	select {
	case <-client2:
		t.Error("client2 should not have received the event")
	default:
		// Good
	}
}

func TestSubscriptionManager_GetSubscribersForEvent(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	client1 := make(chan SSEEvent, 10)
	client2 := make(chan SSEEvent, 10)
	client3 := make(chan SSEEvent, 10)

	manager.Subscribe(client1, "client-1", "user-1", []string{"new_offer"}, nil)
	manager.Subscribe(client2, "client-2", "user-2", []string{"new_offer", "new_match"}, nil)
	manager.Subscribe(client3, "client-3", "user-3", nil, nil) // All events

	count := manager.GetSubscribersForEvent("new_offer")
	if count != 3 {
		t.Errorf("GetSubscribersForEvent(new_offer) = %d, want 3", count)
	}

	count = manager.GetSubscribersForEvent("new_match")
	if count != 2 {
		t.Errorf("GetSubscribersForEvent(new_match) = %d, want 2", count)
	}
}

func TestSubscriptionManager_GetSubscribersForGroup(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	client1 := make(chan SSEEvent, 10)
	client2 := make(chan SSEEvent, 10)

	manager.Subscribe(client1, "client-1", "user-1", nil, []string{"group-A"})
	manager.Subscribe(client2, "client-2", "user-2", nil, nil) // All groups

	count := manager.GetSubscribersForGroup("group-A")
	if count != 2 {
		t.Errorf("GetSubscribersForGroup(group-A) = %d, want 2", count)
	}

	count = manager.GetSubscribersForGroup("group-B")
	if count != 1 {
		t.Errorf("GetSubscribersForGroup(group-B) = %d, want 1 (only client2)", count)
	}
}

func TestSubscriptionManager_GetAllSubscriptions(t *testing.T) {
	log := zerolog.Nop()
	manager := NewSubscriptionManager(log)

	client1 := make(chan SSEEvent, 10)
	client2 := make(chan SSEEvent, 10)

	manager.Subscribe(client1, "client-1", "user-1", nil, nil)
	manager.Subscribe(client2, "client-2", "user-2", nil, nil)

	subs := manager.GetAllSubscriptions()
	if len(subs) != 2 {
		t.Errorf("GetAllSubscriptions() returned %d, want 2", len(subs))
	}
}
