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
			separator + "\n" +
			"/start \\- Welcome message\n" +
			"/status \\- Show system status\n" +
			"/pending \\- List pending matches\n" +
			"/confirm \\<id\\> \\- Confirm a match\n" +
			"/reject \\<id\\> \\- Reject a match\n" +
			"/help \\- Show this help\n" +
			separator + "\n" +
			"أوامر بوت فارما بروكر\n" +
			"/status \\- حالة النظام\n" +
			"/pending \\- المطابقات المعلقة\n" +
			"/confirm \\- تأكيد مطابقة\n" +
			"/reject \\- رفض مطابقة",
		ParseMode: core.ParseModeMarkdownV2,
	}
}
