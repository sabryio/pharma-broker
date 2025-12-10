package commands

import (
	"context"
	"fmt"
	"time"

	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// DashboardCommand handles the /dashboard command.
type DashboardCommand struct {
	statsRepo   repository.StatsRepository
	offerRepo   repository.OfferRepository
	requestRepo repository.RequestRepository
	matchRepo   repository.MatchRepository
	groupRepo   repository.GroupRepository
}

// DashboardRepos holds repositories needed for dashboard.
type DashboardRepos struct {
	Stats    repository.StatsRepository
	Offers   repository.OfferRepository
	Requests repository.RequestRepository
	Matches  repository.MatchRepository
	Groups   repository.GroupRepository
}

// NewDashboardCommand creates a new dashboard command handler.
func NewDashboardCommand(repos DashboardRepos) *DashboardCommand {
	return &DashboardCommand{
		statsRepo:   repos.Stats,
		offerRepo:   repos.Offers,
		requestRepo: repos.Requests,
		matchRepo:   repos.Matches,
		groupRepo:   repos.Groups,
	}
}

func (c *DashboardCommand) Name() string        { return "dashboard" }
func (c *DashboardCommand) Description() string { return "Full system dashboard" }
func (c *DashboardCommand) Usage() string       { return "/dashboard" }

func (c *DashboardCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	stats, err := c.statsRepo.GetStats(ctx)
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
