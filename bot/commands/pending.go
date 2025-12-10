package commands

import (
	"context"
	"fmt"
	"strings"

	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

func init() {
	core.RegisterWithCategory(core.CommandFactory{
		Name:        "pending",
		Description: "Pending matches",
		Emoji:       "🔄",
		Create: func(deps core.Dependencies) core.CommandHandler {
			if deps.Matches == nil {
				return nil
			}
			return NewPendingCommand(deps.Matches)
		},
	}, "matching")
}

// PendingCommand handles the /pending command.
type PendingCommand struct {
	matchRepo repository.MatchRepository
}

// NewPendingCommand creates a new pending command handler.
func NewPendingCommand(matchRepo repository.MatchRepository) *PendingCommand {
	return &PendingCommand{matchRepo: matchRepo}
}

func (c *PendingCommand) Name() string        { return "pending" }
func (c *PendingCommand) Description() string { return "List pending matches" }
func (c *PendingCommand) Usage() string       { return "/pending" }

func (c *PendingCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	matches, err := c.matchRepo.GetPending(ctx, 5, 0)
	if err != nil {
		return core.Response{
			Text:      core.EscapeMarkdownV2("❌ Error fetching matches. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	if len(matches) == 0 {
		return core.Response{
			Text:      core.EscapeMarkdownV2("✅ No pending matches! All caught up."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	title := fmt.Sprintf("📋 Pending Matches (%d)", len(matches))
	separator := core.Separator(title)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*%s*\n%s\n\n", core.EscapeMarkdownV2(title), separator))

	var keyboard core.InlineKeyboard

	for i, m := range matches {
		urgentFlag := ""
		if m.Request != nil && m.Request.Urgent {
			urgentFlag = " 🔥"
		}

		offerMed := "Unknown"
		if m.Offer != nil {
			offerMed = m.Offer.Medication
		}

		shortID := m.ID[:8]

		sb.WriteString(fmt.Sprintf(
			"%d\\. *%s*%s\n   ID: `%s`\n   Score: *%d%%*\n\n",
			i+1,
			core.EscapeMarkdownV2(offerMed),
			urgentFlag,
			shortID,
			int(m.Score*100),
		))

		// Add inline buttons for each match
		keyboard = append(keyboard, []core.InlineButton{
			{Text: fmt.Sprintf("✅ Confirm %s", shortID), CallbackData: fmt.Sprintf("confirm:%s", m.ID)},
			{Text: fmt.Sprintf("❌ Reject %s", shortID), CallbackData: fmt.Sprintf("reject:%s", m.ID)},
		})
	}

	sb.WriteString("_Tap a button to confirm or reject_")

	return core.Response{
		Text:           sb.String(),
		ParseMode:      core.ParseModeMarkdownV2,
		InlineKeyboard: keyboard,
	}
}
