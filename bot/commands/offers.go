package commands

import (
	"context"
	"fmt"
	"strings"

	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// OffersCommand handles the /offers command.
type OffersCommand struct {
	offerRepo repository.OfferRepository
}

// NewOffersCommand creates a new offers command handler.
func NewOffersCommand(offerRepo repository.OfferRepository) *OffersCommand {
	return &OffersCommand{offerRepo: offerRepo}
}

func (c *OffersCommand) Name() string        { return "offers" }
func (c *OffersCommand) Description() string { return "List active offers" }
func (c *OffersCommand) Usage() string       { return "/offers" }

func (c *OffersCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	offers, err := c.offerRepo.GetActive(ctx, 10, 0)
	if err != nil {
		return core.Response{
			Text:      core.EscapeMarkdownV2("❌ Error fetching offers. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	if len(offers) == 0 {
		return core.Response{
			Text:      core.EscapeMarkdownV2("📦 No active offers at the moment."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	count, _ := c.offerRepo.CountActive(ctx)
	title := fmt.Sprintf("💊 Active Offers (%d total)", count)
	separator := core.Separator(title)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*%s*\n%s\n\n", core.EscapeMarkdownV2(title), separator))

	for i, o := range offers {
		price := ""
		if o.Price > 0 {
			price = fmt.Sprintf(" @ %.0f EGP", o.Price)
		}
		sb.WriteString(fmt.Sprintf(
			"%d\\. *%s*\n   Qty: %.0f%s\n   From: %s\n\n",
			i+1,
			core.EscapeMarkdownV2(o.Medication),
			o.Quantity,
			core.EscapeMarkdownV2(price),
			core.EscapeMarkdownV2(o.SourceName),
		))
	}

	if count > 10 {
		sb.WriteString(fmt.Sprintf("_\\.\\.\\. and %d more_", count-10))
	}

	return core.Response{Text: sb.String(), ParseMode: core.ParseModeMarkdownV2}
}
