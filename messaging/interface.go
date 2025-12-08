// Package messaging provides messaging platform interfaces (WhatsApp, Telegram).
package messaging

import (
	"context"
	"time"
)

// Manager defines the messaging platform manager interface
type Manager interface {
	// Connect establishes connection to the messaging platform
	Connect(ctx context.Context) error

	// Disconnect closes the connection
	Disconnect()

	// IsConnected returns connection status
	IsConnected() bool

	// RegisterHandler adds an event handler
	RegisterHandler(handler EventHandler)

	// SendMessage sends a text message
	SendMessage(ctx context.Context, chatID, content string) error

	// GetQRChannel returns channel for QR codes (for WhatsApp pairing)
	GetQRChannel() <-chan string

	// GetJoinedGroups returns all groups the client is a member of
	GetJoinedGroups(ctx context.Context) ([]*GroupInfo, error)

	// SyncGroups fetches and saves all joined groups
	SyncGroups(ctx context.Context, saveFunc func(jid, name, desc string) error) error
}

// EventHandler processes incoming messages and events
type EventHandler interface {
	// HandleMessage processes an incoming message
	HandleMessage(msg *IncomingMessage)

	// HandleGroupJoined handles joining a new group
	HandleGroupJoined(group *GroupInfo)
}

// IncomingMessage represents a received message
type IncomingMessage struct {
	ID          string
	ChatID      string // Group JID or Chat JID
	ChatName    string
	SenderID    string
	SenderPhone string
	SenderName  string
	Content     string
	Timestamp   time.Time
	IsFromMe    bool

	// Reply context
	ReplyToID      string
	ReplyToContent string
	ReplyToSender  string
}

// GroupInfo represents messaging group information
type GroupInfo struct {
	ID          string
	Name        string
	Description string
}

// BotCommand represents a bot command
type BotCommand struct {
	Name        string
	Description string
	Handler     func(ctx context.Context, msg *IncomingMessage) string
}

// BotHandler handles bot commands
type BotHandler interface {
	// RegisterCommand registers a bot command
	RegisterCommand(cmd BotCommand)

	// HandleCommand processes a potential command message
	// Returns true if message was a command, false otherwise
	HandleCommand(ctx context.Context, msg *IncomingMessage) (bool, string)

	// IsAuthorized checks if sender is authorized to use commands
	IsAuthorized(senderPhone string) bool
}

// Config holds messaging configuration
type Config struct {
	Platform        string   // "whatsapp" or "telegram"
	SessionDir      string   // Directory for session storage
	MonitoredGroups []string // Groups to monitor
	BotEnabled      bool
	AuthorizedUsers []string // Phone numbers allowed to use bot
}
