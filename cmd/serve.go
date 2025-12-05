package cmd

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/rs/zerolog"
	"github.com/spf13/cobra"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/api"
	"pharmabroker/internal/config"
	"pharmabroker/internal/storage"
	"pharmabroker/internal/whatsapp"
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

	// Create context for graceful shutdown
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Initialize database
	db, err := storage.New(&cfg.Database)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize database")
	}
	defer db.Close()
	log.Info().Str("path", cfg.Database.Path).Msg("Database initialized")

	// Create repositories
	rawMsgRepo := storage.NewRawMessageRepo(db)
	offerRepo := storage.NewOfferRepo(db)
	requestRepo := storage.NewRequestRepo(db)
	matchRepo := storage.NewMatchRepo(db)
	groupRepo := storage.NewGroupRepo(db)
	statsRepo := storage.NewStatsRepo(db)

	// Initialize AI provider (Gemini or Docker Model Runner based on config)
	aiProvider, err := ai.NewAIProvider(ctx, cfg, log)
	if err != nil {
		log.Fatal().Err(err).Str("provider", cfg.AI.Provider).Msg("Failed to initialize AI provider")
	}
	log.Info().Str("provider", cfg.AI.Provider).Msg("AI provider initialized")

	// Initialize WhatsApp manager
	waManager, err := whatsapp.NewManager(ctx, &cfg.WhatsApp, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize WhatsApp manager")
	}

	// Create message listener
	listener := whatsapp.NewListener(log, rawMsgRepo, groupRepo)
	waManager.RegisterHandler(listener)

	// Create AI parser
	parser := ai.NewParser(
		aiProvider,
		rawMsgRepo,
		offerRepo,
		requestRepo,
		matchRepo,
		listener.MessageChannel(),
		&cfg.Parser,
		log,
	)

	// Create config repository for dynamic settings
	configRepo := storage.NewConfigRepo(db)

	// Wire parser to check auto-parse config
	parser.SetAutoParseChecker(func() bool {
		config, err := configRepo.GetAll(ctx)
		if err != nil {
			return true // Default to enabled on error
		}
		return config.AutoParseEnabled
	})

	// Wire listener to check skip own messages config
	listener.SetSkipOwnMessagesChecker(func() bool {
		config, err := configRepo.GetAll(ctx)
		if err != nil {
			return true // Default to skip on error
		}
		return config.SkipOwnMessages
	})

	// Function to sync groups from WhatsApp
	syncGroups := func() error {
		return waManager.SyncGroups(ctx, func(jid, name, desc string) error {
			return groupRepo.SaveFromSync(ctx, jid, name, desc)
		})
	}

	// Auto-sync groups on startup (runs after WhatsApp connects)
	go func() {
		// Recover from any panics
		defer func() {
			if r := recover(); r != nil {
				log.Error().Interface("panic", r).Msg("Panic in auto-sync goroutine")
			}
		}()

		// Wait for connection with timeout (max 2 minutes)
		timeout := time.After(2 * time.Minute)
		for !waManager.IsConnected() {
			select {
			case <-ctx.Done():
				return
			case <-timeout:
				log.Warn().Msg("Timeout waiting for WhatsApp connection - skipping auto-sync")
				return
			case <-time.After(500 * time.Millisecond):
				// Continue waiting
			}
		}

		// Now connected - sync groups
		log.Info().Msg("Auto-syncing groups from WhatsApp...")
		if err := syncGroups(); err != nil {
			log.Warn().Err(err).Msg("Failed to auto-sync groups (will retry on next startup)")
		} else {
			log.Info().Msg("Groups synced successfully")
		}

		// Enable configured groups (optional - gracefully handle empty config)
		if len(cfg.WhatsApp.MonitoredGroups) > 0 {
			enabled, err := groupRepo.EnableFromConfig(ctx, cfg.WhatsApp.MonitoredGroups)
			if err != nil {
				log.Warn().Err(err).Msg("Failed to enable some groups from config")
			} else if enabled > 0 {
				log.Info().Int("count", enabled).Strs("jids", cfg.WhatsApp.MonitoredGroups).Msg("Enabled groups from config")
			} else {
				log.Info().Strs("jids", cfg.WhatsApp.MonitoredGroups).Msg("Configured groups not found yet - use 'pharmabroker monitor' to discover groups")
			}
		}
	}()

	// Create SSE hub for real-time updates
	sseHub := api.NewSSEHub()

	// Wire SSE broadcaster to parser for real-time updates
	parser.SetSSEBroadcaster(sseHub)

	// Create API handlers
	handlers := api.NewHandlers(
		offerRepo,
		requestRepo,
		matchRepo,
		groupRepo,
		statsRepo,
		sseHub,
		log,
	)

	// Wire config repo to handlers
	handlers.SetConfigRepo(configRepo)

	// Wire sync function to handlers (reuse syncGroups defined earlier)
	handlers.SetGroupSyncFunc(syncGroups)

	// Wire analyze function
	handlers.SetAnalyzeFunc(func(text string) (*api.AnalyzeResult, error) {
		// Use the AI provider directly for analysis
		return nil, fmt.Errorf("analyze not implemented for provider interface yet")
	})

	// Create HTTP router
	router := api.NewRouter(handlers, log)

	// Start WhatsApp connection (async)
	go func() {
		if err := waManager.Connect(ctx); err != nil {
			log.Error().Err(err).Msg("WhatsApp connection error")
		}
	}()

	// Start parser
	parser.Start(ctx)

	// Start HTTP server
	server := &http.Server{
		Addr:    fmt.Sprintf(":%d", cfg.Server.Port),
		Handler: router,
	}

	go func() {
		log.Info().Int("port", cfg.Server.Port).Msg("Starting HTTP server")
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatal().Err(err).Msg("HTTP server error")
		}
	}()

	// Wait for shutdown signal
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	log.Info().Msg("Shutting down...")
	cancel()
	parser.Stop()
	waManager.Disconnect()
	server.Close()
}
