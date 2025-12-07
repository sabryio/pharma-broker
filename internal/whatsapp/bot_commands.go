package whatsapp

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage"
)

// BotConfig holds bot command configuration
type BotConfig struct {
	Enabled          bool     `mapstructure:"enabled"`
	AuthorizedPhones []string `mapstructure:"authorized_phones"`
}

// BotCommandHandler handles WhatsApp bot commands
type BotCommandHandler struct {
	matchRepo        domain.MatchRepository
	statsRepo        domain.StatsRepository
	auditRepo        AuditLogger
	authorizedPhones map[string]bool
	log              zerolog.Logger
}

// AuditLogger interface for audit logging
type AuditLogger interface {
	Log(ctx context.Context, action storage.AuditAction, entityID, details string) error
}

// BotCommand represents a parsed command
type BotCommand struct {
	Command string
	Args    []string
	Sender  string
}

// NewBotCommandHandler creates a new bot command handler
func NewBotCommandHandler(
	matchRepo domain.MatchRepository,
	statsRepo domain.StatsRepository,
	auditRepo AuditLogger,
	authorizedPhones []string,
	log zerolog.Logger,
) *BotCommandHandler {
	phones := make(map[string]bool)
	for _, p := range authorizedPhones {
		// Normalize phone: remove +, spaces, dashes
		normalized := normalizePhone(p)
		phones[normalized] = true
	}

	return &BotCommandHandler{
		matchRepo:        matchRepo,
		statsRepo:        statsRepo,
		auditRepo:        auditRepo,
		authorizedPhones: phones,
		log:              log.With().Str("component", "bot").Logger(),
	}
}

// IsCommand checks if a message is a bot command
func IsCommand(text string) bool {
	return strings.HasPrefix(strings.TrimSpace(text), "/")
}

// ParseCommand parses a command string into command and args
func ParseCommand(text string) BotCommand {
	text = strings.TrimSpace(text)
	if !strings.HasPrefix(text, "/") {
		return BotCommand{}
	}

	parts := strings.Fields(text)
	if len(parts) == 0 {
		return BotCommand{}
	}

	cmd := strings.TrimPrefix(parts[0], "/")
	cmd = strings.ToLower(cmd)

	var args []string
	if len(parts) > 1 {
		args = parts[1:]
	}

	return BotCommand{
		Command: cmd,
		Args:    args,
	}
}

// IsAuthorized checks if a phone number is authorized to use commands
func (h *BotCommandHandler) IsAuthorized(senderJID string) bool {
	// Extract phone from JID (format: 201234567890@s.whatsapp.net)
	phone := extractPhoneFromJID(senderJID)
	return h.authorizedPhones[phone]
}

// HandleCommand processes a bot command and returns response text
func (h *BotCommandHandler) HandleCommand(ctx context.Context, msg *IncomingMessage) string {
	// Check authorization
	if !h.IsAuthorized(msg.SenderJID) {
		h.log.Warn().
			Str("sender", msg.SenderJID).
			Str("content", msg.Content).
			Msg("Unauthorized bot command attempt")
		return "" // Silent ignore for unauthorized users
	}

	cmd := ParseCommand(msg.Content)
	cmd.Sender = msg.SenderJID

	h.log.Info().
		Str("command", cmd.Command).
		Strs("args", cmd.Args).
		Str("sender", cmd.Sender).
		Msg("Processing bot command")

	switch cmd.Command {
	case "status":
		return h.handleStatus(ctx)
	case "pending":
		return h.handlePending(ctx)
	case "confirm":
		return h.handleConfirm(ctx, cmd)
	case "reject":
		return h.handleReject(ctx, cmd)
	case "help":
		return h.handleHelp()
	default:
		return "❌ Unknown command. Type /help for available commands.\n" +
			"أمر غير معروف. اكتب /help للأوامر المتاحة."
	}
}

// handleStatus returns system status
func (h *BotCommandHandler) handleStatus(ctx context.Context) string {
	stats, err := h.statsRepo.GetStats(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get stats for status command")
		return "❌ Error fetching status. Please try again."
	}

	return fmt.Sprintf(`📊 *PharmaBroker Status*
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
}

// handlePending lists pending matches
func (h *BotCommandHandler) handlePending(ctx context.Context) string {
	matches, err := h.matchRepo.GetPending(ctx, 5, 0)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get pending matches")
		return "❌ Error fetching matches. Please try again."
	}

	if len(matches) == 0 {
		return "✅ No pending matches! All caught up."
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
	return sb.String()
}

// handleConfirm confirms a match
func (h *BotCommandHandler) handleConfirm(ctx context.Context, cmd BotCommand) string {
	if len(cmd.Args) == 0 {
		return "❌ Usage: /confirm <match_id>\nExample: /confirm abc12345"
	}

	matchID := cmd.Args[0]

	// Find match by partial ID
	match, err := h.findMatchByPartialID(ctx, matchID)
	if err != nil {
		return fmt.Sprintf("❌ Match not found: %s", matchID)
	}

	// Update status
	err = h.matchRepo.UpdateStatus(ctx, match.ID, domain.MatchStatusConfirmed, "bot:"+extractPhoneFromJID(cmd.Sender))
	if err != nil {
		h.log.Error().Err(err).Str("match_id", match.ID).Msg("Failed to confirm match")
		return "❌ Error confirming match. Please try again."
	}

	// Audit log
	if h.auditRepo != nil {
		h.auditRepo.Log(ctx, storage.AuditMatchConfirmed, match.ID, "Confirmed via WhatsApp bot by "+cmd.Sender)
	}

	h.log.Info().Str("match_id", match.ID).Str("by", cmd.Sender).Msg("Match confirmed via bot")

	return fmt.Sprintf("✅ Match *%s* confirmed!\nتم تأكيد المطابقة", match.ID[:8])
}

// handleReject rejects a match
func (h *BotCommandHandler) handleReject(ctx context.Context, cmd BotCommand) string {
	if len(cmd.Args) == 0 {
		return "❌ Usage: /reject <match_id>\nExample: /reject abc12345"
	}

	matchID := cmd.Args[0]

	// Find match by partial ID
	match, err := h.findMatchByPartialID(ctx, matchID)
	if err != nil {
		return fmt.Sprintf("❌ Match not found: %s", matchID)
	}

	// Update status
	err = h.matchRepo.UpdateStatus(ctx, match.ID, domain.MatchStatusRejected, "bot:"+extractPhoneFromJID(cmd.Sender))
	if err != nil {
		h.log.Error().Err(err).Str("match_id", match.ID).Msg("Failed to reject match")
		return "❌ Error rejecting match. Please try again."
	}

	// Audit log
	if h.auditRepo != nil {
		h.auditRepo.Log(ctx, storage.AuditMatchRejected, match.ID, "Rejected via WhatsApp bot by "+cmd.Sender)
	}

	h.log.Info().Str("match_id", match.ID).Str("by", cmd.Sender).Msg("Match rejected via bot")

	return fmt.Sprintf("❌ Match *%s* rejected.\nتم رفض المطابقة", match.ID[:8])
}

// handleHelp returns help text
func (h *BotCommandHandler) handleHelp() string {
	return `📖 *PharmaBroker Bot Commands*
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
/reject - رفض مطابقة`
}

// findMatchByPartialID finds a match by partial ID prefix
func (h *BotCommandHandler) findMatchByPartialID(ctx context.Context, partialID string) (*domain.Match, error) {
	// Try to get pending matches and find one that starts with partialID
	matches, err := h.matchRepo.GetPending(ctx, 50, 0)
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

// Helper functions

func normalizePhone(phone string) string {
	phone = strings.ReplaceAll(phone, "+", "")
	phone = strings.ReplaceAll(phone, " ", "")
	phone = strings.ReplaceAll(phone, "-", "")
	return phone
}

func extractPhoneFromJID(jid string) string {
	// Format: 201234567890@s.whatsapp.net
	parts := strings.Split(jid, "@")
	if len(parts) > 0 {
		return parts[0]
	}
	return jid
}
