package commands

import (
	"context"

	"pharmabroker/bot/core"
)

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

	return core.Response{
		Text: "*" + core.EscapeMarkdownV2(title) + "*\n" +
			separator + "\n\n" +
			"*📊 Overview*\n" +
			"/start \\- Welcome message\n" +
			"/status \\- Quick system status\n" +
			"/dashboard \\- Full dashboard\n\n" +
			"*💊 Inventory*\n" +
			"/offers \\- Active medication offers\n" +
			"/requests \\- Active medication requests\n\n" +
			"*🔄 Matching*\n" +
			"/pending \\- Pending matches\n" +
			"/confirm \\<id\\> \\- Confirm a match\n" +
			"/reject \\<id\\> \\- Reject a match\n" +
			"/confirmed \\- Recently confirmed\n\n" +
			"*⚙️ Admin*\n" +
			"/groups \\- Monitored WhatsApp groups\n" +
			"/help \\- Show this help\n\n" +
			separator + "\n" +
			"_أوامر بوت فارما بروكر_\n" +
			"/dashboard \\- لوحة التحكم\n" +
			"/offers \\- العروض المتاحة\n" +
			"/requests \\- الطلبات المتاحة\n" +
			"/pending \\- المطابقات المعلقة",
		ParseMode: core.ParseModeMarkdownV2,
	}
}
