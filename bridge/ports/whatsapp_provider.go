package ports

import (
	"context"
	"pharma-bridge/domain"
)

// MessageProvider defines outbound actions that a messaging provider (like WhatsApp) can perform.
type MessageProvider interface {
	// SendMessage sends a plain text message to a JID.
	SendMessage(ctx context.Context, to domain.JID, content string) error

	// SendContactCard sends contact information (VCard) to a target JID.
	SendContactCard(ctx context.Context, to domain.JID, contactJID domain.JID, name string, phone string) error
}
