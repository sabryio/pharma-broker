package commands

import (
	"context"
	"fmt"
	"time"

	"github.com/google/uuid"

	"pharmabroker/bot/core"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

func init() {
	core.RegisterWithCategory(core.CommandFactory{
		Name:        "start",
		Description: "Welcome message",
		Emoji:       "🏥",
		Create: func(deps core.Dependencies) core.CommandHandler {
			return NewStartCommand(deps.BotUsers)
		},
	}, "overview")
}

// StartCommand handles the /start command (Telegram welcome).
type StartCommand struct {
	userRepo repository.BotUserRepository
}

// NewStartCommand creates a new start command handler.
func NewStartCommand(userRepo repository.BotUserRepository) *StartCommand {
	return &StartCommand{userRepo: userRepo}
}

func (c *StartCommand) Name() string        { return "start" }
func (c *StartCommand) Description() string { return "Start the bot and show welcome message" }
func (c *StartCommand) Usage() string       { return "/start" }

func (c *StartCommand) Handle(ctx context.Context, cmd *core.Command, msg *core.Message) core.Response {
	// Save or update user in database
	c.saveUser(ctx, msg)

	title := "🏥 Welcome to PharmaBroker Bot"
	separator := core.Separator(title)

	// Personalized greeting
	greeting := "مرحباً بك"
	if msg.SenderName != "" {
		greeting = fmt.Sprintf("مرحباً بك يا %s", msg.SenderName)
	}
	greeting += " في بوت فارما بروكر"

	return core.Response{
		Text: "*" + core.EscapeMarkdownV2(title) + "*\n" +
			separator + "\n\n" +
			core.EscapeMarkdownV2(greeting+"!") + "\n\n" +
			core.EscapeMarkdownV2("I help you manage medication offers and requests.") + "\n" +
			core.EscapeMarkdownV2("أساعدك في إدارة عروض وطلبات الأدوية.") + "\n\n" +
			"Try /dashboard for a full overview\\.",
		ParseMode: core.ParseModeMarkdownV2,
	}
}

// saveUser creates or updates the user in the database.
func (c *StartCommand) saveUser(ctx context.Context, msg *core.Message) {
	if c.userRepo == nil {
		return
	}

	// Try to find existing user by sender ID (Telegram ID)
	var telegramID int64
	if id, err := parseTelegramID(msg.SenderID); err == nil {
		telegramID = id

		// Check if user already exists
		existing, _ := c.userRepo.GetByTelegramID(ctx, telegramID)
		if existing != nil {
			// Update last active
			c.userRepo.UpdateLastActive(ctx, existing.ID)
			return
		}
	}

	// Create new user
	now := time.Now()
	user := &entity.BotUser{
		ID:           uuid.New().String(),
		TelegramID:   &telegramID,
		TelegramName: msg.SenderName,
		DisplayName:  msg.SenderName,
		FirstName:    msg.SenderName,
		LanguageCode: "en",
		Role:         entity.UserRoleViewer,
		IsAuthorized: false, // Requires admin approval
		CreatedAt:    now,
		UpdatedAt:    now,
		LastActiveAt: now,
		Platform:     string(msg.Platform),
	}

	c.userRepo.Save(ctx, user)
}

// parseTelegramID extracts int64 from sender ID string.
func parseTelegramID(senderID string) (int64, error) {
	var id int64
	_, err := fmt.Sscanf(senderID, "%d", &id)
	return id, err
}
