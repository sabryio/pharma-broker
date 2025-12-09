package whatsapp

import (
	"context"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
)

// Mock repositories for testing

type mockMatchRepo struct {
	matches []*domain.MatchWithDetails
}

func (m *mockMatchRepo) Save(ctx context.Context, match *domain.Match) error { return nil }
func (m *mockMatchRepo) GetByID(ctx context.Context, id string) (*domain.Match, error) {
	for _, match := range m.matches {
		if match.ID == id {
			return &match.Match, nil
		}
	}
	return nil, nil
}
func (m *mockMatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*domain.MatchWithDetails, error) {
	if offset >= len(m.matches) {
		return nil, nil
	}
	end := offset + limit
	if end > len(m.matches) {
		end = len(m.matches)
	}
	return m.matches[offset:end], nil
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
	return 5, nil
}

type mockStatsRepo struct{}

func (m *mockStatsRepo) GetStats(ctx context.Context) (*domain.Stats, error) {
	return &domain.Stats{
		ActiveOffers:   10,
		ActiveRequests: 5,
		PendingMatches: 3,
		ConfirmedToday: 7,
	}, nil
}
func (m *mockStatsRepo) GetProcessedToday(ctx context.Context) (int64, error) {
	return 25, nil
}

type mockAuditLogger struct {
	logs []domain.AuditAction
}

func (m *mockAuditLogger) Log(ctx context.Context, action domain.AuditAction, entityID, details string) error {
	m.logs = append(m.logs, action)
	return nil
}

func newTestBotHandler() *BotCommandHandler {
	log := zerolog.Nop()
	matchRepo := &mockMatchRepo{
		matches: []*domain.MatchWithDetails{
			{
				Match: domain.Match{
					ID:        "abc12345-def6-7890",
					OfferID:   "offer-1",
					RequestID: "req-1",
					Score:     0.85,
					Status:    domain.MatchStatusPending,
				},
				Offer:   &domain.Offer{Medication: "Paracetamol 500mg", Quantity: 100},
				Request: &domain.Request{Medication: "Paracetamol", Quantity: 50, Urgent: true},
			},
			{
				Match: domain.Match{
					ID:        "xyz99999-abc1-2345",
					OfferID:   "offer-2",
					RequestID: "req-2",
					Score:     0.72,
					Status:    domain.MatchStatusPending,
				},
				Offer:   &domain.Offer{Medication: "Ibuprofen 400mg", Quantity: 50},
				Request: &domain.Request{Medication: "Ibuprofen", Quantity: 30},
			},
		},
	}

	return NewBotCommandHandler(
		matchRepo,
		&mockStatsRepo{},
		&mockAuditLogger{},
		[]string{"+201234567890", "201098765432"},
		log,
	)
}

// --- Tests ---

func TestIsCommand(t *testing.T) {
	tests := []struct {
		text     string
		expected bool
	}{
		{"/status", true},
		{"/help", true},
		{" /status", true},
		{"status", false},
		{"hello /status", false},
		{"", false},
		{"البحث عن دواء", false},
	}

	for _, tt := range tests {
		t.Run(tt.text, func(t *testing.T) {
			result := IsCommand(tt.text)
			if result != tt.expected {
				t.Errorf("IsCommand(%q) = %v, want %v", tt.text, result, tt.expected)
			}
		})
	}
}

func TestParseCommand(t *testing.T) {
	tests := []struct {
		text        string
		expectedCmd string
		expectedLen int
	}{
		{"/status", "status", 0},
		{"/confirm abc123", "confirm", 1},
		{"/reject abc123 extra", "reject", 2},
		{"/HELP", "help", 0},
		{"not a command", "", 0},
		{"/", "", 0},
	}

	for _, tt := range tests {
		t.Run(tt.text, func(t *testing.T) {
			result := ParseCommand(tt.text)
			if result.Command != tt.expectedCmd {
				t.Errorf("ParseCommand(%q).Command = %q, want %q", tt.text, result.Command, tt.expectedCmd)
			}
			if len(result.Args) != tt.expectedLen {
				t.Errorf("ParseCommand(%q).Args len = %d, want %d", tt.text, len(result.Args), tt.expectedLen)
			}
		})
	}
}

func TestBotHandler_IsAuthorized(t *testing.T) {
	h := newTestBotHandler()

	tests := []struct {
		jid      string
		expected bool
	}{
		{"201234567890@s.whatsapp.net", true},
		{"201098765432@s.whatsapp.net", true},
		{"209999999999@s.whatsapp.net", false},
		{"unknown", false},
	}

	for _, tt := range tests {
		t.Run(tt.jid, func(t *testing.T) {
			result := h.IsAuthorized(tt.jid)
			if result != tt.expected {
				t.Errorf("IsAuthorized(%q) = %v, want %v", tt.jid, result, tt.expected)
			}
		})
	}
}

func TestBotHandler_HandleCommand_Unauthorized(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "209999999999@s.whatsapp.net",
		Content:   "/status",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)
	if response != "" {
		t.Errorf("Expected empty response for unauthorized user, got %q", response)
	}
}

func TestBotHandler_HandleCommand_Status(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/status",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if response == "" {
		t.Error("Expected non-empty response for /status")
	}
	if !contains(response, "PharmaBroker Status") {
		t.Error("Response should contain 'PharmaBroker Status'")
	}
	if !contains(response, "Pending Matches") {
		t.Error("Response should contain 'Pending Matches'")
	}
}

func TestBotHandler_HandleCommand_Pending(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/pending",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if response == "" {
		t.Error("Expected non-empty response for /pending")
	}
	if !contains(response, "Pending Matches") {
		t.Error("Response should contain 'Pending Matches'")
	}
	if !contains(response, "Paracetamol") {
		t.Error("Response should contain medication name")
	}
	if !contains(response, "abc12345") {
		t.Error("Response should contain match ID prefix")
	}
}

func TestBotHandler_HandleCommand_Confirm(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/confirm abc12345",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if !contains(response, "confirmed") {
		t.Errorf("Expected 'confirmed' in response, got %q", response)
	}
}

func TestBotHandler_HandleCommand_Confirm_NoArgs(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/confirm",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if !contains(response, "Usage") {
		t.Errorf("Expected 'Usage' in response for missing args, got %q", response)
	}
}

func TestBotHandler_HandleCommand_Reject(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/reject abc12345",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if !contains(response, "rejected") {
		t.Errorf("Expected 'rejected' in response, got %q", response)
	}
}

func TestBotHandler_HandleCommand_Help(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/help",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if !contains(response, "Bot Commands") {
		t.Error("Response should contain 'Bot Commands'")
	}
	if !contains(response, "/status") {
		t.Error("Response should list /status command")
	}
	if !contains(response, "/confirm") {
		t.Error("Response should list /confirm command")
	}
}

func TestBotHandler_HandleCommand_Unknown(t *testing.T) {
	h := newTestBotHandler()
	ctx := context.Background()

	msg := &IncomingMessage{
		SenderJID: "201234567890@s.whatsapp.net",
		Content:   "/unknown",
		Timestamp: time.Now(),
	}

	response := h.HandleCommand(ctx, msg)

	if !contains(response, "Unknown command") {
		t.Errorf("Expected 'Unknown command' in response, got %q", response)
	}
}

func TestNormalizePhone(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"+201234567890", "201234567890"},
		{"20 123 456 7890", "201234567890"},
		{"20-123-456-7890", "201234567890"},
		{"201234567890", "201234567890"},
	}

	for _, tt := range tests {
		result := normalizePhone(tt.input)
		if result != tt.expected {
			t.Errorf("normalizePhone(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestExtractPhoneFromJID(t *testing.T) {
	tests := []struct {
		jid      string
		expected string
	}{
		{"201234567890@s.whatsapp.net", "201234567890"},
		{"201234567890@g.us", "201234567890"},
		{"plainphone", "plainphone"},
	}

	for _, tt := range tests {
		result := extractPhoneFromJID(tt.jid)
		if result != tt.expected {
			t.Errorf("extractPhoneFromJID(%q) = %q, want %q", tt.jid, result, tt.expected)
		}
	}
}

// Helper
func contains(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
