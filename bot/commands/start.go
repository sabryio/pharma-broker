package commands

import (
	"context"

	"pharmabroker/bot/core"
)

// StartCommand handles the /start command (Telegram welcome).
type StartCommand struct{}

// NewStartCommand creates a new start command handler.
func NewStartCommand() *StartCommand { return &StartCommand{} }

func (c *StartCommand) Name() string        { return "start" }
func (c *StartCommand) Description() string { return "Start the bot and show welcome message" }
func (c *StartCommand) Usage() string       { return "/start" }

func (c *StartCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	title := "🏥 Welcome to PharmaBroker Bot"
	separator := core.Separator(title)

	return core.Response{
		Text: "*" + core.EscapeMarkdownV2(title) + "*\n" +
			separator + "\n\n" +
			core.EscapeMarkdownV2("مرحباً بك في بوت فارما بروكر!") + "\n\n" +
			core.EscapeMarkdownV2("I help you manage medication offers and requests.") + "\n" +
			core.EscapeMarkdownV2("أساعدك في إدارة عروض وطلبات الأدوية.") + "\n\n" +
			"Try /dashboard for a full overview\\.",
		ParseMode: core.ParseModeMarkdownV2,
	}
}
