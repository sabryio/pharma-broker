package commands

import (
	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// Repositories holds the repositories needed for bot commands.
type Repositories struct {
	Stats   repository.StatsRepository
	Matches repository.MatchRepository
	Audit   core.AuditLogger
}

// RegisterAll registers all standard commands to a bot.
func RegisterAll(bot interface{ RegisterCommand(core.CommandHandler) }, repos Repositories) {
	bot.RegisterCommand(NewStartCommand())
	bot.RegisterCommand(NewStatusCommand(repos.Stats))
	bot.RegisterCommand(NewPendingCommand(repos.Matches))
	bot.RegisterCommand(NewConfirmCommand(repos.Matches, repos.Audit))
	bot.RegisterCommand(NewRejectCommand(repos.Matches, repos.Audit))
	bot.RegisterCommand(NewHelpCommand())
}
