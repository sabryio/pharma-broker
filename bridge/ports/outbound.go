package ports

import (
	"context"

	"pharma-bridge/domain"
)

// MessageSink represents an outbound port for sending messages.
// Implementations: gRPC adapter, Mock adapter for testing.
type MessageSink interface {
	// Send forwards a message to the destination.
	Send(ctx context.Context, msg domain.Message) error
	// Close releases resources.
	Close() error
}

// GroupRepository provides access to monitored groups.
type GroupRepository interface {
	// GetMonitoredGroups returns the list of monitored group JIDs.
	GetMonitoredGroups(ctx context.Context) ([]domain.JID, error)
}

// GroupSyncer syncs groups from WhatsApp to the core.
type GroupSyncer interface {
	// SyncGroups sends groups to the core for storage.
	SyncGroups(ctx context.Context, groups []domain.GroupInfo) (added, updated int32, err error)
}
