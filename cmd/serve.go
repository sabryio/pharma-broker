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
	"pharmabroker/internal/domain"
	"pharmabroker/internal/janitor"
	"pharmabroker/internal/monitor"
	"pharmabroker/internal/notify"
	"pharmabroker/internal/reports"
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
	matchQueueRepo := storage.NewMatchQueueRepo(db)
	groupRepo := storage.NewGroupRepo(db)
	statsRepo := storage.NewStatsRepo(db)
	medicationRepo := storage.NewMedicationMappingRepo(db)
	feedbackRepo := storage.NewFeedbackRepo(db)
	leaderboardRepo := storage.NewLeaderboardRepo(db)

	// Load medication mappings from file
	commonMedications, err := domain.LoadMedicationMappings("medications.json")
	if err != nil {
		log.Warn().Err(err).Msg("Failed to load medications.json")
	} else {
		log.Info().Int("count", len(commonMedications)).Msg("Loaded medication mappings")
	}

	// Initialize AI provider (Gemini or Docker Model Runner based on config)
	// Moved up to be available for seeding
	aiProvider, err := ai.NewAIProvider(ctx, cfg, log)
	if err != nil {
		log.Fatal().Err(err).Str("provider", cfg.AI.Provider).Msg("Failed to initialize AI provider")
	}
	log.Info().Str("provider", cfg.AI.Provider).Msg("AI provider initialized")

	// Seed medication mappings if empty
	count, err := medicationRepo.Count(ctx)
	if err == nil && count == 0 {
		log.Info().Msg("Seeding medication mappings (consolidating synonyms)...")

		// 1. Group by English Name for consolidation
		// Also collect all texts to embed in batch
		grouped := make(map[string][]string)
		var arabicsToEmbed []string

		for arabic, english := range commonMedications {
			grouped[english] = append(grouped[english], arabic)
		}

		// Pre-calculate canonical names for batch embedding
		type seedItem struct {
			Canonical string
			English   string
			Synonyms  []string
		}
		var seedQueue []seedItem

		for english, arabics := range grouped {
			if len(arabics) == 0 {
				continue
			}
			canonical := arabics[0]
			var synonyms []string
			if len(arabics) > 1 {
				synonyms = arabics[1:]
			}

			seedQueue = append(seedQueue, seedItem{
				Canonical: canonical,
				English:   english,
				Synonyms:  synonyms,
			})
			arabicsToEmbed = append(arabicsToEmbed, canonical)
		}

		// Batch Embed
		log.Info().Int("count", len(arabicsToEmbed)).Msg("Generating embeddings in batch...")
		embeddings, err := aiProvider.EmbedBatch(ctx, arabicsToEmbed)
		if err != nil {
			log.Warn().Err(err).Msg("Failed to generate batch embeddings, falling back to individual or none")
			// We continue; the embeddings slice might be nil, handled below
		} else {
			log.Info().Msg("Batch embeddings generated successfully")
		}

		// Save mapped data
		for i, item := range seedQueue {
			mapping := &domain.MedicationMapping{
				ArabicName:  item.Canonical,
				EnglishName: item.English,
				Synonyms:    item.Synonyms,
				CreatedAt:   time.Now(),
			}

			if embeddings != nil && i < len(embeddings) {
				mapping.Embedding = embeddings[i]
			}

			if err := medicationRepo.Save(ctx, mapping); err != nil {
				log.Warn().Err(err).Str("english", item.English).Msg("Failed to seed mapping")
			}
		}
		log.Info().Msg("Seeding complete")
	}

	// Initialize WhatsApp manager
	waManager, err := whatsapp.NewManager(ctx, &cfg.WhatsApp, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize WhatsApp manager")
	}

	// Create message listener
	listener := whatsapp.NewListener(log, rawMsgRepo, groupRepo)
	waManager.RegisterHandler(listener)

	// Create SSE hub for real-time updates (Needed BEFORE parser)
	sseHub := api.NewSSEHub()

	// Create config repository for dynamic settings
	configRepo := storage.NewConfigRepo(db)

	// Seed AdminPhone from static config if not present in DB
	// This allows setting initial alert contact via config.yaml
	startupCtx := context.Background()
	if currentCfg, err := configRepo.GetAll(startupCtx); err == nil {
		if currentCfg.AdminPhone == "" && cfg.Monitor.AdminPhone != "" {
			log.Info().Str("phone", cfg.Monitor.AdminPhone).Msg("Seeding AdminPhone from config.yaml")
			if err := configRepo.UpdateFromMap(startupCtx, map[string]any{
				"admin_phone": cfg.Monitor.AdminPhone,
			}); err != nil {
				log.Warn().Err(err).Msg("Failed to seed AdminPhone")
			}
		}
	}

	// Create WarRoom monitor for alerting
	warRoom := monitor.NewWarRoom(waManager, configRepo, log)

	// Create AI parser
	parser := ai.NewParser(
		rawMsgRepo,
		aiProvider,
		offerRepo,
		requestRepo,
		medicationRepo,
		matchQueueRepo,
		configRepo, // Dynamic config
		warRoom,    // Error notifier
		sseHub,     // Pass hub directly
		log,
	)

	// Create and start Janitor for data archival
	janitorService := janitor.NewJanitor(rawMsgRepo, cfg.Database, log)
	janitorService.Start()

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
		// Start message feeding loop
		go func() {
			msgChan := listener.MessageChannel()
			for msg := range msgChan {
				parser.ProcessMessage(context.Background(), msg)
			}
		}()

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

	// Wire feedback and leaderboard repos
	handlers.SetFeedbackRepo(feedbackRepo)
	handlers.SetLeaderboardRepo(leaderboardRepo)

	// Wire audit repo
	auditRepo := storage.NewAuditRepo(db)
	handlers.SetAuditRepo(auditRepo)

	// Create HTTP router
	router := api.NewRouter(handlers, &cfg.API, log)

	// Start WhatsApp connection (async)
	go func() {
		if err := waManager.Connect(ctx); err != nil {
			log.Error().Err(err).Msg("WhatsApp connection error")
		}
	}()

	// Start parser
	parser.Start(ctx)

	// Initialize and start report scheduler if enabled
	var reportScheduler *reports.Scheduler
	if cfg.Reports.Enabled {
		reportRepo := storage.NewReportRepo(db)
		reportGenerator := reports.NewGenerator(reportRepo, log)

		// Configure notification service
		telegramConfig := notify.TelegramConfig{
			Enabled:  cfg.Reports.Telegram.Enabled,
			BotToken: cfg.Reports.Telegram.BotToken,
			ChatIDs:  cfg.Reports.Telegram.ChatIDs,
		}
		emailConfig := notify.EmailConfig{
			Enabled:    cfg.Reports.Email.Enabled,
			SMTPHost:   cfg.Reports.Email.SMTPHost,
			SMTPPort:   cfg.Reports.Email.SMTPPort,
			Username:   cfg.Reports.Email.Username,
			Password:   cfg.Reports.Email.Password,
			FromName:   cfg.Reports.Email.FromName,
			FromEmail:  cfg.Reports.Email.FromEmail,
			Recipients: cfg.Reports.Email.Recipients,
		}
		notifier := notify.NewNotificationService(telegramConfig, emailConfig, log)

		// Configure scheduler
		schedulerConfig := reports.SchedulerConfig{
			Enabled:      cfg.Reports.Enabled,
			IntervalMins: cfg.Reports.IntervalMins,
		}
		if schedulerConfig.IntervalMins <= 0 {
			schedulerConfig.IntervalMins = 60 // Default hourly
		}

		reportConfig := reports.ReportConfig{
			IncludePending:   true,
			IncludeConfirmed: true,
			IncludeRejected:  false,
			MinScore:         cfg.Reports.MinScore,
			Limit:            cfg.Reports.Limit,
			PeriodHours:      schedulerConfig.IntervalMins / 60,
		}
		if reportConfig.MinScore <= 0 {
			reportConfig.MinScore = 0.5
		}
		if reportConfig.Limit <= 0 {
			reportConfig.Limit = 100
		}
		if reportConfig.PeriodHours <= 0 {
			reportConfig.PeriodHours = 1
		}

		reportScheduler = reports.NewScheduler(reportGenerator, notifier, schedulerConfig, reportConfig, log)
		if err := reportScheduler.Start(ctx); err != nil {
			log.Error().Err(err).Msg("Failed to start report scheduler")
		} else {
			log.Info().
				Int("interval_mins", schedulerConfig.IntervalMins).
				Msg("Report scheduler started")
		}
	}

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
	janitorService.Stop()
	if reportScheduler != nil {
		reportScheduler.Stop()
	}
	waManager.Disconnect()
	server.Close()
}
