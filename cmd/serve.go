package cmd

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/rs/zerolog"
	"github.com/spf13/cobra"

	publicAI "pharmabroker/ai"
	"pharmabroker/api"
	apiHandlers "pharmabroker/api/handlers"
	"pharmabroker/api/sse"
	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/janitor"
	"pharmabroker/internal/metrics"
	"pharmabroker/internal/monitor"
	"pharmabroker/internal/notify"
	"pharmabroker/internal/reports"
	"pharmabroker/internal/whatsapp"
	"pharmabroker/matching"
	"pharmabroker/parsing"

	// New clean architecture modules
	storageGorm "pharmabroker/storage/gorm"
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

	// Initialize storage/gorm layer
	newDB, err := storageGorm.NewDB(&storageGorm.Config{Path: cfg.Database.Path})
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize storage layer")
	}
	defer newDB.Close()
	log.Info().Str("path", cfg.Database.Path).Msg("Database initialized using storage/gorm")

	// Core repositories
	rawMsgRepo := storageGorm.NewRawMessageRepo(newDB)
	offerRepo := storageGorm.NewOfferRepo(newDB)
	requestRepo := storageGorm.NewRequestRepo(newDB)
	matchRepo := storageGorm.NewMatchRepo(newDB)
	matchQueueRepo := storageGorm.NewMatchQueueRepo(newDB)
	groupRepo := storageGorm.NewGroupRepo(newDB)
	statsRepo := storageGorm.NewStatsRepo(newDB)
	medicationRepo := storageGorm.NewMedicationMappingRepo(newDB)
	feedbackRepo := storageGorm.NewFeedbackRepo(newDB)
	leaderboardRepo := storageGorm.NewLeaderboardRepo(newDB)
	auditRepo := storageGorm.NewAuditRepo(newDB)

	_ = domain.StatusActive // Ensure domain package is used

	// Load medication mappings from file
	commonMedications, err := domain.LoadRichMedicationMappings("medications.json")
	if err != nil {
		log.Warn().Err(err).Msg("Failed to load medications.json")
	} else {
		log.Info().Int("count", len(commonMedications)).Msg("Loaded medication mappings")
	}

	// Initialize AI provider (Gemini or Docker Model Runner based on config)
	// Moved up to be available for seeding
	aiProvider, err := publicAI.NewAIProvider(ctx, cfg, log)
	if err != nil {
		log.Fatal().Err(err).Str("provider", cfg.AI.Provider).Msg("Failed to initialize AI provider")
	}
	log.Info().Str("provider", cfg.AI.Provider).Msg("AI provider initialized")

	// Seed medication mappings if empty
	count, err := medicationRepo.Count(ctx)
	if err == nil && count == 0 {
		log.Info().Msg("Seeding medication mappings from rich JSON format...")

		// Pre-calculate canonical names for batch embedding
		type seedItem struct {
			Canonical string
			English   string
			Synonyms  []string
		}
		var seedQueue []seedItem
		var arabicsToEmbed []string

		// New rich format: commonMedications is []*domain.MedicationMapping
		// Each entry contains ArabicName, EnglishName, and Synonyms
		for _, entry := range commonMedications {
			seedQueue = append(seedQueue, seedItem{
				Canonical: entry.ArabicName,
				English:   entry.EnglishName,
				Synonyms:  entry.Synonyms,
			})
			arabicsToEmbed = append(arabicsToEmbed, entry.ArabicName)
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

	// Load all medication mappings and set them on AI provider for hybrid RAG filtering
	allDBMappings, err := medicationRepo.GetAll(ctx)
	if err != nil {
		log.Warn().Err(err).Msg("Failed to load medication mappings for hybrid filtering")
	} else {
		aiProvider.SetMappings(allDBMappings)
		log.Info().Int("count", len(allDBMappings)).Msg("Configured hybrid RAG filtering")
	}

	// Initialize unmapped medications repo for active learning
	unmappedRepo := storageGorm.NewUnmappedRepo(newDB)
	aiProvider.SetUnmappedRepo(unmappedRepo)

	// Initialize WhatsApp manager
	waManager, err := whatsapp.NewManager(ctx, &cfg.WhatsApp, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize WhatsApp manager")
	}

	// Create message listener
	listener := whatsapp.NewListener(log, rawMsgRepo, groupRepo)
	waManager.RegisterHandler(listener)

	// Create SSE hub for real-time updates (Needed BEFORE parser)
	sseHub := sse.NewSSEHub()

	// Create config repository for dynamic settings
	configRepo := storageGorm.NewConfigRepo(newDB)

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

	// Initialize WhatsApp bot commands if enabled
	if cfg.WhatsApp.BotCommands.Enabled {
		botHandler := whatsapp.NewBotCommandHandler(
			matchRepo,
			statsRepo,
			auditRepo,
			cfg.WhatsApp.BotCommands.AuthorizedPhones,
			log,
		)
		waManager.SetBotHandler(botHandler)
		log.Info().
			Int("authorized_phones", len(cfg.WhatsApp.BotCommands.AuthorizedPhones)).
			Msg("WhatsApp bot commands enabled")
	}

	// Create AI parser
	parser := parsing.NewParser(
		rawMsgRepo,
		aiProvider,
		offerRepo,
		requestRepo,
		matchRepo, // Added dependency
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

	// Wire review queue repo for multi-pass parsing (Phase D.2)
	reviewQueueRepo := storageGorm.NewReviewQueueRepo(newDB)
	parser.SetReviewQueueRepo(reviewQueueRepo)
	log.Info().Msg("Multi-pass parsing enabled with review queue")

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

	// Start message feeding loop (Listener -> Parser)
	go func() {
		msgChan := listener.MessageChannel()
		for msg := range msgChan {
			parser.ProcessMessage(context.Background(), msg)
		}
	}()

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

	// Create individual API handlers from api/handlers package
	offerHandler := apiHandlers.NewOfferHandler(offerRepo, log)
	requestHandler := apiHandlers.NewRequestHandler(requestRepo, log)
	matchHandler := apiHandlers.NewMatchHandler(matchRepo, offerRepo, requestRepo, nil, sseHub, log)
	groupHandler := apiHandlers.NewGroupHandler(groupRepo, log)
	statsHandler := apiHandlers.NewStatsHandler(statsRepo, log)
	configHandler := apiHandlers.NewConfigHandler(configRepo, log)
	feedbackHandler := apiHandlers.NewFeedbackHandler(feedbackRepo, matchRepo, log)
	leaderboardHandler := apiHandlers.NewLeaderboardHandler(leaderboardRepo, log)
	auditHandler := apiHandlers.NewAuditHandler(auditRepo, log)
	healthChecker := apiHandlers.NewHealthChecker()

	// Wire sync function to group handler
	groupHandler.SetSyncFunc(syncGroups)

	// Create api.Handlers struct for router
	appHandlers := &api.Handlers{
		Offer:       offerHandler,
		Request:     requestHandler,
		Match:       matchHandler,
		Group:       groupHandler,
		Stats:       statsHandler,
		Config:      configHandler,
		Feedback:    feedbackHandler,
		Leaderboard: leaderboardHandler,
		Audit:       auditHandler,
		SSE:         sseHub,
		Health:      healthChecker,
	}

	// ========== Adaptive Learning System Setup ==========
	var learningScheduler *matching.LearningScheduler

	if cfg.AdaptiveLearning.Enabled {
		log.Info().Msg("Initializing adaptive learning system...")

		// Create learning repositories
		feedbackRecordRepo := storageGorm.NewFeedbackRecordRepo(newDB)
		weightHistoryRepo := storageGorm.NewWeightHistoryRepo(newDB)

		// Create scorer (shared with parser for live weight updates)
		scorer := matching.NewScorer(nil, nil)

		// Create weight learner
		learner := matching.NewWeightLearnerWithConfig(
			feedbackRecordRepo,
			weightHistoryRepo,
			scorer,
			matching.LearningConfig{
				LearningRate:   cfg.AdaptiveLearning.Algorithm.LearningRate,
				MinWeight:      cfg.AdaptiveLearning.Algorithm.MinWeight,
				MaxWeight:      cfg.AdaptiveLearning.Algorithm.MaxWeight,
				MinChange:      cfg.AdaptiveLearning.Algorithm.MinChange,
				MinSamples:     cfg.AdaptiveLearning.Algorithm.MinSamples,
				AnalysisWindow: cfg.AdaptiveLearning.Algorithm.AnalysisWindowDays,
			},
		)

		// Create learning scheduler
		learningScheduler = matching.NewLearningScheduler(
			learner,
			cfg.AdaptiveLearning,
			slogFromZerolog(log),
		)

		// Start scheduler
		if err := learningScheduler.Start(); err != nil {
			log.Error().Err(err).Msg("Failed to start learning scheduler")
		} else {
			log.Info().
				Str("schedule", cfg.AdaptiveLearning.Schedule).
				Bool("auto_apply", cfg.AdaptiveLearning.AutoApply.Enabled).
				Msg("Learning scheduler started")
		}

		// Create learning handler using public AI interface
		// Wrap internal scheduler with adapter to implement public interface
		publicScheduler := publicAI.WrapLearningScheduler(learningScheduler)
		learningHandler := apiHandlers.NewLearningHandler(
			publicScheduler,
			feedbackRecordRepo,
			weightHistoryRepo,
			log,
		)
		appHandlers.Learning = learningHandler

		// Update Prometheus metrics for scheduler state
		updateLearningMetrics(learningScheduler)

		log.Info().Msg("Adaptive learning system initialized")
	} else {
		log.Info().Msg("Adaptive learning disabled (set adaptive_learning.enabled: true to enable)")
	}

	// Create HTTP router using the new api module
	router := api.NewRouter(appHandlers, &cfg.API, log)

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
		reportRepo := storageGorm.NewReportRepo(newDB)
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
	if learningScheduler != nil {
		learningScheduler.Stop()
	}
	waManager.Disconnect()
	server.Close()
}

// slogFromZerolog creates an slog.Logger that wraps zerolog
// This allows the learning scheduler (which uses slog) to log through zerolog
func slogFromZerolog(zlog zerolog.Logger) *slog.Logger {
	return slog.New(&zerologSlogHandler{zlog: zlog})
}

// zerologSlogHandler adapts slog to zerolog
type zerologSlogHandler struct {
	zlog zerolog.Logger
}

func (h *zerologSlogHandler) Enabled(_ context.Context, level slog.Level) bool {
	return true
}

func (h *zerologSlogHandler) Handle(_ context.Context, record slog.Record) error {
	event := h.zlog.Info()

	switch record.Level {
	case slog.LevelDebug:
		event = h.zlog.Debug()
	case slog.LevelInfo:
		event = h.zlog.Info()
	case slog.LevelWarn:
		event = h.zlog.Warn()
	case slog.LevelError:
		event = h.zlog.Error()
	}

	record.Attrs(func(attr slog.Attr) bool {
		event = event.Interface(attr.Key, attr.Value.Any())
		return true
	})

	event.Msg(record.Message)
	return nil
}

func (h *zerologSlogHandler) WithAttrs(attrs []slog.Attr) slog.Handler {
	return h
}

func (h *zerologSlogHandler) WithGroup(name string) slog.Handler {
	return h
}

// updateLearningMetrics updates Prometheus metrics for the learning system
func updateLearningMetrics(scheduler *matching.LearningScheduler) {
	if scheduler == nil {
		return
	}

	status := scheduler.Status()

	// Update scheduler state
	if status.Enabled {
		metrics.LearningSchedulerEnabled.Set(1)
	} else {
		metrics.LearningSchedulerEnabled.Set(0)
	}

	// Update pending weights indicator
	if status.PendingApply != nil {
		metrics.PendingWeightsAvailable.Set(1)
	} else {
		metrics.PendingWeightsAvailable.Set(0)
	}

	// Update last run timestamp
	if !status.LastRun.IsZero() {
		metrics.LastLearningJobTimestamp.Set(float64(status.LastRun.Unix()))
	}

	// Update metrics if we have them
	if status.LastMetrics != nil {
		metrics.ConfirmationRate.Set(status.LastMetrics.ConfirmationRate)
		metrics.ScoreSeparation.Set(status.LastMetrics.AvgScoreConfirmed - status.LastMetrics.AvgScoreRejected)
		metrics.FeedbackSamplesAnalyzed.Set(float64(status.LastMetrics.SampleSize))
	}
}
