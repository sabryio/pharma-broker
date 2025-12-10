// Package telegram provides Telegram-specific bot implementation.
package telegram

import (
	"context"
	"fmt"
	"strings"

	"github.com/go-telegram/bot"
	"github.com/go-telegram/bot/models"
	"github.com/rs/zerolog"

	"pharmabroker/bot/core"
)

// Bot implements a Telegram bot using github.com/go-telegram/bot.
type Bot struct {
	client           *bot.Bot
	router           *core.CommandRouter
	callbackHandlers map[string]CallbackHandler
	log              zerolog.Logger
	token            string
}

// CallbackHandler handles callback query data.
type CallbackHandler func(ctx context.Context, b *Bot, query *models.CallbackQuery) error

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
		router:           router,
		callbackHandlers: make(map[string]CallbackHandler),
		log:              botLog,
		token:            cfg.BotToken,
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

	// Set commands in Telegram menu
	b.setCommands(ctx)

	b.log.Info().Msg("Starting Telegram bot...")
	client.Start(ctx)
	b.log.Info().Msg("Telegram bot stopped")
	return nil
}

// setCommands registers bot commands with Telegram to show in the menu.
func (b *Bot) setCommands(ctx context.Context) {
	metadata := core.GetAllMetadata()
	commands := make([]models.BotCommand, 0, len(metadata))

	for _, m := range metadata {
		commands = append(commands, models.BotCommand{
			Command:     m.Name,
			Description: m.Description,
		})
	}

	_, err := b.client.SetMyCommands(ctx, &bot.SetMyCommandsParams{
		Commands: commands,
	})
	if err != nil {
		b.log.Warn().Err(err).Msg("Failed to set bot commands menu")
	} else {
		b.log.Info().Int("commands", len(commands)).Msg("Bot commands menu updated")
	}
}

// RegisterCommand adds a command handler.
func (b *Bot) RegisterCommand(handler core.CommandHandler) {
	b.router.Register(handler)
}

// RegisterCallback registers a callback handler for a prefix.
func (b *Bot) RegisterCallback(prefix string, handler CallbackHandler) {
	b.callbackHandlers[prefix] = handler
}

// Platform returns the bot's platform.
func (b *Bot) Platform() core.Platform {
	return core.PlatformTelegram
}

// Client returns the underlying Telegram bot client.
func (b *Bot) Client() *bot.Bot {
	return b.client
}

// handleUpdate processes incoming Telegram updates.
func (b *Bot) handleUpdate(ctx context.Context, client *bot.Bot, update *models.Update) {
	// Handle callback queries (button clicks)
	if update.CallbackQuery != nil {
		b.handleCallback(ctx, client, update.CallbackQuery)
		return
	}

	// Handle messages
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

	// Build core message - prefer display name, fallback to username
	senderName := update.Message.From.FirstName
	if update.Message.From.LastName != "" {
		senderName += " " + update.Message.From.LastName
	}
	// Only use username if no display name
	if senderName == "" && update.Message.From.Username != "" {
		senderName = "@" + update.Message.From.Username
	}

	msg := &core.Message{
		ID:         fmt.Sprintf("%d", update.Message.ID),
		Platform:   core.PlatformTelegram,
		SenderID:   fmt.Sprintf("%d", update.Message.From.ID),
		SenderName: senderName,
		ChatID:     fmt.Sprintf("%d", update.Message.Chat.ID),
		Content:    content,
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

	// Build send params
	params := &bot.SendMessageParams{
		ChatID:    update.Message.Chat.ID,
		Text:      response.Text,
		ParseMode: toTelegramParseMode(response.ParseMode),
	}

	// Add inline keyboard if present
	if len(response.InlineKeyboard) > 0 {
		params.ReplyMarkup = buildInlineKeyboard(response.InlineKeyboard)
	}

	// Send response
	_, err := client.SendMessage(ctx, params)
	if err != nil {
		b.log.Error().Err(err).Msg("Failed to send Telegram response")
	}
}

// handleCallback processes callback queries (button clicks).
func (b *Bot) handleCallback(ctx context.Context, client *bot.Bot, query *models.CallbackQuery) {
	data := query.Data
	b.log.Debug().Str("data", data).Int64("from", query.From.ID).Msg("Callback query received")

	// Find handler by prefix
	for prefix, handler := range b.callbackHandlers {
		if strings.HasPrefix(data, prefix) {
			if err := handler(ctx, b, query); err != nil {
				b.log.Error().Err(err).Str("prefix", prefix).Msg("Callback handler error")
			}
			return
		}
	}

	// Answer callback to remove loading state
	client.AnswerCallbackQuery(ctx, &bot.AnswerCallbackQueryParams{
		CallbackQueryID: query.ID,
		Text:            "Unknown action",
	})
}

// AnswerCallback answers a callback query.
func (b *Bot) AnswerCallback(ctx context.Context, queryID, text string, showAlert bool) {
	if b.client == nil {
		return
	}
	b.client.AnswerCallbackQuery(ctx, &bot.AnswerCallbackQueryParams{
		CallbackQueryID: queryID,
		Text:            text,
		ShowAlert:       showAlert,
	})
}

// EditMessage edits an existing message.
func (b *Bot) EditMessage(ctx context.Context, chatID, messageID int64, text string, parseMode core.ParseMode, keyboard core.InlineKeyboard) error {
	if b.client == nil {
		return fmt.Errorf("client not initialized")
	}

	params := &bot.EditMessageTextParams{
		ChatID:    chatID,
		MessageID: int(messageID),
		Text:      text,
		ParseMode: toTelegramParseMode(parseMode),
	}

	if len(keyboard) > 0 {
		params.ReplyMarkup = buildInlineKeyboard(keyboard)
	}

	_, err := b.client.EditMessageText(ctx, params)
	return err
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
	case core.ParseModeMarkdownV1:
		return models.ParseModeMarkdownV1
	case core.ParseModeMarkdownV2:
		return models.ParseModeMarkdown
	case core.ParseModeHTML:
		return models.ParseModeHTML
	default:
		return ""
	}
}

// buildInlineKeyboard converts core.InlineKeyboard to Telegram inline keyboard.
func buildInlineKeyboard(keyboard core.InlineKeyboard) *models.InlineKeyboardMarkup {
	rows := make([][]models.InlineKeyboardButton, len(keyboard))
	for i, row := range keyboard {
		buttons := make([]models.InlineKeyboardButton, len(row))
		for j, btn := range row {
			buttons[j] = models.InlineKeyboardButton{
				Text:         btn.Text,
				CallbackData: btn.CallbackData,
				URL:          btn.URL,
			}
		}
		rows[i] = buttons
	}
	return &models.InlineKeyboardMarkup{InlineKeyboard: rows}
}
