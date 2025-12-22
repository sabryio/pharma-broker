package commands

import (
	"context"
	"fmt"
	"time"

	"pharmabroker/bot/core"
)

func init() {
	core.RegisterWithCategory(core.CommandFactory{
		Name:        "dashboard",
		Description: "Full system dashboard",
		Emoji:       "📊",
		Create: func(deps core.Dependencies) core.CommandHandler {
			if deps.Stats == nil {
				return nil
			}
			return NewDashboardCommand(deps)
		},
	}, "overview")
}

// DashboardCommand handles the /dashboard command.
type DashboardCommand struct {
	deps core.Dependencies
}

// NewDashboardCommand creates a new dashboard command handler.
func NewDashboardCommand(deps core.Dependencies) *DashboardCommand {
	return &DashboardCommand{deps: deps}
}

func (c *DashboardCommand) Name() string        { return "dashboard" }
func (c *DashboardCommand) Description() string { return "Full system dashboard" }
func (c *DashboardCommand) Usage() string       { return "/dashboard" }

func (c *DashboardCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	stats, err := c.deps.Stats.GetStats(ctx)
	if err != nil {
		return core.Response{
			Text:      core.EscapeMarkdownV2("❌ Error fetching dashboard. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	title := "📊 PharmaBroker Dashboard"
	separator := core.Separator(title)
	miniSep := "─────────────"

	text := fmt.Sprintf("*%s*\n%s\n\n", core.EscapeMarkdownV2(title), separator)

	// System Status
	text += "*🖥️ System Status*\n"
	text += "   Status: ✅ *Online*\n"
	text += fmt.Sprintf("   Time: %s\n\n", core.EscapeMarkdownV2(time.Now().Format("15:04 MST")))

	// Inventory
	text += "*💊 Inventory*\n"
	text += fmt.Sprintf("   Active Offers: *%d*\n", stats.ActiveOffers)
	text += fmt.Sprintf("   Active Requests: *%d*\n\n", stats.ActiveRequests)

	// Matching
	text += "*🔄 Matching*\n"
	text += fmt.Sprintf("   Pending Matches: *%d*\n", stats.PendingMatches)
	text += fmt.Sprintf("   Confirmed Today: *%d*\n", stats.ConfirmedToday)
	if stats.AvgMatchScore > 0 {
		text += fmt.Sprintf("   Avg Score: *%.0f%%*\n", stats.AvgMatchScore*100)
	}
	text += "\n"

	// Processing
	text += "*⚡ Processing*\n"
	text += fmt.Sprintf("   Processed Today: *%d*\n", stats.ProcessedToday)
	text += fmt.Sprintf("   Monitored Groups: *%d*\n\n", stats.MonitoredGroups)

	// Separator
	text += miniSep + "\n"
	text += "_Quick Commands:_\n"
	text += "/offers \\- View offers\n"
	text += "/requests \\- View requests\n"
	text += "/pending \\- Pending matches\n"
	text += "/confirmed \\- Confirmed today"

	return core.Response{Text: text, ParseMode: core.ParseModeMarkdownV2}
}
