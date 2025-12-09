package sse

import (
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"
)

// SSEHub manages Server-Sent Events connections
type SSEHub struct {
	clients    map[chan SSEEvent]bool
	mu         sync.RWMutex
	register   chan chan SSEEvent
	unregister chan chan SSEEvent
	broadcast  chan SSEEvent
	maxClients int // Maximum concurrent SSE connections
}

// SSEEvent represents an event to send to clients
type SSEEvent struct {
	Type string      `json:"type"`
	Data interface{} `json:"data"`
}

// NewSSEHub creates a new SSE hub
func NewSSEHub() *SSEHub {
	return NewSSEHubWithLimit(100) // Default max 100 clients
}

// NewSSEHubWithLimit creates a new SSE hub with a custom client limit
func NewSSEHubWithLimit(maxClients int) *SSEHub {
	if maxClients <= 0 {
		maxClients = 100
	}
	hub := &SSEHub{
		clients:    make(map[chan SSEEvent]bool),
		register:   make(chan chan SSEEvent),
		unregister: make(chan chan SSEEvent),
		broadcast:  make(chan SSEEvent, 100),
		maxClients: maxClients,
	}
	go hub.run()
	return hub
}

func (h *SSEHub) run() {
	// Heartbeat ticker
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case client := <-h.register:
			h.mu.Lock()
			h.clients[client] = true
			h.mu.Unlock()

		case client := <-h.unregister:
			h.mu.Lock()
			if _, ok := h.clients[client]; ok {
				delete(h.clients, client)
				close(client)
			}
			h.mu.Unlock()

		case event := <-h.broadcast:
			h.mu.RLock()
			for client := range h.clients {
				select {
				case client <- event:
				default:
					// Client buffer full, skip
				}
			}
			h.mu.RUnlock()

		case <-ticker.C:
			// Send heartbeat to all clients
			h.mu.RLock()
			for client := range h.clients {
				select {
				case client <- SSEEvent{Type: "heartbeat", Data: time.Now().Unix()}:
				default:
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
		// Buffer full, drop event
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
	// Check connection limit
	h.mu.RLock()
	clientCount := len(h.clients)
	h.mu.RUnlock()

	if clientCount >= h.maxClients {
		http.Error(w, "Too many SSE connections", http.StatusServiceUnavailable)
		return
	}

	// Set SSE headers
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.Header().Set("Access-Control-Allow-Origin", "*")

	// Create client channel
	client := make(chan SSEEvent, 10)
	h.register <- client

	// Ensure cleanup on disconnect
	defer func() {
		h.unregister <- client
	}()

	// Get flusher
	flusher, ok := w.(http.Flusher)
	if !ok {
		http.Error(w, "SSE not supported", http.StatusInternalServerError)
		return
	}

	// Send initial connection event
	fmt.Fprintf(w, "event: connected\ndata: {\"status\":\"connected\"}\n\n")
	flusher.Flush()

	// Stream events
	for {
		select {
		case <-r.Context().Done():
			return
		case event, ok := <-client:
			if !ok {
				return
			}

			data, err := json.Marshal(event.Data)
			if err != nil {
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
		Data: map[string]interface{}{
			"id":    matchID,
			"score": score,
		},
	})
}
