package core

import (
	"pharmabroker/domain/repository"
)

// Dependencies holds all repositories needed by commands.
type Dependencies struct {
	Stats    repository.StatsRepository
	Matches  repository.MatchRepository
	Offers   repository.OfferRepository
	Requests repository.RequestRepository
	Groups   repository.GroupRepository
	Audit    repository.AuditRepository
	BotUsers repository.BotUserRepository
}

// CommandFactory defines how to create a command with dependencies.
type CommandFactory struct {
	Name        string                                 // Command name (e.g., "status")
	Description string                                 // Short description for menu
	Emoji       string                                 // Emoji prefix for menu
	Create      func(deps Dependencies) CommandHandler // Factory function
}

// registry holds all registered command factories.
var registry []CommandFactory

// Register adds a command factory to the global registry.
// Call this from init() in each command file.
func Register(factory CommandFactory) {
	registry = append(registry, factory)
}

// GetRegistry returns all registered command factories.
func GetRegistry() []CommandFactory {
	return registry
}

// BuildCommands creates all command handlers with injected dependencies.
func BuildCommands(deps Dependencies) []CommandHandler {
	handlers := make([]CommandHandler, 0, len(registry))
	for _, factory := range registry {
		if handler := factory.Create(deps); handler != nil {
			handlers = append(handlers, handler)
		}
	}
	return handlers
}

// GetCommandMetadata returns command info for Telegram menu.
type CommandMetadata struct {
	Name        string
	Description string
}

// GetAllMetadata returns metadata for all registered commands.
func GetAllMetadata() []CommandMetadata {
	metadata := make([]CommandMetadata, 0, len(registry))
	for _, f := range registry {
		desc := f.Description
		if f.Emoji != "" {
			desc = f.Emoji + " " + f.Description
		}
		metadata = append(metadata, CommandMetadata{
			Name:        f.Name,
			Description: desc,
		})
	}
	return metadata
}

// HelpEntry provides info for dynamic help generation.
type HelpEntry struct {
	Command     string
	Description string
	Category    string
}

// CommandWithCategory extends CommandFactory with category info.
type CommandWithCategory struct {
	CommandFactory
	Category string // e.g., "overview", "inventory", "matching", "admin"
}

// registryWithCategories for help organization
var registryCategories = make(map[string]string)

// RegisterWithCategory registers a command with a category for help organization.
func RegisterWithCategory(factory CommandFactory, category string) {
	Register(factory)
	registryCategories[factory.Name] = category
}

// GetCategory returns the category for a command.
func GetCategory(name string) string {
	if cat, ok := registryCategories[name]; ok {
		return cat
	}
	return "other"
}
