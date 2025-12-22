// Package ports defines the interfaces (ports) for the hexagonal architecture.
package ports

import (
	"context"

	"pharma-bridge/domain"
)

// MessageSource represents an inbound port for receiving messages.
// Implementations: WhatsApp adapter, Mock adapter for testing.
type MessageSource interface {
	// Connect establishes connection to the message source.
	Connect(ctx context.Context) error
	// Disconnect closes the connection.
	Disconnect()
	// Messages returns a channel of incoming messages.
	Messages() <-chan domain.Message
	// IsConnected returns true if connected.
	IsConnected() bool
}

// GroupProvider provides access to joined groups from the message source.
type GroupProvider interface {
	// GetJoinedGroups returns all groups the user has joined.
	GetJoinedGroups(ctx context.Context) ([]domain.GroupInfo, error)
}

// QRHandler handles QR code events for pairing.
type QRHandler interface {
	// HandleQRCode processes a new QR code.
	HandleQRCode(code string)
	// HandleEvent processes QR events (success, timeout, etc).
	HandleEvent(event string)
	// HandleError records an error state.
	HandleError(err error)
	// SetPaired marks the handler as paired.
	SetPaired()
	// IsPaired returns true if paired.
	IsPaired() bool
}
