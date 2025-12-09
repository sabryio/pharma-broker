package cmd

import (
	"context"
	"os"

	"github.com/rs/zerolog"
	"github.com/spf13/cobra"

	"pharmabroker/app/bootstrap"
	"pharmabroker/pkg/config"
)

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Start the API server and message processor",
	Long: `Starts the PharmaBroker server which:
- Connects to WhatsApp Web
- Monitors configured groups for pharmaceutical messages
- Uses AI to extract offers and requests
- Matches them and serves a dashboard`,
	Run: runServe,
}

func runServe(cmd *cobra.Command, args []string) {
	// Load configuration
	cfg := config.Load()

	// Setup logging
	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stdout}).
		With().
		Timestamp().
		Logger()

	log.Info().Msg("Starting PharmaBroker server...")

	// Validate config
	if err := cfg.Validate(); err != nil {
		log.Fatal().Err(err).Msg("Invalid configuration")
	}

	// Create context
	ctx := context.Background()

	// Create container with database and repositories
	container, err := bootstrap.New(ctx, cfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create container")
	}
	defer container.Close()

	// Initialize AI provider
	if err := container.InitAI(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize AI")
	}

	// Seed medications if needed
	if err := container.SeedMedications(ctx); err != nil {
		log.Warn().Err(err).Msg("Failed to seed medications")
	}

	// Initialize WhatsApp
	if err := container.InitWhatsApp(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize WhatsApp")
	}

	// Initialize SSE hub and war room
	container.InitSSE()
	container.InitWarRoom()

	// Seed admin phone from config if not set
	container.SeedAdminPhone(ctx)

	// Initialize parser
	if err := container.InitParser(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize parser")
	}

	// Initialize janitor for data archival
	container.InitJanitor()

	// Initialize API handlers and router
	if err := container.InitHandlers(); err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize handlers")
	}
	container.InitRouter()

	// Initialize optional schedulers
	if err := container.InitLearningScheduler(ctx); err != nil {
		log.Error().Err(err).Msg("Failed to initialize learning scheduler")
	}
	if err := container.InitReportScheduler(ctx); err != nil {
		log.Error().Err(err).Msg("Failed to initialize report scheduler")
	}

	// Run blocks until shutdown signal
	if err := container.Run(ctx); err != nil {
		log.Fatal().Err(err).Msg("Server error")
	}
}
