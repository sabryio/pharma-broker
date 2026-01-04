package grpcadapter

import (
	"context"
	"errors"
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

// Feature: send-message, Property 5: Bridge Delivery Success Response
// Validates: Requirements 3.2, 3.3

func TestSendMessage_Success(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "201234567890@s.whatsapp.net",
		Content:      "Hello, this is a test message",
	}

	// Expect SendMessage to be called
	mockProvider.On("SendMessage", ctx, domain.JID(req.RecipientJid), req.Content).Return(nil)

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.True(t, resp.Success)
	assert.NotEmpty(t, resp.MessageId)
	assert.Nil(t, resp.Error)
	mockProvider.AssertExpectations(t)
}

func TestSendMessage_WithReferenceId(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	refID := "ref-12345"
	req := &pb.SendMessageRequest{
		RecipientJid: "201234567890@s.whatsapp.net",
		Content:      "Hello with reference",
		ReferenceId:  &refID,
	}

	mockProvider.On("SendMessage", ctx, domain.JID(req.RecipientJid), req.Content).Return(nil)

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.True(t, resp.Success)
	assert.Equal(t, refID, resp.MessageId)
	mockProvider.AssertExpectations(t)
}

// Feature: send-message, Property 6: Bridge Error Propagation
// Validates: Requirements 3.4

func TestSendMessage_DeliveryError(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "201234567890@s.whatsapp.net",
		Content:      "This message will fail",
	}

	// Simulate WhatsApp delivery failure
	mockProvider.On("SendMessage", ctx, domain.JID(req.RecipientJid), req.Content).
		Return(errors.New("connection timeout"))

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err) // gRPC call succeeds, error is in response
	assert.False(t, resp.Success)
	assert.NotNil(t, resp.Error)
	assert.Contains(t, *resp.Error, "connection timeout")
	mockProvider.AssertExpectations(t)
}

func TestSendMessage_InvalidJID(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "invalid-jid",
		Content:      "Test message",
	}

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.False(t, resp.Success)
	assert.NotNil(t, resp.Error)
	assert.Contains(t, *resp.Error, "invalid JID")
	// SendMessage should NOT be called on provider
	mockProvider.AssertNotCalled(t, "SendMessage")
}

func TestSendMessage_EmptyContent(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "201234567890@s.whatsapp.net",
		Content:      "",
	}

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.False(t, resp.Success)
	assert.NotNil(t, resp.Error)
	assert.Contains(t, *resp.Error, "cannot be empty")
	mockProvider.AssertNotCalled(t, "SendMessage")
}

func TestSendMessage_GroupJID(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "120363123456789012@g.us",
		Content:      "Message to group",
	}

	mockProvider.On("SendMessage", ctx, domain.JID(req.RecipientJid), req.Content).Return(nil)

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.True(t, resp.Success)
	mockProvider.AssertExpectations(t)
}

// Feature: send-message, Edge case tests
// Validates: Requirements 3.5

func TestSendMessage_WhatsAppNotConnected(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "201234567890@s.whatsapp.net",
		Content:      "Test message",
	}

	// Simulate WhatsApp client not connected
	mockProvider.On("SendMessage", ctx, domain.JID(req.RecipientJid), req.Content).
		Return(errors.New("not connected to WhatsApp"))

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.False(t, resp.Success)
	assert.NotNil(t, resp.Error)
	assert.Contains(t, *resp.Error, "not connected")
	mockProvider.AssertExpectations(t)
}

func TestSendMessage_LIDFormat(t *testing.T) {
	mockProvider := new(MockMessageProvider)
	logger := zerolog.Nop()
	server := NewBridgeServer(mockProvider, "operator@s.whatsapp.net", logger)

	ctx := context.Background()
	req := &pb.SendMessageRequest{
		RecipientJid: "abc123def456@lid",
		Content:      "Message to LID",
	}

	mockProvider.On("SendMessage", ctx, domain.JID(req.RecipientJid), req.Content).Return(nil)

	resp, err := server.SendMessage(ctx, req)

	assert.NoError(t, err)
	assert.True(t, resp.Success)
	mockProvider.AssertExpectations(t)
}
