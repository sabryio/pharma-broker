package api

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
)

// Mock repositories for testing

type mockOfferRepo struct {
	offers []*domain.Offer
}

func (m *mockOfferRepo) Save(ctx context.Context, o *domain.Offer) error { return nil }
func (m *mockOfferRepo) GetByID(ctx context.Context, id string) (*domain.Offer, error) {
	for _, o := range m.offers {
		if o.ID == id {
			return o, nil
		}
	}
	return nil, nil
}
func (m *mockOfferRepo) GetActive(ctx context.Context, limit, offset int) ([]*domain.Offer, error) {
	return m.offers, nil
}
func (m *mockOfferRepo) Search(ctx context.Context, query string, limit, offset int) ([]*domain.Offer, error) {
	return m.offers, nil
}
func (m *mockOfferRepo) CountActive(ctx context.Context) (int64, error) {
	return int64(len(m.offers)), nil
}
func (m *mockOfferRepo) UpdateStatus(ctx context.Context, id string, status domain.ItemStatus) error {
	return nil
}

type mockRequestRepo struct {
	requests []*domain.Request
}

func (m *mockRequestRepo) Save(ctx context.Context, r *domain.Request) error { return nil }
func (m *mockRequestRepo) GetByID(ctx context.Context, id string) (*domain.Request, error) {
	for _, r := range m.requests {
		if r.ID == id {
			return r, nil
		}
	}
	return nil, nil
}
func (m *mockRequestRepo) GetActive(ctx context.Context, limit, offset int) ([]*domain.Request, error) {
	return m.requests, nil
}
func (m *mockRequestRepo) Search(ctx context.Context, query string, limit, offset int) ([]*domain.Request, error) {
	return m.requests, nil
}
func (m *mockRequestRepo) CountActive(ctx context.Context) (int64, error) {
	return int64(len(m.requests)), nil
}
func (m *mockRequestRepo) UpdateStatus(ctx context.Context, id string, status domain.ItemStatus) error {
	return nil
}

type mockMatchRepo struct {
	matches []*domain.MatchWithDetails
}

func (m *mockMatchRepo) Save(ctx context.Context, match *domain.Match) error { return nil }
func (m *mockMatchRepo) GetByID(ctx context.Context, id string) (*domain.Match, error) {
	for _, match := range m.matches {
		if match.ID == id {
			return &domain.Match{
				ID:        match.ID,
				OfferID:   match.OfferID,
				RequestID: match.RequestID,
				Score:     match.Score,
				Status:    match.Status,
			}, nil
		}
	}
	return nil, nil
}
func (m *mockMatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*domain.MatchWithDetails, error) {
	return m.matches, nil
}
func (m *mockMatchRepo) CountPending(ctx context.Context) (int64, error) {
	return int64(len(m.matches)), nil
}
func (m *mockMatchRepo) UpdateStatus(ctx context.Context, id string, status domain.MatchStatus, matchedBy string) error {
	return nil
}
func (m *mockMatchRepo) GetByOfferID(ctx context.Context, offerID string) ([]*domain.Match, error) {
	return nil, nil
}
func (m *mockMatchRepo) GetByRequestID(ctx context.Context, requestID string) ([]*domain.Match, error) {
	return nil, nil
}
func (m *mockMatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) {
	return 0, nil
}

type mockGroupRepo struct {
	groups []*domain.Group
}

func (m *mockGroupRepo) Save(ctx context.Context, g *domain.Group) error {
	return nil
}
func (m *mockGroupRepo) GetAll(ctx context.Context) ([]*domain.Group, error) {
	return m.groups, nil
}
func (m *mockGroupRepo) GetMonitored(ctx context.Context) ([]*domain.Group, error) {
	var result []*domain.Group
	for _, g := range m.groups {
		if g.Monitored {
			result = append(result, g)
		}
	}
	return result, nil
}
func (m *mockGroupRepo) SetMonitored(ctx context.Context, jid string, monitored bool) error {
	return nil
}
func (m *mockGroupRepo) UpdateLastMessage(ctx context.Context, jid string) error {
	return nil
}
func (m *mockGroupRepo) IncrementMessageCount(ctx context.Context, jid string) error {
	return nil
}

type mockStatsRepo struct{}

func (m *mockStatsRepo) GetStats(ctx context.Context) (*domain.Stats, error) {
	return &domain.Stats{
		ActiveOffers:   10,
		ActiveRequests: 5,
		PendingMatches: 3,
	}, nil
}
func (m *mockStatsRepo) GetProcessedToday(ctx context.Context) (int64, error) {
	return 25, nil
}

type mockConfigRepo struct{}

func (m *mockConfigRepo) GetAll(ctx context.Context) (*domain.AppConfig, error) {
	return &domain.AppConfig{AutoParseEnabled: true, SkipOwnMessages: true}, nil
}
func (m *mockConfigRepo) UpdateFromMap(ctx context.Context, updates map[string]interface{}) error {
	return nil
}

type mockAuditRepo struct {
	logs []*domain.AuditLog
}

func (m *mockAuditRepo) Log(ctx context.Context, action domain.AuditAction, entityID, details string) error {
	m.logs = append(m.logs, &domain.AuditLog{
		ID:        "test-id",
		Action:    action,
		EntityID:  entityID,
		Details:   details,
		CreatedAt: time.Now(),
	})
	return nil
}
func (m *mockAuditRepo) LogWithValues(ctx context.Context, action domain.AuditAction, entityID, oldVal, newVal, details string) error {
	return nil
}
func (m *mockAuditRepo) GetRecent(ctx context.Context, limit int) ([]*domain.AuditLog, error) {
	return m.logs, nil
}
func (m *mockAuditRepo) GetByAction(ctx context.Context, action domain.AuditAction, limit int) ([]*domain.AuditLog, error) {
	return m.logs, nil
}

// Test helper to create handlers
func newTestHandlers() *Handlers {
	log := zerolog.Nop()
	sseHub := NewSSEHub()

	return NewHandlers(
		&mockOfferRepo{offers: []*domain.Offer{
			{ID: "offer-1", Medication: "Paracetamol", Quantity: 100, Status: domain.StatusActive},
			{ID: "offer-2", Medication: "Ibuprofen", Quantity: 50, Status: domain.StatusActive},
		}},
		&mockRequestRepo{requests: []*domain.Request{
			{ID: "req-1", Medication: "Paracetamol", Quantity: 50, Status: domain.StatusActive},
		}},
		&mockMatchRepo{matches: []*domain.MatchWithDetails{
			{Match: domain.Match{ID: "match-1", OfferID: "offer-1", RequestID: "req-1", Score: 0.9, Status: domain.MatchStatusPending}},
		}},
		&mockGroupRepo{groups: []*domain.Group{
			{JID: "group-1@g.us", Name: "Pharmacy Group", Monitored: true},
		}},
		&mockStatsRepo{},
		sseHub,
		log,
	)
}

// --- Actual Tests ---

func TestGetOffers(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/offers", nil)
	w := httptest.NewRecorder()

	h.GetOffers(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp struct {
		Data []domain.Offer `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if len(resp.Data) != 2 {
		t.Errorf("Expected 2 offers, got %d", len(resp.Data))
	}
}

func TestGetRequests(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/requests", nil)
	w := httptest.NewRecorder()

	h.GetRequests(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestGetMatches(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/matches", nil)
	w := httptest.NewRecorder()

	h.GetMatches(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestGetStats(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/stats", nil)
	w := httptest.NewRecorder()

	h.GetStats(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp struct {
		Data domain.Stats `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if resp.Data.ActiveOffers != 10 {
		t.Errorf("Expected 10 offers, got %d", resp.Data.ActiveOffers)
	}
}

func TestGetGroups(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/groups", nil)
	w := httptest.NewRecorder()

	h.GetGroups(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestGetConfig_NotConfigured(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/config", nil)
	w := httptest.NewRecorder()

	h.GetConfig(w, req)

	// Should return error because configRepo is nil
	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Expected status 503, got %d", w.Code)
	}
}

func TestGetConfig_Success(t *testing.T) {
	h := newTestHandlers()
	h.SetConfigRepo(&mockConfigRepo{})

	req := httptest.NewRequest(http.MethodGet, "/api/config", nil)
	w := httptest.NewRecorder()

	h.GetConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestConfirmMatch_MissingID(t *testing.T) {
	h := newTestHandlers()

	// Create request without path value
	req := httptest.NewRequest(http.MethodPost, "/api/matches//confirm", bytes.NewReader([]byte("{}")))
	w := httptest.NewRecorder()

	h.ConfirmMatch(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Expected status 400, got %d", w.Code)
	}
}

func TestRejectMatch_MissingID(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodPost, "/api/matches//reject", bytes.NewReader([]byte("{}")))
	w := httptest.NewRecorder()

	h.RejectMatch(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Expected status 400, got %d", w.Code)
	}
}

func TestGetAuditLogs_NotConfigured(t *testing.T) {
	h := newTestHandlers()

	req := httptest.NewRequest(http.MethodGet, "/api/audit", nil)
	w := httptest.NewRecorder()

	h.GetAuditLogs(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Expected status 503, got %d", w.Code)
	}
}

func TestGetAuditLogs_Success(t *testing.T) {
	h := newTestHandlers()
	h.SetAuditRepo(&mockAuditRepo{logs: []*domain.AuditLog{
		{ID: "log-1", Action: domain.AuditMatchConfirmed, EntityID: "match-1"},
	}})

	req := httptest.NewRequest(http.MethodGet, "/api/audit", nil)
	w := httptest.NewRecorder()

	h.GetAuditLogs(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

// Health Check Tests

func TestHealthLive(t *testing.T) {
	hc := NewHealthChecker()

	req := httptest.NewRequest(http.MethodGet, "/health/live", nil)
	w := httptest.NewRecorder()

	hc.LiveHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp map[string]interface{}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if resp["status"] != "OK" {
		t.Errorf("Expected status OK, got %v", resp["status"])
	}
}

func TestHealthReady(t *testing.T) {
	hc := NewHealthChecker()

	// Configure DB ping that succeeds
	hc.SetDBPingFunc(func(ctx context.Context) error {
		return nil
	})

	req := httptest.NewRequest(http.MethodGet, "/health/ready", nil)
	w := httptest.NewRecorder()

	hc.ReadyHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp HealthResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if resp.Status != HealthOK {
		t.Errorf("Expected status OK, got %v", resp.Status)
	}

	if resp.Components["database"].Status != HealthOK {
		t.Errorf("Expected database status OK, got %v", resp.Components["database"].Status)
	}
}

func TestHealthReady_WithWhatsAppConnected(t *testing.T) {
	hc := NewHealthChecker()

	hc.SetDBPingFunc(func(ctx context.Context) error { return nil })
	hc.SetWAConnectedFunc(func() bool { return true })

	req := httptest.NewRequest(http.MethodGet, "/health/ready", nil)
	w := httptest.NewRecorder()

	hc.ReadyHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp HealthResponse
	json.NewDecoder(w.Body).Decode(&resp)

	if resp.Components["whatsapp"].Status != HealthOK {
		t.Errorf("Expected whatsapp status OK, got %v", resp.Components["whatsapp"].Status)
	}
}

func TestHealthReady_DatabaseDown(t *testing.T) {
	hc := NewHealthChecker()

	hc.SetDBPingFunc(func(ctx context.Context) error {
		return context.DeadlineExceeded
	})

	req := httptest.NewRequest(http.MethodGet, "/health/ready", nil)
	w := httptest.NewRecorder()

	hc.ReadyHandler(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Expected status 503, got %d", w.Code)
	}
}
