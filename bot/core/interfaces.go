// Package core provides platform-agnostic bot interfaces and types.
package core

import (
	"context"
	"pharmabroker/domain/entity"
	"time"
)

// Platform represents a messaging platform.
type Platform string

const (
	PlatformWhatsApp Platform = "whatsapp"
	PlatformTelegram Platform = "telegram"
)

// Message represents an incoming message from any platform.
type Message struct {
	ID        string
	Platform  Platform
	SenderID  string // Platform-specific sender identifier
	ChatID    string // Group or chat identifier
	Content   string // Message text content
	Timestamp time.Time
	ReplyToID string // ID of message being replied to (optional)

	// Metadata for platform-specific data
	Metadata map[string]any
}

// Response represents a bot response.
type Response struct {
	Text      string
	ParseMode ParseMode
	ReplyToID string // Optional: reply to specific message
	Keyboard  any    // Platform-specific keyboard/buttons
}

// ParseMode indicates how to parse the response text.
type ParseMode string

const (
	ParseModeText     ParseMode = "text"
	ParseModeMarkdown ParseMode = "markdown"
	ParseModeHTML     ParseMode = "html"
)

// Command represents a parsed bot command.
type Command struct {
	Name     string   // Command name without prefix (e.g., "status")
	Args     []string // Command arguments
	RawText  string   // Original message text
	SenderID string   // Who sent the command
}

// CommandHandler handles a specific bot command.
type CommandHandler interface {
	// Name returns the command name (without /).
	Name() string

	// Description returns a brief description for help text.
	Description() string

	// Usage returns usage instructions (optional).
	Usage() string

	// Handle processes the command and returns a response.
	Handle(ctx context.Context, cmd *Command, msg *Message) Response
}

// Bot is the main bot interface for handling messages.
type Bot interface {
	// HandleMessage processes an incoming message.
	HandleMessage(ctx context.Context, msg *Message) *Response

	// RegisterCommand adds a command handler.
	RegisterCommand(handler CommandHandler)

	// Platform returns the bot's platform.
	Platform() Platform
}

// Middleware wraps command handling for cross-cutting concerns.
type Middleware func(next CommandHandler) CommandHandler

// Router routes commands to handlers with middleware support.
type Router interface {
	// Use adds middleware to all commands.
	Use(mw Middleware)

	// Register adds a command handler.
	Register(handler CommandHandler)

	// Handle routes a command to the appropriate handler.
	Handle(ctx context.Context, cmd *Command, msg *Message) *Response
}

// Authorizer checks if a user is authorized to use commands.
type Authorizer interface {
	// IsAuthorized returns true if the sender can use bot commands.
	IsAuthorized(ctx context.Context, senderID string) bool
}

// AuditLogger logs bot command actions.
type AuditLogger interface {
	// Log records a bot action.
	Log(ctx context.Context, action entity.AuditAction, entityID, details string) error
}
