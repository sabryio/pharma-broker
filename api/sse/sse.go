package sse

import (
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/rs/zerolog/log"
)

// Constants for SSE configuration
const (
	DefaultMaxClients   = 100
	BroadcastBufferSize = 100
	ClientBufferSize    = 10
	HeartbeatInterval   = 30 * time.Second
)

// SSEHub manages Server-Sent Events connections
type SSEHub struct {
	clients           map[chan SSEEvent]bool
	mu                sync.RWMutex
	broadcast         chan SSEEvent
	done              chan struct{}
	maxClients        int
	heartbeatInterval time.Duration
}

// SSEEvent represents an event to send to clients
type SSEEvent struct {
	Type string `json:"type"`
	Data any    `json:"data"`
}

// NewSSEHub creates a new SSE hub with default settings
func NewSSEHub() *SSEHub {
	return NewSSEHubWithLimit(DefaultMaxClients)
}

// NewSSEHubWithLimit creates a new SSE hub with a custom client limit
func NewSSEHubWithLimit(maxClients int) *SSEHub {
	return NewSSEHubWithOptions(maxClients, HeartbeatInterval)
}

// NewSSEHubWithOptions creates a new SSE hub with custom settings
func NewSSEHubWithOptions(maxClients int, heartbeatInterval time.Duration) *SSEHub {
	if maxClients <= 0 {
		maxClients = DefaultMaxClients
	}
	if heartbeatInterval <= 0 {
		heartbeatInterval = HeartbeatInterval
	}

	hub := &SSEHub{
		clients:           make(map[chan SSEEvent]bool),
		broadcast:         make(chan SSEEvent, BroadcastBufferSize),
		done:              make(chan struct{}),
		maxClients:        maxClients,
		heartbeatInterval: heartbeatInterval,
	}
	go hub.run()
	return hub
}

// Shutdown gracefully stops the SSE hub and closes all client connections
func (h *SSEHub) Shutdown() {
	close(h.done)
}

func (h *SSEHub) run() {
	ticker := time.NewTicker(h.heartbeatInterval)
	defer ticker.Stop()

	for {
		select {
		case <-h.done:
			h.mu.Lock()
			for client := range h.clients {
				close(client)
			}
			h.clients = nil
			h.mu.Unlock()
			log.Info().Msg("SSE hub shutdown complete")
			return

		case event := <-h.broadcast:
			h.mu.RLock()
			for client := range h.clients {
				select {
				case client <- event:
				default:
					log.Warn().Str("event_type", event.Type).Msg("SSE client buffer full, event skipped")
				}
			}
			h.mu.RUnlock()

		case <-ticker.C:
			h.mu.RLock()
			for client := range h.clients {
				select {
				case client <- SSEEvent{Type: "heartbeat", Data: time.Now().Unix()}:
				default:
					log.Debug().Msg("SSE heartbeat skipped for slow client")
				}
			}
			h.mu.RUnlock()
		}
	}
}

// Broadcast sends an event to all connected clients
func (h *SSEHub) Broadcast(event SSEEvent) {
	select {
	case h.broadcast <- event:
	default:
		log.Warn().Str("event_type", event.Type).Msg("SSE broadcast buffer full, event dropped")
	}
}

// BroadcastWithTimeout sends an event with a timeout, returns false if timed out
func (h *SSEHub) BroadcastWithTimeout(event SSEEvent, timeout time.Duration) bool {
	select {
	case h.broadcast <- event:
		return true
	case <-time.After(timeout):
		log.Warn().Str("event_type", event.Type).Dur("timeout", timeout).Msg("SSE broadcast timed out")
		return false
	}
}

// ClientCount returns the number of connected clients
func (h *SSEHub) ClientCount() int {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return len(h.clients)
}

// ServeHTTP handles SSE connections
func (h *SSEHub) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Check flusher support before registration to avoid resource leak
	flusher, ok := w.(http.Flusher)
	if !ok {
		http.Error(w, "SSE not supported", http.StatusInternalServerError)
		return
	}

	// Create client channel
	client := make(chan SSEEvent, ClientBufferSize)

	// Atomic registration with limit check to prevent race condition
	h.mu.Lock()
	if len(h.clients) >= h.maxClients {
		h.mu.Unlock()
		http.Error(w, "Too many SSE connections", http.StatusServiceUnavailable)
		return
	}
	h.clients[client] = true
	h.mu.Unlock()

	// Ensure cleanup on disconnect
	defer func() {
		h.mu.Lock()
		if _, ok := h.clients[client]; ok {
			delete(h.clients, client)
			close(client)
		}
		h.mu.Unlock()
	}()

	// Set SSE headers
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.Header().Set("Access-Control-Allow-Origin", "*")

	// Send initial connection event
	fmt.Fprintf(w, "event: connected\ndata: {\"status\":\"connected\"}\n\n")
	flusher.Flush()

	log.Debug().Int("client_count", h.ClientCount()).Msg("SSE client connected")

	// Stream events
	for {
		select {
		case <-h.done:
			return
		case <-r.Context().Done():
			log.Debug().Int("client_count", h.ClientCount()-1).Msg("SSE client disconnected")
			return
		case event, ok := <-client:
			if !ok {
				return
			}

			data, err := json.Marshal(event.Data)
			if err != nil {
				log.Error().Err(err).Str("event_type", event.Type).Msg("Failed to marshal SSE event data")
				continue
			}

			fmt.Fprintf(w, "event: %s\ndata: %s\n\n", event.Type, data)
			flusher.Flush()
		}
	}
}

// BroadcastNewOffer sends a new offer event
func (h *SSEHub) BroadcastNewOffer(offerID string, medication string) {
	h.Broadcast(SSEEvent{
		Type: "new_offer",
		Data: map[string]string{
			"id":         offerID,
			"medication": medication,
		},
	})
}

// BroadcastNewRequest sends a new request event
func (h *SSEHub) BroadcastNewRequest(requestID string, medication string) {
	h.Broadcast(SSEEvent{
		Type: "new_request",
		Data: map[string]string{
			"id":         requestID,
			"medication": medication,
		},
	})
}

// BroadcastNewMatch sends a new match suggestion event
func (h *SSEHub) BroadcastNewMatch(matchID string, score float64) {
	h.Broadcast(SSEEvent{
		Type: "new_match",
		Data: map[string]any{
			"id":    matchID,
			"score": score,
		},
	})
}
