package commands

import (
	"context"
	"fmt"
	"strings"

	"pharmabroker/bot/core"
)

func init() {
	core.RegisterWithCategory(core.CommandFactory{
		Name:        "help",
		Description: "Show all commands",
		Emoji:       "❓",
		Create: func(deps core.Dependencies) core.CommandHandler {
			return NewHelpCommand()
		},
	}, "overview")
}

// HelpCommand handles the /help command.
type HelpCommand struct{}

// NewHelpCommand creates a new help command handler.
func NewHelpCommand() *HelpCommand { return &HelpCommand{} }

func (c *HelpCommand) Name() string        { return "help" }
func (c *HelpCommand) Description() string { return "Show available commands" }
func (c *HelpCommand) Usage() string       { return "/help" }

func (c *HelpCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	title := "📖 PharmaBroker Bot Commands"
	separator := core.Separator(title)

	var sb strings.Builder
	sb.WriteString("*" + core.EscapeMarkdownV2(title) + "*\n")
	sb.WriteString(separator + "\n\n")

	// Group commands by category
	categories := map[string][]core.CommandMetadata{
		"overview":  {},
		"inventory": {},
		"matching":  {},
		"admin":     {},
		"other":     {},
	}

	for _, meta := range core.GetAllMetadata() {
		cat := core.GetCategory(meta.Name)
		categories[cat] = append(categories[cat], meta)
	}

	// Render each category
	categoryTitles := map[string]string{
		"overview":  "📊 Overview",
		"inventory": "💊 Inventory",
		"matching":  "🔄 Matching",
		"admin":     "⚙️ Admin",
		"other":     "Other",
	}

	order := []string{"overview", "inventory", "matching", "admin"}
	for _, cat := range order {
		cmds := categories[cat]
		if len(cmds) == 0 {
			continue
		}
		sb.WriteString(fmt.Sprintf("*%s*\n", categoryTitles[cat]))
		for _, m := range cmds {
			sb.WriteString(fmt.Sprintf("/%s \\- %s\n", m.Name, core.EscapeMarkdownV2(m.Description)))
		}
		sb.WriteString("\n")
	}

	return core.Response{Text: sb.String(), ParseMode: core.ParseModeMarkdownV2}
}
