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
		Name:        "reject",
		Description: "Reject a match",
		Emoji:       "❌",
		Create: func(deps core.Dependencies) core.CommandHandler {
			if deps.Matches == nil {
				return nil
			}
			return NewRejectCommand(deps.Matches, deps.Audit)
		},
	}, "matching")
}

// RejectCommand handles the /reject command.
type RejectCommand struct {
	matchRepo repository.MatchRepository
	audit     core.AuditLogger
}

// NewRejectCommand creates a new reject command handler.
func NewRejectCommand(matchRepo repository.MatchRepository, audit core.AuditLogger) *RejectCommand {
	return &RejectCommand{matchRepo: matchRepo, audit: audit}
}

func (c *RejectCommand) Name() string        { return "reject" }
func (c *RejectCommand) Description() string { return "Reject a pending match" }
func (c *RejectCommand) Usage() string       { return "/reject <match_id>" }

func (c *RejectCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	if len(cmd.Args) == 0 {
		return core.Response{
			Text:      "❌ Usage: /reject \\<match\\_id\\>\nExample: `/reject abc12345`",
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
	err = c.matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusRejected, "bot:"+senderPhone)
	if err != nil {
		return core.Response{Text: core.EscapeMarkdownV2("❌ Error rejecting match. Please try again."), ParseMode: core.ParseModeMarkdownV2}
	}

	if c.audit != nil {
		c.audit.Log(ctx, entity.AuditMatchRejected, match.ID, "Rejected via bot by "+msg.SenderID)
	}

	return core.Response{
		Text:      fmt.Sprintf("❌ Match `%s` rejected\\.\nتم رفض المطابقة", match.ID[:8]),
		ParseMode: core.ParseModeMarkdownV2,
	}
}

// RejectByID rejects a match by full ID (used by callback handlers).
func (c *RejectCommand) RejectByID(ctx context.Context, matchID, senderID string) (string, error) {
	match, err := c.matchRepo.GetByID(ctx, matchID)
	if err != nil || match == nil {
		return "", fmt.Errorf("match not found: %s", matchID)
	}

	senderPhone := extractPhone(senderID)
	err = c.matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusRejected, "bot:"+senderPhone)
	if err != nil {
		return "", err
	}

	if c.audit != nil {
		c.audit.Log(ctx, entity.AuditMatchRejected, match.ID, "Rejected via button by "+senderID)
	}

	return match.ID[:8], nil
}

func (c *RejectCommand) findMatchByPartialID(ctx context.Context, partialID string) (*entity.Match, error) {
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
