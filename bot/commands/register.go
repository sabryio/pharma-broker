package commands

import (
	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// Repositories holds the repositories needed for bot commands.
type Repositories struct {
	Stats    repository.StatsRepository
	Matches  repository.MatchRepository
	Offers   repository.OfferRepository
	Requests repository.RequestRepository
	Groups   repository.GroupRepository
	Audit    repository.AuditRepository
}

// RegisterAll registers all standard commands to a bot.
func RegisterAll(bot interface{ RegisterCommand(core.CommandHandler) }, repos Repositories) {
	// Basic commands
	bot.RegisterCommand(NewStartCommand())
	bot.RegisterCommand(NewHelpCommand())
	bot.RegisterCommand(NewStatusCommand(repos.Stats))

	// Match commands
	bot.RegisterCommand(NewPendingCommand(repos.Matches))
	bot.RegisterCommand(NewConfirmCommand(repos.Matches, repos.Audit))
	bot.RegisterCommand(NewRejectCommand(repos.Matches, repos.Audit))
	bot.RegisterCommand(NewConfirmedCommand(repos.Audit))
	// Inventory commands
	bot.RegisterCommand(NewOffersCommand(repos.Offers))
	bot.RegisterCommand(NewRequestsCommand(repos.Requests))
	// Admin commands
	bot.RegisterCommand(NewGroupsCommand(repos.Groups))
	// Dashboard (needs multiple repos)
	bot.RegisterCommand(NewDashboardCommand(DashboardRepos{
		Stats:    repos.Stats,
		Offers:   repos.Offers,
		Requests: repos.Requests,
		Matches:  repos.Matches,
		Groups:   repos.Groups,
	}))
}
