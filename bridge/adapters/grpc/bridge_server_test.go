package grpcadapter

import (
	"context"
	"testing"

	"github.com/rs/zerolog"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/mock"

	"pharma-bridge/domain"
	pb "pharma-bridge/proto"
)

// MockMessageProvider is a mock for ports.MessageProvider.
type MockMessageProvider struct {
	mock.Mock
}

func (m *MockMessageProvider) SendMessage(ctx context.Context, to domain.JID, content string) error {
	args := m.Called(ctx, to, content)
	return args.Error(0)
}

func (m *MockMessageProvider) SendContactCard(ctx context.Context, to domain.JID, contactJID domain.JID, name string, phone string) error {
	args := m.Called(ctx, to, contactJID, name, phone)
	return args.Error(0)
}

func TestConnectMatch(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	operatorJID := "operator@s.whatsapp.net"
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, operatorJID, logger)

	ctx := context.Background()
	req := &pb.ConnectMatchRequest{
		MatchId:        "match-123",
		OffererJid:     "offerer@s.whatsapp.net",
		OffererPhone:   "1111111111",
		OffererName:    "Alice",
		RequesterJid:   "requester@s.whatsapp.net",
		RequesterPhone: "2222222222",
		RequesterName:  "Bob",
		Medication:     "Aspirin",
	}

	// Expect intro message
	mockProvider.On("SendMessage", ctx, domain.JID(operatorJID), mock.Anything).Return(nil)
	// Expect Offerer card
	mockProvider.On("SendContactCard", ctx, domain.JID(operatorJID), domain.JID(req.OffererJid), mock.MatchedBy(func(s string) bool {
		return s == "Alice (Seller - Aspirin)"
	}), req.OffererPhone).Return(nil)
	// Expect Requester card
	mockProvider.On("SendContactCard", ctx, domain.JID(operatorJID), domain.JID(req.RequesterJid), mock.MatchedBy(func(s string) bool {
		return s == "Bob (Buyer - Aspirin)"
	}), req.RequesterPhone).Return(nil)

	resp, err := server.ConnectMatch(ctx, req)

	assert.NoError(t, err)
	assert.True(t, resp.Success)
	mockProvider.AssertExpectations(t)
}

func TestConnectMatch_NoOperator(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	server := NewBridgeServer(mockProvider, "", zerolog.Nop())

	resp, err := server.ConnectMatch(context.Background(), &pb.ConnectMatchRequest{})

	assert.NoError(t, err)
	assert.False(t, resp.Success)
	assert.Equal(t, "Operator JID is not configured", resp.Error)
}
