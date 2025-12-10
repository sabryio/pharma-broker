// Package whatsapp provides WhatsApp-specific bot implementation.
package whatsapp

import (
	"context"
	"strings"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/bot/core"
)

// Bot implements core.Bot for WhatsApp platform.
type Bot struct {
	router *core.CommandRouter
	auth   core.Authorizer
	log    zerolog.Logger
}

// Config holds WhatsApp bot configuration.
type Config struct {
	AuthorizedPhones []string
}

// NewBot creates a new WhatsApp bot.
func NewBot(cfg Config, log zerolog.Logger) *Bot {
	botLog := log.With().Str("component", "whatsapp-bot").Logger()

	auth := core.NewPhoneAuthorizer(cfg.AuthorizedPhones, botLog)
	router := core.NewRouter(botLog)

	// Add middleware
	router.Use(core.AuthMiddleware(auth, botLog))
	router.Use(core.LoggingMiddleware(botLog))

	return &Bot{
		router: router,
		auth:   auth,
		log:    botLog,
	}
}

// RegisterCommand adds a command handler.
func (b *Bot) RegisterCommand(handler core.CommandHandler) {
	b.router.Register(handler)
}

// Platform returns the bot's platform.
func (b *Bot) Platform() core.Platform {
	return core.PlatformWhatsApp
}

// HandleMessage processes an incoming WhatsApp message.
func (b *Bot) HandleMessage(ctx context.Context, msg *core.Message) *core.Response {
	if !core.IsCommand(msg.Content) {
		return nil
	}

	cmd := core.ParseCommand(msg.Content)
	if cmd == nil {
		return nil
	}
	cmd.SenderID = msg.SenderID

	return b.router.Handle(ctx, cmd, msg)
}

// HandleIncoming is a convenience method for WhatsApp message handling.
// Converts platform-specific message to core.Message.
func (b *Bot) HandleIncoming(ctx context.Context, senderJID, chatID, content string, timestamp time.Time) *core.Response {
	msg := &core.Message{
		Platform:  core.PlatformWhatsApp,
		SenderID:  senderJID,
		ChatID:    chatID,
		Content:   content,
		Timestamp: timestamp,
	}
	return b.HandleMessage(ctx, msg)
}

// IsAuthorized checks if a sender is authorized.
func (b *Bot) IsAuthorized(ctx context.Context, senderJID string) bool {
	return b.auth.IsAuthorized(ctx, senderJID)
}

// ExtractPhoneFromJID extracts phone number from WhatsApp JID.
func ExtractPhoneFromJID(jid string) string {
	// Format: 201234567890@s.whatsapp.net
	parts := strings.Split(jid, "@")
	if len(parts) > 0 {
		return parts[0]
	}
	return jid
}
