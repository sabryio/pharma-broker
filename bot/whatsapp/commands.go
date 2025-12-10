package whatsapp

import (
	"context"
	"fmt"
	"strings"
	"time"

	"pharmabroker/bot/core"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// StatusCommand handles the /status command.
type StatusCommand struct {
	statsRepo repository.StatsRepository
}

func NewStatusCommand(statsRepo repository.StatsRepository) *StatusCommand {
	return &StatusCommand{statsRepo: statsRepo}
}

func (c *StatusCommand) Name() string        { return "status" }
func (c *StatusCommand) Description() string { return "Show system status" }
func (c *StatusCommand) Usage() string       { return "/status" }

func (c *StatusCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	stats, err := c.statsRepo.GetStats(ctx)
	if err != nil {
		return core.Response{
			Text:      "❌ Error fetching status. Please try again.",
			ParseMode: core.ParseModeText,
		}
	}

	text := fmt.Sprintf(`📊 *PharmaBroker Status*
━━━━━━━━━━━━━━━━
✅ System: *Online*
📦 Pending Matches: *%d*
💊 Active Offers: *%d*
📋 Active Requests: *%d*
✔️ Confirmed Today: *%d*
━━━━━━━━━━━━━━━━
🕐 %s`,
		stats.PendingMatches,
		stats.ActiveOffers,
		stats.ActiveRequests,
		stats.ConfirmedToday,
		time.Now().Format("15:04 MST"),
	)

	return core.Response{Text: text, ParseMode: core.ParseModeMarkdown}
}

// PendingCommand handles the /pending command.
type PendingCommand struct {
	matchRepo repository.MatchRepository
}

func NewPendingCommand(matchRepo repository.MatchRepository) *PendingCommand {
	return &PendingCommand{matchRepo: matchRepo}
}

func (c *PendingCommand) Name() string        { return "pending" }
func (c *PendingCommand) Description() string { return "List pending matches" }
func (c *PendingCommand) Usage() string       { return "/pending" }

func (c *PendingCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	matches, err := c.matchRepo.GetPending(ctx, 5, 0)
	if err != nil {
		return core.Response{Text: "❌ Error fetching matches. Please try again."}
	}

	if len(matches) == 0 {
		return core.Response{Text: "✅ No pending matches! All caught up."}
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("📋 *Pending Matches (%d)*\n━━━━━━━━━━━━━━━━\n", len(matches)))

	for i, m := range matches {
		urgentFlag := ""
		if m.Request != nil && m.Request.Urgent {
			urgentFlag = " 🔥"
		}

		offerMed := "Unknown"
		if m.Offer != nil {
			offerMed = m.Offer.Medication
		}

		sb.WriteString(fmt.Sprintf(
			"%d. *%s*%s\n   ID: `%s`\n   Score: %.0f%%\n\n",
			i+1,
			offerMed,
			urgentFlag,
			m.ID[:8],
			m.Score*100,
		))
	}

	sb.WriteString("Reply /confirm <ID> or /reject <ID>")
	return core.Response{Text: sb.String(), ParseMode: core.ParseModeMarkdown}
}

// ConfirmCommand handles the /confirm command.
type ConfirmCommand struct {
	matchRepo repository.MatchRepository
	audit     core.AuditLogger
}

func NewConfirmCommand(matchRepo repository.MatchRepository, audit core.AuditLogger) *ConfirmCommand {
	return &ConfirmCommand{matchRepo: matchRepo, audit: audit}
}

func (c *ConfirmCommand) Name() string        { return "confirm" }
func (c *ConfirmCommand) Description() string { return "Confirm a pending match" }
func (c *ConfirmCommand) Usage() string       { return "/confirm <match_id>" }

func (c *ConfirmCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	if len(cmd.Args) == 0 {
		return core.Response{Text: "❌ Usage: /confirm <match_id>\nExample: /confirm abc12345"}
	}

	matchID := cmd.Args[0]
	match, err := c.findMatchByPartialID(ctx, matchID)
	if err != nil {
		return core.Response{Text: fmt.Sprintf("❌ Match not found: %s", matchID)}
	}

	senderPhone := ExtractPhoneFromJID(msg.SenderID)
	err = c.matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusConfirmed, "bot:"+senderPhone)
	if err != nil {
		return core.Response{Text: "❌ Error confirming match. Please try again."}
	}

	if c.audit != nil {
		c.audit.Log(ctx, entity.AuditMatchConfirmed, match.ID, "Confirmed via WhatsApp bot by "+msg.SenderID)
	}

	return core.Response{
		Text:      fmt.Sprintf("✅ Match *%s* confirmed!\nتم تأكيد المطابقة", match.ID[:8]),
		ParseMode: core.ParseModeMarkdown,
	}
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

// RejectCommand handles the /reject command.
type RejectCommand struct {
	matchRepo repository.MatchRepository
	audit     core.AuditLogger
}

func NewRejectCommand(matchRepo repository.MatchRepository, audit core.AuditLogger) *RejectCommand {
	return &RejectCommand{matchRepo: matchRepo, audit: audit}
}

func (c *RejectCommand) Name() string        { return "reject" }
func (c *RejectCommand) Description() string { return "Reject a pending match" }
func (c *RejectCommand) Usage() string       { return "/reject <match_id>" }

func (c *RejectCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	if len(cmd.Args) == 0 {
		return core.Response{Text: "❌ Usage: /reject <match_id>\nExample: /reject abc12345"}
	}

	matchID := cmd.Args[0]
	match, err := c.findMatchByPartialID(ctx, matchID)
	if err != nil {
		return core.Response{Text: fmt.Sprintf("❌ Match not found: %s", matchID)}
	}

	senderPhone := ExtractPhoneFromJID(msg.SenderID)
	err = c.matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusRejected, "bot:"+senderPhone)
	if err != nil {
		return core.Response{Text: "❌ Error rejecting match. Please try again."}
	}

	if c.audit != nil {
		c.audit.Log(ctx, entity.AuditMatchRejected, match.ID, "Rejected via WhatsApp bot by "+msg.SenderID)
	}

	return core.Response{
		Text:      fmt.Sprintf("❌ Match *%s* rejected.\nتم رفض المطابقة", match.ID[:8]),
		ParseMode: core.ParseModeMarkdown,
	}
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

// HelpCommand handles the /help command.
type HelpCommand struct{}

func NewHelpCommand() *HelpCommand { return &HelpCommand{} }

func (c *HelpCommand) Name() string        { return "help" }
func (c *HelpCommand) Description() string { return "Show available commands" }
func (c *HelpCommand) Usage() string       { return "/help" }

func (c *HelpCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	return core.Response{
		Text: `📖 *PharmaBroker Bot Commands*
━━━━━━━━━━━━━━━━
/status - Show system status
/pending - List pending matches
/confirm <id> - Confirm a match
/reject <id> - Reject a match
/help - Show this help
━━━━━━━━━━━━━━━━
أوامر بوت فارما بروكر
/status - حالة النظام
/pending - المطابقات المعلقة
/confirm - تأكيد مطابقة
/reject - رفض مطابقة`,
		ParseMode: core.ParseModeMarkdown,
	}
}
