// Package telegram provides Telegram-specific bot implementation.
package telegram

import (
	"context"
	"fmt"

	"github.com/go-telegram/bot"
	"github.com/go-telegram/bot/models"
	"github.com/rs/zerolog"

	"pharmabroker/bot/core"
)

// Bot implements a Telegram bot using github.com/go-telegram/bot.
type Bot struct {
	client *bot.Bot
	router *core.CommandRouter
	log    zerolog.Logger
	token  string
}

// Config holds Telegram bot configuration.
type Config struct {
	BotToken string
}

// NewBot creates a new Telegram bot.
func NewBot(cfg Config, log zerolog.Logger) (*Bot, error) {
	botLog := log.With().Str("component", "telegram-bot").Logger()

	router := core.NewRouter(botLog)
	router.Use(core.LoggingMiddleware(botLog))

	b := &Bot{
		router: router,
		log:    botLog,
		token:  cfg.BotToken,
	}

	return b, nil
}

// Start starts the Telegram bot and blocks until context is cancelled.
func (b *Bot) Start(ctx context.Context) error {
	opts := []bot.Option{
		bot.WithDefaultHandler(b.handleUpdate),
	}

	client, err := bot.New(b.token, opts...)
	if err != nil {
		return fmt.Errorf("failed to create telegram bot: %w", err)
	}
	b.client = client

	b.log.Info().Msg("Starting Telegram bot...")
	client.Start(ctx)
	b.log.Info().Msg("Telegram bot stopped")
	return nil
}

// RegisterCommand adds a command handler.
func (b *Bot) RegisterCommand(handler core.CommandHandler) {
	b.router.Register(handler)
}

// Platform returns the bot's platform.
func (b *Bot) Platform() core.Platform {
	return core.PlatformTelegram
}

// handleUpdate processes incoming Telegram updates.
func (b *Bot) handleUpdate(ctx context.Context, client *bot.Bot, update *models.Update) {
	if update.Message == nil {
		return
	}

	content := update.Message.Text
	if content == "" {
		return
	}

	// Check if this is a command
	if !core.IsCommand(content) {
		return
	}

	// Build core message
	msg := &core.Message{
		ID:       fmt.Sprintf("%d", update.Message.ID),
		Platform: core.PlatformTelegram,
		SenderID: fmt.Sprintf("%d", update.Message.From.ID),
		ChatID:   fmt.Sprintf("%d", update.Message.Chat.ID),
		Content:  content,
	}

	// Parse and handle command
	cmd := core.ParseCommand(content)
	if cmd == nil {
		return
	}
	cmd.SenderID = msg.SenderID

	response := b.router.Handle(ctx, cmd, msg)
	if response == nil || response.Text == "" {
		return
	}

	// Send response
	_, err := client.SendMessage(ctx, &bot.SendMessageParams{
		ChatID:    update.Message.Chat.ID,
		Text:      response.Text,
		ParseMode: toTelegramParseMode(response.ParseMode),
	})
	if err != nil {
		b.log.Error().Err(err).Msg("Failed to send Telegram response")
	}
}

// HandleMessage implements core.Bot interface for manual message handling.
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

// toTelegramParseMode converts core.ParseMode to Telegram parse mode string.
func toTelegramParseMode(mode core.ParseMode) models.ParseMode {
	switch mode {
	case core.ParseModeMarkdown:
		return models.ParseModeMarkdown
	case core.ParseModeHTML:
		return models.ParseModeHTML
	default:
		return ""
	}
}
