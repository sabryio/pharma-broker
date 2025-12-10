package commands

import (
	"context"
	"fmt"
	"strings"

	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// RequestsCommand handles the /requests command.
type RequestsCommand struct {
	requestRepo repository.RequestRepository
}

// NewRequestsCommand creates a new requests command handler.
func NewRequestsCommand(requestRepo repository.RequestRepository) *RequestsCommand {
	return &RequestsCommand{requestRepo: requestRepo}
}

func (c *RequestsCommand) Name() string        { return "requests" }
func (c *RequestsCommand) Description() string { return "List active requests" }
func (c *RequestsCommand) Usage() string       { return "/requests" }

func (c *RequestsCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	requests, err := c.requestRepo.GetActive(ctx, 10, 0)
	if err != nil {
		return core.Response{
			Text:      core.EscapeMarkdownV2("❌ Error fetching requests. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	if len(requests) == 0 {
		return core.Response{
			Text:      core.EscapeMarkdownV2("📋 No active requests at the moment."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	count, _ := c.requestRepo.CountActive(ctx)
	title := fmt.Sprintf("📋 Active Requests (%d total)", count)
	separator := core.Separator(title)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*%s*\n%s\n\n", core.EscapeMarkdownV2(title), separator))

	for i, r := range requests {
		urgentFlag := ""
		if r.Urgent {
			urgentFlag = " 🔥"
		}
		maxPrice := ""
		if r.MaxPrice > 0 {
			maxPrice = fmt.Sprintf(" (max %.0f EGP)", r.MaxPrice)
		}
		sb.WriteString(fmt.Sprintf(
			"%d\\. *%s*%s\n   Qty: %.0f%s\n   From: %s\n\n",
			i+1,
			core.EscapeMarkdownV2(r.Medication),
			urgentFlag,
			r.Quantity,
			core.EscapeMarkdownV2(maxPrice),
			core.EscapeMarkdownV2(r.SourceName),
		))
	}

	if count > 10 {
		sb.WriteString(fmt.Sprintf("_\\.\\.\\. and %d more_", count-10))
	}

	return core.Response{Text: sb.String(), ParseMode: core.ParseModeMarkdownV2}
}
