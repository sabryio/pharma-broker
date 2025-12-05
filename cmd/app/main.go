package main

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/joho/godotenv"
	"github.com/mdp/qrterminal/v3"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/api"
	"pharmabroker/internal/config"
	"pharmabroker/internal/storage"
	"pharmabroker/internal/whatsapp"
)

func main() {
	// Load .env file if it exists
	godotenv.Load()

	// Setup logging
	zerolog.TimeFieldFormat = time.RFC3339
	log.Logger = zerolog.New(zerolog.ConsoleWriter{Out: os.Stderr, TimeFormat: "15:04:05"}).
		With().Timestamp().Caller().Logger()

	// Load configuration
	cfg := config.Load()
	if err := cfg.Validate(); err != nil {
		log.Fatal().Err(err).Msg("Invalid configuration")
	}

	log.Info().Msg("Starting PharmaBroker...")

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

	// Initialize Gemini client
	geminiClient, err := ai.NewGeminiClient(ctx, &cfg.Gemini, log.Logger)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize Gemini client")
	}
	log.Info().Str("model", cfg.Gemini.Model).Msg("Gemini client initialized")

	// Initialize WhatsApp manager
	waManager, err := whatsapp.NewManager(ctx, &cfg.WhatsApp, log.Logger)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize WhatsApp manager")
	}

	// Create message listener
	listener := whatsapp.NewListener(log.Logger, rawMsgRepo, groupRepo)
	waManager.RegisterHandler(listener)

	// Create AI parser
	parser := ai.NewParser(
		geminiClient,
		rawMsgRepo,
		offerRepo,
		requestRepo,
		matchRepo,
		listener.MessageChannel(),
		&cfg.Gemini,
		&cfg.Parser,
		log.Logger,
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
		log.Logger,
	)

	// Wire config repo to handlers
	handlers.SetConfigRepo(configRepo)

	// Wire up group sync function
	handlers.SetGroupSyncFunc(func() error {
		if !waManager.IsConnected() {
			return fmt.Errorf("WhatsApp not connected")
		}
		groups, err := waManager.GetJoinedGroups(ctx)
		if err != nil {
			return err
		}
		return listener.SyncGroups(ctx, groups)
	})

	// Create HTTP router
	router := api.NewRouter(handlers, log.Logger)

	// Start HTTP server
	server := &http.Server{
		Addr:         fmt.Sprintf(":%d", cfg.Server.Port),
		Handler:      router,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
	}

	go func() {
		log.Info().Int("port", cfg.Server.Port).Msg("Starting HTTP server")
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatal().Err(err).Msg("HTTP server error")
		}
	}()

	// Start AI parser
	parser.Start(ctx)
	log.Info().Msg("AI parser started")

	// Connect to WhatsApp
	go func() {
		log.Info().Msg("Connecting to WhatsApp...")
		if err := waManager.Connect(ctx); err != nil {
			log.Error().Err(err).Msg("WhatsApp connection failed")
			return
		}

		// Sync groups
		groups, err := waManager.GetJoinedGroups(ctx)
		if err != nil {
			log.Error().Err(err).Msg("Failed to get groups")
		} else {
			if err := listener.SyncGroups(ctx, groups); err != nil {
				log.Error().Err(err).Msg("Failed to sync groups")
			}
			log.Info().Int("count", len(groups)).Msg("Synced WhatsApp groups")
		}
	}()

	// Handle QR code for new sessions
	go func() {
		for qr := range waManager.GetQRChannel() {
			log.Info().Msg("===== SCAN QR CODE =====")
			log.Info().Msg("Open https://web.whatsapp.com and scan this QR code:")
			printQR(qr)
			log.Info().Msg("========================")
		}
	}()

	log.Info().Msgf("Dashboard available at http://localhost:%d", cfg.Server.Port)

	// Wait for shutdown signal
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	log.Info().Msg("Shutting down...")

	// Graceful shutdown
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer shutdownCancel()

	parser.Stop()
	waManager.Disconnect()
	server.Shutdown(shutdownCtx)

	log.Info().Msg("Shutdown complete")
}

// printQR generates a terminal QR code
func printQR(code string) {
	qrterminal.GenerateHalfBlock(code, qrterminal.L, os.Stdout)
}
