package sse

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestNewSSEHub(t *testing.T) {
	hub := NewSSEHub()
	defer hub.Shutdown()

	if hub.maxClients != DefaultMaxClients {
		t.Errorf("expected maxClients=%d, got %d", DefaultMaxClients, hub.maxClients)
	}
	if hub.heartbeatInterval != HeartbeatInterval {
		t.Errorf("expected heartbeatInterval=%v, got %v", HeartbeatInterval, hub.heartbeatInterval)
	}
}

func TestNewSSEHubWithLimit(t *testing.T) {
	hub := NewSSEHubWithLimit(50)
	defer hub.Shutdown()

	if hub.maxClients != 50 {
		t.Errorf("expected maxClients=50, got %d", hub.maxClients)
	}
}

func TestNewSSEHubWithOptions(t *testing.T) {
	hub := NewSSEHubWithOptions(25, 5*time.Second)
	defer hub.Shutdown()

	if hub.maxClients != 25 {
		t.Errorf("expected maxClients=25, got %d", hub.maxClients)
	}
	if hub.heartbeatInterval != 5*time.Second {
		t.Errorf("expected heartbeatInterval=5s, got %v", hub.heartbeatInterval)
	}
}

func TestNewSSEHubWithOptions_InvalidValues(t *testing.T) {
	hub := NewSSEHubWithOptions(-1, -1*time.Second)
	defer hub.Shutdown()

	if hub.maxClients != DefaultMaxClients {
		t.Errorf("expected default maxClients=%d for invalid input, got %d", DefaultMaxClients, hub.maxClients)
	}
	if hub.heartbeatInterval != HeartbeatInterval {
		t.Errorf("expected default heartbeatInterval=%v for invalid input, got %v", HeartbeatInterval, hub.heartbeatInterval)
	}
}

func TestClientCount(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour) // Long heartbeat to avoid interference
	defer hub.Shutdown()

	if hub.ClientCount() != 0 {
		t.Errorf("expected 0 clients initially, got %d", hub.ClientCount())
	}
}

func TestBroadcast(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	// Register a client manually for testing
	client := make(chan SSEEvent, ClientBufferSize)
	hub.mu.Lock()
	hub.clients[client] = true
	hub.mu.Unlock()

	// Broadcast an event
	hub.Broadcast(SSEEvent{Type: "test", Data: "hello"})

	// Wait for event
	select {
	case event := <-client:
		if event.Type != "test" {
			t.Errorf("expected event type 'test', got '%s'", event.Type)
		}
		if event.Data != "hello" {
			t.Errorf("expected event data 'hello', got '%v'", event.Data)
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for broadcast event")
	}

	// Cleanup
	hub.mu.Lock()
	delete(hub.clients, client)
	close(client)
	hub.mu.Unlock()
}

func TestBroadcastWithTimeout_Success(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	success := hub.BroadcastWithTimeout(SSEEvent{Type: "test", Data: "data"}, time.Second)
	if !success {
		t.Error("expected BroadcastWithTimeout to succeed")
	}
}

func TestBroadcastWithTimeout_Timeout(t *testing.T) {
	// Create hub manually without starting run() goroutine to control the buffer
	hub := &SSEHub{
		clients:           make(map[chan SSEEvent]bool),
		broadcast:         make(chan SSEEvent, BroadcastBufferSize),
		done:              make(chan struct{}),
		maxClients:        10,
		heartbeatInterval: time.Hour,
	}
	defer close(hub.done)

	// Fill the broadcast buffer (no consumer running)
	for i := 0; i < BroadcastBufferSize; i++ {
		hub.broadcast <- SSEEvent{Type: "filler", Data: i}
	}

	// This should timeout since buffer is full and no one is consuming
	success := hub.BroadcastWithTimeout(SSEEvent{Type: "test", Data: "data"}, 10*time.Millisecond)
	if success {
		t.Error("expected BroadcastWithTimeout to timeout")
	}
}

func TestServeHTTP_Connection(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	// Create a request with a cancellable context
	ctx, cancel := context.WithCancel(context.Background())
	req := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx)
	rec := httptest.NewRecorder()

	// Run ServeHTTP in a goroutine
	done := make(chan struct{})
	go func() {
		hub.ServeHTTP(rec, req)
		close(done)
	}()

	// Wait for connection
	time.Sleep(50 * time.Millisecond)

	if hub.ClientCount() != 1 {
		t.Errorf("expected 1 client, got %d", hub.ClientCount())
	}

	// Cancel the request
	cancel()

	// Wait for handler to exit
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Error("handler did not exit after context cancellation")
	}

	// Client should be removed
	time.Sleep(50 * time.Millisecond)
	if hub.ClientCount() != 0 {
		t.Errorf("expected 0 clients after disconnect, got %d", hub.ClientCount())
	}
}

func TestServeHTTP_Headers(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	req := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx)
	rec := httptest.NewRecorder()

	go func() {
		hub.ServeHTTP(rec, req)
	}()

	time.Sleep(50 * time.Millisecond)
	cancel()

	// Check headers
	if rec.Header().Get("Content-Type") != "text/event-stream" {
		t.Errorf("expected Content-Type 'text/event-stream', got '%s'", rec.Header().Get("Content-Type"))
	}
	if rec.Header().Get("Cache-Control") != "no-cache" {
		t.Errorf("expected Cache-Control 'no-cache', got '%s'", rec.Header().Get("Cache-Control"))
	}
}

func TestServeHTTP_InitialEvent(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	ctx, cancel := context.WithCancel(context.Background())
	req := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx)
	rec := httptest.NewRecorder()

	go func() {
		hub.ServeHTTP(rec, req)
	}()

	time.Sleep(50 * time.Millisecond)
	cancel()
	time.Sleep(50 * time.Millisecond)

	body := rec.Body.String()
	if !strings.Contains(body, "event: connected") {
		t.Errorf("expected initial 'connected' event, got: %s", body)
	}
}

func TestServeHTTP_MaxClientsLimit(t *testing.T) {
	hub := NewSSEHubWithOptions(2, time.Hour)
	defer hub.Shutdown()

	ctx1, cancel1 := context.WithCancel(context.Background())
	ctx2, cancel2 := context.WithCancel(context.Background())
	defer cancel1()
	defer cancel2()

	// Connect 2 clients (max)
	req1 := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx1)
	rec1 := httptest.NewRecorder()
	go hub.ServeHTTP(rec1, req1)

	req2 := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx2)
	rec2 := httptest.NewRecorder()
	go hub.ServeHTTP(rec2, req2)

	time.Sleep(50 * time.Millisecond)

	// Third client should be rejected
	req3 := httptest.NewRequest(http.MethodGet, "/events", nil)
	rec3 := httptest.NewRecorder()
	hub.ServeHTTP(rec3, req3)

	if rec3.Code != http.StatusServiceUnavailable {
		t.Errorf("expected status %d for max clients, got %d", http.StatusServiceUnavailable, rec3.Code)
	}
}

func TestServeHTTP_ReceivesBroadcast(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	ctx, cancel := context.WithCancel(context.Background())
	req := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx)
	rec := httptest.NewRecorder()

	go func() {
		hub.ServeHTTP(rec, req)
	}()

	time.Sleep(50 * time.Millisecond)

	// Broadcast an event
	hub.Broadcast(SSEEvent{Type: "test_event", Data: map[string]string{"key": "value"}})

	time.Sleep(50 * time.Millisecond)
	cancel()
	time.Sleep(50 * time.Millisecond)

	body := rec.Body.String()
	if !strings.Contains(body, "event: test_event") {
		t.Errorf("expected 'test_event' in body, got: %s", body)
	}
}

func TestShutdown(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	req := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx)
	rec := httptest.NewRecorder()

	done := make(chan struct{})
	go func() {
		hub.ServeHTTP(rec, req)
		close(done)
	}()

	time.Sleep(50 * time.Millisecond)

	// Shutdown should close all connections
	hub.Shutdown()

	select {
	case <-done:
	case <-time.After(time.Second):
		t.Error("handler did not exit after shutdown")
	}
}

func TestConcurrentConnections(t *testing.T) {
	hub := NewSSEHubWithOptions(50, time.Hour)
	defer hub.Shutdown()

	var wg sync.WaitGroup
	contexts := make([]context.CancelFunc, 20)

	for i := 0; i < 20; i++ {
		wg.Add(1)
		ctx, cancel := context.WithCancel(context.Background())
		contexts[i] = cancel

		go func() {
			defer wg.Done()
			req := httptest.NewRequest(http.MethodGet, "/events", nil).WithContext(ctx)
			rec := httptest.NewRecorder()
			hub.ServeHTTP(rec, req)
		}()
	}

	time.Sleep(100 * time.Millisecond)

	if hub.ClientCount() != 20 {
		t.Errorf("expected 20 clients, got %d", hub.ClientCount())
	}

	// Cancel all
	for _, cancel := range contexts {
		cancel()
	}

	wg.Wait()

	if hub.ClientCount() != 0 {
		t.Errorf("expected 0 clients after all disconnected, got %d", hub.ClientCount())
	}
}

func TestBroadcastNewOffer(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	client := make(chan SSEEvent, ClientBufferSize)
	hub.mu.Lock()
	hub.clients[client] = true
	hub.mu.Unlock()

	hub.BroadcastNewOffer("offer-123", "Aspirin")

	select {
	case event := <-client:
		if event.Type != "new_offer" {
			t.Errorf("expected type 'new_offer', got '%s'", event.Type)
		}
		data := event.Data.(map[string]string)
		if data["id"] != "offer-123" {
			t.Errorf("expected id 'offer-123', got '%s'", data["id"])
		}
		if data["medication"] != "Aspirin" {
			t.Errorf("expected medication 'Aspirin', got '%s'", data["medication"])
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for event")
	}

	hub.mu.Lock()
	delete(hub.clients, client)
	close(client)
	hub.mu.Unlock()
}

func TestBroadcastNewRequest(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	client := make(chan SSEEvent, ClientBufferSize)
	hub.mu.Lock()
	hub.clients[client] = true
	hub.mu.Unlock()

	hub.BroadcastNewRequest("req-456", "Ibuprofen")

	select {
	case event := <-client:
		if event.Type != "new_request" {
			t.Errorf("expected type 'new_request', got '%s'", event.Type)
		}
		data := event.Data.(map[string]string)
		if data["id"] != "req-456" {
			t.Errorf("expected id 'req-456', got '%s'", data["id"])
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for event")
	}

	hub.mu.Lock()
	delete(hub.clients, client)
	close(client)
	hub.mu.Unlock()
}

func TestBroadcastNewMatch(t *testing.T) {
	hub := NewSSEHubWithOptions(10, time.Hour)
	defer hub.Shutdown()

	client := make(chan SSEEvent, ClientBufferSize)
	hub.mu.Lock()
	hub.clients[client] = true
	hub.mu.Unlock()

	hub.BroadcastNewMatch("match-789", 0.95)

	select {
	case event := <-client:
		if event.Type != "new_match" {
			t.Errorf("expected type 'new_match', got '%s'", event.Type)
		}
		data := event.Data.(map[string]any)
		if data["id"] != "match-789" {
			t.Errorf("expected id 'match-789', got '%v'", data["id"])
		}
		if data["score"] != 0.95 {
			t.Errorf("expected score 0.95, got '%v'", data["score"])
		}
	case <-time.After(time.Second):
		t.Error("timeout waiting for event")
	}

	hub.mu.Lock()
	delete(hub.clients, client)
	close(client)
	hub.mu.Unlock()
}

func TestHeartbeat(t *testing.T) {
	// Use a short heartbeat interval for testing
	hub := NewSSEHubWithOptions(10, 100*time.Millisecond)
	defer hub.Shutdown()

	client := make(chan SSEEvent, ClientBufferSize)
	hub.mu.Lock()
	hub.clients[client] = true
	hub.mu.Unlock()

	// Wait for heartbeat
	select {
	case event := <-client:
		if event.Type != "heartbeat" {
			t.Errorf("expected heartbeat event, got '%s'", event.Type)
		}
	case <-time.After(500 * time.Millisecond):
		t.Error("timeout waiting for heartbeat")
	}

	hub.mu.Lock()
	delete(hub.clients, client)
	close(client)
	hub.mu.Unlock()
}
