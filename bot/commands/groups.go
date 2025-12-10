package commands

import (
	"context"
	"fmt"
	"strings"

	"pharmabroker/bot/core"
	"pharmabroker/domain/repository"
)

// GroupsCommand handles the /groups command.
type GroupsCommand struct {
	groupRepo repository.GroupRepository
}

// NewGroupsCommand creates a new groups command handler.
func NewGroupsCommand(groupRepo repository.GroupRepository) *GroupsCommand {
	return &GroupsCommand{groupRepo: groupRepo}
}

func (c *GroupsCommand) Name() string        { return "groups" }
func (c *GroupsCommand) Description() string { return "List monitored WhatsApp groups" }
func (c *GroupsCommand) Usage() string       { return "/groups" }

func (c *GroupsCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	groups, err := c.groupRepo.GetMonitored(ctx)
	if err != nil {
		return core.Response{
			Text:      core.EscapeMarkdownV2("❌ Error fetching groups. Please try again."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	if len(groups) == 0 {
		return core.Response{
			Text:      core.EscapeMarkdownV2("📱 No monitored groups configured."),
			ParseMode: core.ParseModeMarkdownV2,
		}
	}

	title := fmt.Sprintf("📱 Monitored Groups (%d)", len(groups))
	separator := core.Separator(title)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*%s*\n%s\n\n", core.EscapeMarkdownV2(title), separator))

	for i, g := range groups {
		lastMsg := "Never"
		if g.LastMessage != nil {
			lastMsg = g.LastMessage.Format("Jan 2 15:04")
		}
		sb.WriteString(fmt.Sprintf(
			"%d\\. *%s*\n   Messages: %d\n   Last: %s\n\n",
			i+1,
			core.EscapeMarkdownV2(g.Name),
			g.MessageCount,
			core.EscapeMarkdownV2(lastMsg),
		))
	}

	return core.Response{Text: sb.String(), ParseMode: core.ParseModeMarkdownV2}
}
