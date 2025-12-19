package sse

import (
	"sync"

	"github.com/rs/zerolog"
)

// =============================================================================
// Subscription
// =============================================================================

// Subscription represents a client's event subscription.
type Subscription struct {
	ClientChan chan SSEEvent
	EventTypes map[string]bool // Subscribed event types (empty = all)
	GroupIDs   map[string]bool // Subscribed groups (empty = all)
	UserID     string          // Authenticated user ID
	ClientID   string          // Unique client identifier
}

// NewSubscription creates a new subscription.
func NewSubscription(clientChan chan SSEEvent, clientID, userID string) *Subscription {
	return &Subscription{
		ClientChan: clientChan,
		EventTypes: make(map[string]bool),
		GroupIDs:   make(map[string]bool),
		UserID:     userID,
		ClientID:   clientID,
	}
}

// SubscribeToEvents subscribes to specific event types.
func (s *Subscription) SubscribeToEvents(eventTypes ...string) {
	for _, t := range eventTypes {
		s.EventTypes[t] = true
	}
}

// SubscribeToGroups subscribes to specific groups.
func (s *Subscription) SubscribeToGroups(groupIDs ...string) {
	for _, g := range groupIDs {
		s.GroupIDs[g] = true
	}
}

// UnsubscribeFromEvents unsubscribes from specific event types.
func (s *Subscription) UnsubscribeFromEvents(eventTypes ...string) {
	for _, t := range eventTypes {
		delete(s.EventTypes, t)
	}
}

// UnsubscribeFromGroups unsubscribes from specific groups.
func (s *Subscription) UnsubscribeFromGroups(groupIDs ...string) {
	for _, g := range groupIDs {
		delete(s.GroupIDs, g)
	}
}

// ShouldReceive checks if this subscription should receive the event.
func (s *Subscription) ShouldReceive(event SSEEvent, groupID string) bool {
	// Check event type filter
	if len(s.EventTypes) > 0 && !s.EventTypes[event.Type] {
		return false
	}

	// Check group filter (if groupID provided)
	if groupID != "" && len(s.GroupIDs) > 0 && !s.GroupIDs[groupID] {
		return false
	}

	return true
}

// =============================================================================
// Subscription Manager
// =============================================================================

// SubscriptionManager manages client subscriptions with filtering.
type SubscriptionManager struct {
	subscriptions map[chan SSEEvent]*Subscription
	log           zerolog.Logger
	mu            sync.RWMutex
}

// NewSubscriptionManager creates a new subscription manager.
func NewSubscriptionManager(log zerolog.Logger) *SubscriptionManager {
	return &SubscriptionManager{
		subscriptions: make(map[chan SSEEvent]*Subscription),
		log:           log.With().Str("component", "subscription-mgr").Logger(),
	}
}

// Subscribe creates a new subscription for a client.
func (m *SubscriptionManager) Subscribe(clientChan chan SSEEvent, clientID, userID string, eventTypes, groupIDs []string) *Subscription {
	m.mu.Lock()
	defer m.mu.Unlock()

	sub := NewSubscription(clientChan, clientID, userID)
	sub.SubscribeToEvents(eventTypes...)
	sub.SubscribeToGroups(groupIDs...)

	m.subscriptions[clientChan] = sub

	m.log.Debug().
		Str("client_id", clientID).
		Str("user_id", userID).
		Int("event_types", len(eventTypes)).
		Int("groups", len(groupIDs)).
		Msg("Client subscribed")

	return sub
}

// Unsubscribe removes a client's subscription.
func (m *SubscriptionManager) Unsubscribe(clientChan chan SSEEvent) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if sub, ok := m.subscriptions[clientChan]; ok {
		m.log.Debug().
			Str("client_id", sub.ClientID).
			Msg("Client unsubscribed")
		delete(m.subscriptions, clientChan)
	}
}

// GetSubscription returns a client's subscription.
func (m *SubscriptionManager) GetSubscription(clientChan chan SSEEvent) *Subscription {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.subscriptions[clientChan]
}

// UpdateSubscription updates a client's subscription.
func (m *SubscriptionManager) UpdateSubscription(clientChan chan SSEEvent, eventTypes, groupIDs []string) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if sub, ok := m.subscriptions[clientChan]; ok {
		// Clear existing and set new
		sub.EventTypes = make(map[string]bool)
		sub.GroupIDs = make(map[string]bool)
		sub.SubscribeToEvents(eventTypes...)
		sub.SubscribeToGroups(groupIDs...)
	}
}

// BroadcastFiltered sends an event only to subscribed clients.
func (m *SubscriptionManager) BroadcastFiltered(event SSEEvent, groupID string) int {
	m.mu.RLock()
	defer m.mu.RUnlock()

	sent := 0
	for _, sub := range m.subscriptions {
		if sub.ShouldReceive(event, groupID) {
			select {
			case sub.ClientChan <- event:
				sent++
			default:
				m.log.Debug().
					Str("client_id", sub.ClientID).
					Str("event_type", event.Type).
					Msg("Event skipped for slow client")
			}
		}
	}

	return sent
}

// GetSubscriberCount returns the number of subscribers.
func (m *SubscriptionManager) GetSubscriberCount() int {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return len(m.subscriptions)
}

// GetSubscribersForEvent returns count of subscribers for an event type.
func (m *SubscriptionManager) GetSubscribersForEvent(eventType string) int {
	m.mu.RLock()
	defer m.mu.RUnlock()

	count := 0
	for _, sub := range m.subscriptions {
		// Empty filter means subscribed to all
		if len(sub.EventTypes) == 0 || sub.EventTypes[eventType] {
			count++
		}
	}
	return count
}

// GetSubscribersForGroup returns count of subscribers for a group.
func (m *SubscriptionManager) GetSubscribersForGroup(groupID string) int {
	m.mu.RLock()
	defer m.mu.RUnlock()

	count := 0
	for _, sub := range m.subscriptions {
		// Empty filter means subscribed to all
		if len(sub.GroupIDs) == 0 || sub.GroupIDs[groupID] {
			count++
		}
	}
	return count
}

// GetAllSubscriptions returns all subscriptions (for debugging).
func (m *SubscriptionManager) GetAllSubscriptions() []*Subscription {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var subs []*Subscription
	for _, sub := range m.subscriptions {
		subs = append(subs, sub)
	}
	return subs
}
