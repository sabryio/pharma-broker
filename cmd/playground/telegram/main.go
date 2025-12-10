// Telegram Bot Playground - Test REAL bot commands against REAL database
//
// Usage:
//
//	go run cmd/playground/main.go -token=YOUR_TELEGRAM_BOT_TOKEN
//	TELEGRAM_BOT_TOKEN=xxx go run cmd/playground/main.go
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"strings"
	"time"

	"github.com/rs/zerolog"

	// Import commands package to trigger init() registrations
	_ "pharmabroker/bot/commands"
	"pharmabroker/bot/core"
	"pharmabroker/bot/telegram"
	"pharmabroker/domain/entity"
	"pharmabroker/pkg/config"
	storageGorm "pharmabroker/storage/gorm"
)

func main() {
	token := flag.String("token", "", "Telegram bot token from @BotFather")
	dbDSN := flag.String("dsn", "postgres://postgres:password@localhost:5432/pharmabroker_test?sslmode=disable", "PostgreSQL DSN")
	seedData := flag.Bool("seed", true, "Seed test data (offers, requests, matches)")
	flag.Parse()

	// Load config
	cfg := config.Load()
	cfg.Database.DSN = *dbDSN

	if *token == "" {
		*token = cfg.Reports.Telegram.BotToken
	}

	// Setup logger
	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stdout, TimeFormat: "15:04:05"}).
		With().
		Timestamp().
		Logger()

	fmt.Println(strings.Repeat("=", 60))
	fmt.Println("🤖 TELEGRAM BOT PLAYGROUND")
	fmt.Println("   Testing REAL commands against REAL database")
	fmt.Println(strings.Repeat("=", 60))

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()

	// ============================================================
	// Initialize Database & Repositories (REAL implementations)
	// ============================================================
	log.Info().Str("dsn", "postgres://***@localhost:5432/...").Msg("Initializing database...")

	db, err := storageGorm.NewDB(&storageGorm.Config{DSN: cfg.Database.DSN})
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to init database")
	}
	defer db.Close()

	// Create repositories
	matchRepo := storageGorm.NewMatchRepo(db)
	offerRepo := storageGorm.NewOfferRepo(db)
	requestRepo := storageGorm.NewRequestRepo(db)
	statsRepo := storageGorm.NewStatsRepo(db)
	auditRepo := storageGorm.NewAuditRepo(db)
	groupRepo := storageGorm.NewGroupRepo(db)
	botUserRepo := storageGorm.NewBotUserRepo(db)

	log.Info().Msg("✅ Repositories initialized")

	// ============================================================
	// Seed Test Data (optional)
	// ============================================================
	if *seedData {
		log.Info().Msg("Seeding test data...")
		seedTestData(ctx, db, offerRepo, requestRepo, matchRepo, log)
	}

	// ============================================================
	// Create Telegram Bot with REAL Commands
	// ============================================================
	log.Info().Msg("Creating Telegram bot with real commands...")

	bot, err := telegram.NewBot(telegram.Config{
		BotToken: *token,
	}, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create bot")
	}

	// Build dependencies for commands
	deps := core.Dependencies{
		Stats:    statsRepo,
		Matches:  matchRepo,
		Offers:   offerRepo,
		Requests: requestRepo,
		Groups:   groupRepo,
		Audit:    auditRepo,
		BotUsers: botUserRepo,
	}

	// Register all commands from registry
	for _, handler := range core.BuildCommands(deps) {
		bot.RegisterCommand(handler)
	}

	// Register Telegram callback handlers for inline buttons
	bot.RegisterMatchCallbacks(deps)

	log.Info().Msg("✅ Registered all commands and callbacks")

	// ============================================================
	// Print Summary
	// ============================================================
	offers, _ := offerRepo.GetActive(ctx, 100, 0)
	requests, _ := requestRepo.GetActive(ctx, 100, 0)
	pending, _ := matchRepo.GetPending(ctx, 100, 0)

	fmt.Println(strings.Repeat("-", 60))
	fmt.Println("📊 Database State:")
	fmt.Printf("   Offers:   %d\n", len(offers))
	fmt.Printf("   Requests: %d\n", len(requests))
	fmt.Printf("   Pending:  %d matches\n", len(pending))
	fmt.Println(strings.Repeat("-", 60))
	fmt.Println("🚀 Bot is running! Commands auto-registered from registry.")
	fmt.Println("   Type /help in Telegram to see all commands")
	fmt.Println(strings.Repeat("-", 60))
	fmt.Println("Press Ctrl+C to stop")
	fmt.Println()

	// Start bot
	if err := bot.Start(ctx); err != nil {
		log.Error().Err(err).Msg("Bot error")
	}

	log.Info().Msg("Bot stopped")
}

// seedTestData creates sample offers, requests, and matches for testing
func seedTestData(ctx context.Context, _ *storageGorm.DB, offerRepo *storageGorm.OfferRepo, requestRepo *storageGorm.RequestRepo, matchRepo *storageGorm.MatchRepo, log zerolog.Logger) {
	// Check if data already exists
	existing, _ := offerRepo.GetActive(ctx, 1, 0)
	if len(existing) > 0 {
		log.Info().Msg("Test data already exists, skipping seed")
		return
	}

	now := time.Now()

	// Create test offers
	offers := []*entity.Offer{
		{
			ID:          "offer-1-paracetamol",
			Medication:  "Paracetamol 500mg",
			Quantity:    100,
			Price:       45,
			SourceGroup: "test-group@g.us",
			SourcePhone: "201234567890",
			SourceName:  "Test Pharmacy",
			Status:      entity.StatusActive,
			CreatedAt:   now,
		},
		{
			ID:          "offer-2-augmentin",
			Medication:  "Augmentin 1g",
			Quantity:    50,
			Price:       150,
			SourceGroup: "test-group@g.us",
			SourcePhone: "201098765432",
			SourceName:  "Cairo Pharmacy",
			Status:      entity.StatusActive,
			CreatedAt:   now,
		},
		{
			ID:          "offer-3-concor",
			Medication:  "Concor 5mg",
			Quantity:    30,
			Price:       85,
			SourceGroup: "test-group@g.us",
			SourcePhone: "201555555555",
			SourceName:  "Alex Pharmacy",
			Status:      entity.StatusActive,
			CreatedAt:   now,
		},
	}

	for _, o := range offers {
		if err := offerRepo.Save(ctx, o); err != nil {
			log.Error().Err(err).Str("id", o.ID).Msg("Failed to save offer")
		}
	}

	// Create test requests
	requests := []*entity.Request{
		{
			ID:          "req-1-paracetamol",
			Medication:  "Paracetamol 500mg",
			Quantity:    50,
			MaxPrice:    50,
			Urgent:      true,
			SourceGroup: "test-group@g.us",
			SourcePhone: "201111111111",
			SourceName:  "Urgent Buyer",
			Status:      entity.StatusActive,
			CreatedAt:   now,
		},
		{
			ID:          "req-2-augmentin",
			Medication:  "Augmentin 1g",
			Quantity:    20,
			MaxPrice:    160,
			SourceGroup: "test-group@g.us",
			SourcePhone: "201222222222",
			SourceName:  "Regular Buyer",
			Status:      entity.StatusActive,
			CreatedAt:   now,
		},
	}

	for _, r := range requests {
		if err := requestRepo.Save(ctx, r); err != nil {
			log.Error().Err(err).Str("id", r.ID).Msg("Failed to save request")
		}
	}

	// Create test matches
	matches := []*entity.Match{
		{
			ID:        "match-abc12345",
			OfferID:   "offer-1-paracetamol",
			RequestID: "req-1-paracetamol",
			Score:     0.95,
			Status:    entity.MatchStatusPending,
			CreatedAt: now,
		},
		{
			ID:        "match-def67890",
			OfferID:   "offer-2-augmentin",
			RequestID: "req-2-augmentin",
			Score:     0.87,
			Status:    entity.MatchStatusPending,
			CreatedAt: now,
		},
		{
			ID:        "match-ghi11111",
			OfferID:   "offer-3-concor",
			RequestID: "req-1-paracetamol",
			Score:     0.72,
			Status:    entity.MatchStatusPending,
			CreatedAt: now,
		},
	}

	for _, m := range matches {
		if err := matchRepo.Save(ctx, m); err != nil {
			log.Error().Err(err).Str("id", m.ID).Msg("Failed to save match")
		}
	}

	log.Info().
		Int("offers", len(offers)).
		Int("requests", len(requests)).
		Int("matches", len(matches)).
		Msg("✅ Test data seeded")
}
