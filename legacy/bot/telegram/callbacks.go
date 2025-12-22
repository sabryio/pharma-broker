package telegram

import (
	"context"
	"fmt"
	"strings"

	"github.com/go-telegram/bot/models"

	"pharmabroker/bot/commands"
	"pharmabroker/bot/core"
)

// RegisterMatchCallbacks registers confirm/reject callback handlers.
func (b *Bot) RegisterMatchCallbacks(deps core.Dependencies) {
	confirmCmd := commands.NewConfirmCommand(deps.Matches, deps.Audit)
	rejectCmd := commands.NewRejectCommand(deps.Matches, deps.Audit)

	// Handle confirm button clicks
	b.RegisterCallback("confirm:", func(ctx context.Context, tgBot *Bot, query *models.CallbackQuery) error {
		matchID := strings.TrimPrefix(query.Data, "confirm:")
		senderID := fmt.Sprintf("%d", query.From.ID)

		shortID, err := confirmCmd.ConfirmByID(ctx, matchID, senderID)
		if err != nil {
			b.AnswerCallback(ctx, query.ID, "❌ "+err.Error(), true)
			return nil
		}

		// Answer callback
		b.AnswerCallback(ctx, query.ID, fmt.Sprintf("✅ Match %s confirmed!", shortID), false)

		// Update the message to show it's confirmed
		if query.Message.Message != nil {
			newText := fmt.Sprintf("✅ *Match Confirmed*\n\nMatch `%s` has been confirmed\\.", shortID)
			b.EditMessage(ctx, query.Message.Message.Chat.ID, int64(query.Message.Message.ID), newText, core.ParseModeMarkdownV2, nil)
		}

		return nil
	})

	// Handle reject button clicks
	b.RegisterCallback("reject:", func(ctx context.Context, tgBot *Bot, query *models.CallbackQuery) error {
		matchID := strings.TrimPrefix(query.Data, "reject:")
		senderID := fmt.Sprintf("%d", query.From.ID)

		shortID, err := rejectCmd.RejectByID(ctx, matchID, senderID)
		if err != nil {
			b.AnswerCallback(ctx, query.ID, "❌ "+err.Error(), true)
			return nil
		}

		// Answer callback
		b.AnswerCallback(ctx, query.ID, fmt.Sprintf("❌ Match %s rejected!", shortID), false)

		// Update the message to show it's rejected
		if query.Message.Message != nil {
			newText := fmt.Sprintf("❌ *Match Rejected*\n\nMatch `%s` has been rejected\\.", shortID)
			b.EditMessage(ctx, query.Message.Message.Chat.ID, int64(query.Message.Message.ID), newText, core.ParseModeMarkdownV2, nil)
		}

		return nil
	})
}
