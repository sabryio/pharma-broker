// Package commands provides shared bot command handlers for all platforms.
package commands

import (
	"context"
	"fmt"
	"time"

	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// StatusCommand handles the /status command.
type StatusCommand struct {
	statsRepo repository.StatsRepository
}

// NewStatusCommand creates a new status command handler.
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
			Text:      core.EscapeMarkdownV2("❌ Error fetching status. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	title := "📊 PharmaBroker Status"
	separator := core.Separator(title)

	text := fmt.Sprintf("*%s*\n%s\n"+
		"✅ System: *Online*\n"+
		"📦 Pending Matches: *%d*\n"+
		"💊 Active Offers: *%d*\n"+
		"📋 Active Requests: *%d*\n"+
		"✔️ Confirmed Today: *%d*\n"+
		"%s\n"+
		"🕐 %s",
		core.EscapeMarkdownV2(title),
		separator,
		stats.PendingMatches,
		stats.ActiveOffers,
		stats.ActiveRequests,
		stats.ConfirmedToday,
		separator,
		core.EscapeMarkdownV2(time.Now().Format("15:04 MST")),
	)

	return core.Response{Text: text, ParseMode: core.ParseModeMarkdownV2}
}
