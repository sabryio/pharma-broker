package commands

import (
	"context"
	"fmt"
	"strings"

	"pharmabroker/bot/core"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

func init() {
	core.RegisterWithCategory(core.CommandFactory{
		Name:        "confirmed",
		Description: "Recently confirmed",
		Emoji:       "✅",
		Create: func(deps core.Dependencies) core.CommandHandler {
			return NewConfirmedCommand(deps.Audit)
		},
	}, "matching")
}

// ConfirmedCommand handles the /confirmed command.
type ConfirmedCommand struct {
	auditRepo repository.AuditRepository
}

// NewConfirmedCommand creates a new confirmed command handler.
func NewConfirmedCommand(auditRepo repository.AuditRepository) *ConfirmedCommand {
	return &ConfirmedCommand{auditRepo: auditRepo}
}

func (c *ConfirmedCommand) Name() string        { return "confirmed" }
func (c *ConfirmedCommand) Description() string { return "Show recently confirmed matches" }
func (c *ConfirmedCommand) Usage() string       { return "/confirmed" }

func (c *ConfirmedCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	logs, err := c.auditRepo.GetByAction(ctx, entity.AuditMatchConfirmed, 10)
	if err != nil {
		return core.Response{
			Text:      core.EscapeMarkdownV2("❌ Error fetching confirmed matches. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	if len(logs) == 0 {
		return core.Response{
			Text:      core.EscapeMarkdownV2("✅ No recently confirmed matches."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	title := "✅ Recently Confirmed Matches"
	separator := core.Separator(title)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*%s*\n%s\n\n", core.EscapeMarkdownV2(title), separator))

	for i, log := range logs {
		shortID := log.EntityID
		if len(shortID) > 8 {
			shortID = shortID[:8]
		}
		sb.WriteString(fmt.Sprintf(
			"%d\\. Match `%s`\n   %s\n   %s\n\n",
			i+1,
			shortID,
			core.EscapeMarkdownV2(log.Details),
			core.EscapeMarkdownV2(log.CreatedAt.Format("Jan 2 15:04")),
		))
	}

	return core.Response{Text: sb.String(), ParseMode: core.ParseModeMarkdownV2}
}
