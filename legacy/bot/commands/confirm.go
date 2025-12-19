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
		Name:        "confirm",
		Description: "Confirm a match",
		Emoji:       "✅",
		Create: func(deps core.Dependencies) core.CommandHandler {
			if deps.Matches == nil {
				return nil
			}
			return NewConfirmCommand(deps.Matches, deps.Audit)
		},
	}, "matching")
}

// ConfirmCommand handles the /confirm command.
type ConfirmCommand struct {
	matchRepo repository.MatchRepository
	audit     core.AuditLogger
}

// NewConfirmCommand creates a new confirm command handler.
func NewConfirmCommand(matchRepo repository.MatchRepository, audit core.AuditLogger) *ConfirmCommand {
	return &ConfirmCommand{matchRepo: matchRepo, audit: audit}
}

func (c *ConfirmCommand) Name() string        { return "confirm" }
func (c *ConfirmCommand) Description() string { return "Confirm a pending match" }
func (c *ConfirmCommand) Usage() string       { return "/confirm <match_id>" }

func (c *ConfirmCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	if len(cmd.Args) == 0 {
		return core.Response{
			Text:      "❌ Usage: /confirm \\<match\\_id\\>\nExample: `/confirm abc12345`",
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	matchID := cmd.Args[0]
	match, err := c.findMatchByPartialID(ctx, matchID)
	if err != nil {
		return core.Response{
			Text:      fmt.Sprintf("❌ Match not found: `%s`", core.EscapeMarkdownV2(matchID)),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	senderPhone := extractPhone(msg.SenderID)
	err = c.matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusConfirmed, "bot:"+senderPhone, "Confirmed via bot")
	if err != nil {
		return core.Response{Text: core.EscapeMarkdownV2("❌ Error confirming match. Please try again."), ParseMode: core.ParseModeMarkdownV2}
	}

	if c.audit != nil {
		c.audit.Log(ctx, entity.AuditMatchConfirmed, match.ID, "Confirmed via bot by "+msg.SenderID)
	}

	return core.Response{
		Text:      fmt.Sprintf("✅ Match `%s` confirmed\\!\nتم تأكيد المطابقة", match.ID[:8]),
		ParseMode: core.ParseModeMarkdownV2,
	}
}

// ConfirmByID confirms a match by full ID (used by callback handlers).
func (c *ConfirmCommand) ConfirmByID(ctx context.Context, matchID, senderID string) (string, error) {
	match, err := c.matchRepo.GetByID(ctx, matchID)
	if err != nil || match == nil {
		return "", fmt.Errorf("match not found: %s", matchID)
	}

	senderPhone := extractPhone(senderID)
	err = c.matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusConfirmed, "bot:"+senderPhone, "Confirmed via button")
	if err != nil {
		return "", err
	}

	if c.audit != nil {
		c.audit.Log(ctx, entity.AuditMatchConfirmed, match.ID, "Confirmed via button by "+senderID)
	}

	return match.ID[:8], nil
}

func (c *ConfirmCommand) findMatchByPartialID(ctx context.Context, partialID string) (*entity.Match, error) {
	matches, err := c.matchRepo.GetPending(ctx, 50, 0)
	if err != nil {
		return nil, err
	}

	partialID = strings.ToLower(partialID)
	for _, m := range matches {
		if strings.HasPrefix(strings.ToLower(m.ID), partialID) {
			return &m.Match, nil
		}
	}

	return nil, fmt.Errorf("no match found with ID starting with %s", partialID)
}
