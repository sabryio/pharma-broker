// Package bootstrap provides dependency injection and application wiring.
package bootstrap

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

	"pharmabroker/ai"
	"pharmabroker/api"
	apiHandlers "pharmabroker/api/handlers"
	"pharmabroker/api/monitor"
	"pharmabroker/api/sse"
	whatsappbot "pharmabroker/bot/whatsapp"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/matching"
	"pharmabroker/messaging/whatsapp"
	"pharmabroker/notify"
	"pharmabroker/parsing"
	"pharmabroker/pkg/config"
	"pharmabroker/pkg/metrics"
	"pharmabroker/reports"
	"pharmabroker/storage/janitor"

	storageGorm "pharmabroker/storage/gorm"
)

// Bootstrap constants
const (
	// Timeouts
	shutdownTimeout      = 10 * time.Second
	autoSyncTimeout      = 2 * time.Minute
	autoSyncPollInterval = 500 * time.Millisecond

	// Default config values
	defaultReportIntervalMins = 60
)

// Container holds all application dependencies
type Container struct {
	// Configuration
	Config *config.Config
	Logger zerolog.Logger

	// Infrastructure
	DB *storageGorm.DB

	// Repositories
	Repos *Repositories

	// Core Services
	AIProvider ai.Provider
	Parser     *parsing.Parser
	WAManager  *whatsapp.Manager
	SSEHub     *sse.SSEHub
	WarRoom    *monitor.WarRoom

	// HTTP
	Handlers *api.Handlers
	Router   http.Handler
	Server   *http.Server

	// Schedulers
	LearningScheduler *matching.LearningScheduler
	ReportScheduler   *reports.Scheduler
	Janitor           *janitor.Janitor

	// Cleanup functions
	cleanups []func() error
}

// Repositories bundles all repository implementations
type Repositories struct {
	// Core domain repositories
	Offers   repository.OfferRepository
	Requests repository.RequestRepository
	Matches  repository.MatchRepository
	Groups   repository.GroupRepository
	Stats    repository.StatsRepository
	Messages repository.RawMessageRepository

	// Medication and mapping
	Mappings repository.MedicationMappingRepository
	Unmapped repository.UnmappedMedicationRepo

	// Queue management
	Queue  repository.MatchQueueRepository
	Review repository.ReviewQueueRepository

	// Configuration and system
	Config      repository.ConfigRepository
	Audit       repository.AuditRepository
	Feedback    repository.FeedbackRepository
	Leaderboard repository.LeaderboardRepository
}

// New creates a new application container with database and repositories
func New(ctx context.Context, cfg *config.Config, log zerolog.Logger) (*Container, error) {
	c := &Container{
		Config: cfg,
		Logger: log,
	}

	// Initialize database
	db, err := storageGorm.NewDB(&storageGorm.Config{Path: cfg.Database.Path})
	if err != nil {
		return nil, fmt.Errorf("database init: %w", err)
	}
	c.DB = db
	c.cleanups = append(c.cleanups, db.Close)

	log.Info().Str("path", cfg.Database.Path).Msg("Database initialized")

	// Initialize all repositories
	c.Repos = &Repositories{
		Offers:      storageGorm.NewOfferRepo(db),
		Requests:    storageGorm.NewRequestRepo(db),
		Matches:     storageGorm.NewMatchRepo(db),
		Groups:      storageGorm.NewGroupRepo(db),
		Stats:       storageGorm.NewStatsRepo(db),
		Messages:    storageGorm.NewRawMessageRepo(db),
		Mappings:    storageGorm.NewMedicationMappingRepo(db),
		Unmapped:    storageGorm.NewUnmappedRepo(db),
		Queue:       storageGorm.NewMatchQueueRepo(db),
		Review:      storageGorm.NewReviewQueueRepo(db),
		Config:      storageGorm.NewConfigRepo(db),
		Audit:       storageGorm.NewAuditRepo(db),
		Feedback:    storageGorm.NewFeedbackRepo(db),
		Leaderboard: storageGorm.NewLeaderboardRepo(db),
	}

	log.Info().Msg("Repositories initialized")

	return c, nil
}

// InitAI initializes the AI provider
func (c *Container) InitAI(ctx context.Context) error {
	provider, err := ai.NewAIProvider(ctx, c.Config, c.Logger)
	if err != nil {
		return fmt.Errorf("AI provider: %w", err)
	}
	c.AIProvider = provider
	c.Logger.Info().Str("provider", c.Config.AI.Provider).Msg("AI provider initialized")

	// Load and set medication mappings for hybrid RAG
	mappings, err := c.Repos.Mappings.GetAll(ctx)
	if err != nil {
		c.Logger.Warn().Err(err).Msg("Failed to load medication mappings")
	} else {
		c.AIProvider.SetMappings(mappings)
		c.Logger.Info().Int("count", len(mappings)).Msg("Configured hybrid RAG filtering")
	}

	// Set unmapped repo for active learning
	c.AIProvider.SetUnmappedRepo(c.Repos.Unmapped)

	return nil
}

// InitWhatsApp initializes the WhatsApp manager
func (c *Container) InitWhatsApp(ctx context.Context) error {
	manager, err := whatsapp.NewManager(ctx, &c.Config.WhatsApp, c.Logger)
	if err != nil {
		return fmt.Errorf("WhatsApp manager: %w", err)
	}
	c.WAManager = manager
	c.Logger.Info().Msg("WhatsApp manager initialized")
	return nil
}

// InitSSE initializes the SSE hub
func (c *Container) InitSSE() {
	c.SSEHub = sse.NewSSEHub()
	c.Logger.Info().Msg("SSE hub initialized")
}

// InitWarRoom initializes the monitoring/alerting system
func (c *Container) InitWarRoom() {
	c.WarRoom = monitor.NewWarRoom(c.WAManager, c.Repos.Config, c.Logger)
	c.Logger.Info().Msg("WarRoom monitor initialized")
}

// InitParser initializes the message parser
func (c *Container) InitParser(ctx context.Context) error {
	if c.AIProvider == nil {
		return fmt.Errorf("AI provider must be initialized before parser")
	}
	if c.SSEHub == nil {
		return fmt.Errorf("SSE hub must be initialized before parser")
	}

	c.Parser = parsing.NewParser(
		c.Repos.Messages,
		c.AIProvider,
		c.Repos.Offers,
		c.Repos.Requests,
		c.Repos.Matches,
		c.Repos.Mappings,
		c.Repos.Queue,
		c.Repos.Config,
		c.WarRoom,
		c.SSEHub,
		c.Logger,
	)

	// Wire review queue for multi-pass parsing
	c.Parser.SetReviewQueueRepo(c.Repos.Review)

	// Wire auto-parse checker
	c.Parser.SetAutoParseChecker(func() bool {
		cfg, err := c.Repos.Config.GetAll(ctx)
		if err != nil {
			return true
		}
		return cfg.AutoParseEnabled
	})

	c.Logger.Info().Msg("Parser initialized with multi-pass support")
	return nil
}

// InitHandlers initializes all API handlers
func (c *Container) InitHandlers() error {
	offerHandler := apiHandlers.NewOfferHandler(c.Repos.Offers, c.Logger)
	requestHandler := apiHandlers.NewRequestHandler(c.Repos.Requests, c.Logger)
	matchHandler := apiHandlers.NewMatchHandler(c.Repos.Matches, c.Repos.Offers, c.Repos.Requests, c.Repos.Audit, c.SSEHub, c.Logger)
	groupHandler := apiHandlers.NewGroupHandler(c.Repos.Groups, c.Logger)
	statsHandler := apiHandlers.NewStatsHandler(c.Repos.Stats, c.Logger)
	configHandler := apiHandlers.NewConfigHandler(c.Repos.Config, c.Logger)
	feedbackHandler := apiHandlers.NewFeedbackHandler(c.Repos.Feedback, c.Repos.Matches, c.Logger)
	leaderboardHandler := apiHandlers.NewLeaderboardHandler(c.Repos.Leaderboard, c.Logger)
	auditHandler := apiHandlers.NewAuditHandler(c.Repos.Audit, c.Logger)
	healthChecker := apiHandlers.NewHealthChecker()

	// Wire sync function to group handler
	groupHandler.SetSyncFunc(func() error {
		return c.WAManager.SyncGroups(context.Background(), func(jid, name, desc string) error {
			return c.Repos.Groups.SaveFromSync(context.Background(), jid, name, desc)
		})
	})

	c.Handlers = &api.Handlers{
		Offer:       offerHandler,
		Request:     requestHandler,
		Match:       matchHandler,
		Group:       groupHandler,
		Stats:       statsHandler,
		Config:      configHandler,
		Feedback:    feedbackHandler,
		Leaderboard: leaderboardHandler,
		Audit:       auditHandler,
		SSE:         c.SSEHub,
		Health:      healthChecker,
	}

	c.Logger.Info().Msg("API handlers initialized")
	return nil
}

// InitRouter creates the HTTP router
func (c *Container) InitRouter() {
	c.Router = api.NewRouter(c.Handlers, &c.Config.API, c.Logger)
	c.Logger.Info().Msg("HTTP router initialized")
}

// InitLearningScheduler initializes adaptive learning if enabled
func (c *Container) InitLearningScheduler(ctx context.Context) error {
	if !c.Config.AdaptiveLearning.Enabled {
		c.Logger.Info().Msg("Adaptive learning disabled")
		return nil
	}

	feedbackRecordRepo := storageGorm.NewFeedbackRecordRepo(c.DB)
	weightHistoryRepo := storageGorm.NewWeightHistoryRepo(c.DB)
	scorer := matching.NewScorer(nil, nil)

	learner := matching.NewWeightLearnerWithConfig(
		feedbackRecordRepo,
		weightHistoryRepo,
		scorer,
		matching.LearningConfig{
			LearningRate:   c.Config.AdaptiveLearning.Algorithm.LearningRate,
			MinWeight:      c.Config.AdaptiveLearning.Algorithm.MinWeight,
			MaxWeight:      c.Config.AdaptiveLearning.Algorithm.MaxWeight,
			MinChange:      c.Config.AdaptiveLearning.Algorithm.MinChange,
			MinSamples:     c.Config.AdaptiveLearning.Algorithm.MinSamples,
			AnalysisWindow: c.Config.AdaptiveLearning.Algorithm.AnalysisWindowDays,
		},
	)

	c.LearningScheduler = matching.NewLearningScheduler(
		learner,
		c.Config.AdaptiveLearning,
		slogFromZerolog(c.Logger),
	)

	if err := c.LearningScheduler.Start(); err != nil {
		return fmt.Errorf("learning scheduler start: %w", err)
	}

	// Add learning handler to API
	publicScheduler := ai.WrapLearningScheduler(c.LearningScheduler)
	learningHandler := apiHandlers.NewLearningHandler(
		publicScheduler,
		feedbackRecordRepo,
		weightHistoryRepo,
		c.Logger,
	)
	c.Handlers.Learning = learningHandler

	// Update Prometheus metrics
	updateLearningMetrics(c.LearningScheduler)

	c.Logger.Info().
		Str("schedule", c.Config.AdaptiveLearning.Schedule).
		Bool("auto_apply", c.Config.AdaptiveLearning.AutoApply.Enabled).
		Msg("Learning scheduler started")

	return nil
}

// InitReportScheduler initializes report generation if enabled
func (c *Container) InitReportScheduler(ctx context.Context) error {
	if !c.Config.Reports.Enabled {
		c.Logger.Info().Msg("Report scheduler disabled")
		return nil
	}

	reportRepo := storageGorm.NewReportRepo(c.DB)
	reportGenerator := reports.NewGenerator(reportRepo, c.Logger)

	telegramConfig, emailConfig := c.buildNotifierConfigs()
	notifier := notify.NewNotificationService(telegramConfig, emailConfig, c.Logger)

	schedulerConfig := reports.SchedulerConfig{
		Enabled:      c.Config.Reports.Enabled,
		IntervalMins: c.Config.Reports.IntervalMins,
	}
	if schedulerConfig.IntervalMins <= 0 {
		schedulerConfig.IntervalMins = defaultReportIntervalMins
	}

	reportConfig := reports.ReportConfig{
		IncludePending:   true,
		IncludeConfirmed: true,
		IncludeRejected:  false,
		MinScore:         c.Config.Reports.MinScore,
		Limit:            c.Config.Reports.Limit,
		PeriodHours:      schedulerConfig.IntervalMins / 60,
	}

	c.ReportScheduler = reports.NewScheduler(reportGenerator, notifier, schedulerConfig, reportConfig, c.Logger)
	if err := c.ReportScheduler.Start(ctx); err != nil {
		return fmt.Errorf("report scheduler start: %w", err)
	}

	c.Logger.Info().Int("interval_mins", schedulerConfig.IntervalMins).Msg("Report scheduler started")
	return nil
}

// InitJanitor starts the data archival service
func (c *Container) InitJanitor() {
	c.Janitor = janitor.NewJanitor(c.Repos.Messages, c.Config.Database, c.Logger)
	c.Janitor.Start()
	c.Logger.Info().Msg("Janitor service started")
}

// buildNotifierConfigs creates Telegram and Email notification configs from app config
func (c *Container) buildNotifierConfigs() (notify.TelegramConfig, notify.EmailConfig) {
	telegramConfig := notify.TelegramConfig{
		Enabled:  c.Config.Reports.Telegram.Enabled,
		BotToken: c.Config.Reports.Telegram.BotToken,
		ChatIDs:  c.Config.Reports.Telegram.ChatIDs,
	}
	emailConfig := notify.EmailConfig{
		Enabled:    c.Config.Reports.Email.Enabled,
		SMTPHost:   c.Config.Reports.Email.SMTPHost,
		SMTPPort:   c.Config.Reports.Email.SMTPPort,
		Username:   c.Config.Reports.Email.Username,
		Password:   c.Config.Reports.Email.Password,
		FromName:   c.Config.Reports.Email.FromName,
		FromEmail:  c.Config.Reports.Email.FromEmail,
		Recipients: c.Config.Reports.Email.Recipients,
	}
	return telegramConfig, emailConfig
}

// Run starts all services and blocks until shutdown signal
func (c *Container) Run(ctx context.Context) error {
	// Create listener for WhatsApp messages
	listener := whatsapp.NewListener(c.Logger, c.Repos.Messages, c.Repos.Groups)
	c.WAManager.RegisterHandler(listener)

	// Wire skip own messages checker
	listener.SetSkipOwnMessagesChecker(func() bool {
		cfg, err := c.Repos.Config.GetAll(ctx)
		if err != nil {
			return true
		}
		return cfg.SkipOwnMessages
	})

	// Configure bot commands if enabled
	if c.Config.WhatsApp.BotCommands.Enabled {
		// Create WhatsApp bot with commands
		bot := whatsappbot.NewBot(whatsappbot.Config{
			AuthorizedPhones: c.Config.WhatsApp.BotCommands.AuthorizedPhones,
		}, c.Logger)

		// Register commands
		bot.RegisterCommand(whatsappbot.NewStatusCommand(c.Repos.Stats))
		bot.RegisterCommand(whatsappbot.NewPendingCommand(c.Repos.Matches))
		bot.RegisterCommand(whatsappbot.NewConfirmCommand(c.Repos.Matches, c.Repos.Audit))
		bot.RegisterCommand(whatsappbot.NewRejectCommand(c.Repos.Matches, c.Repos.Audit))
		bot.RegisterCommand(whatsappbot.NewHelpCommand())

		c.WAManager.SetBotHandler(bot)
		c.Logger.Info().
			Int("authorized_phones", len(c.Config.WhatsApp.BotCommands.AuthorizedPhones)).
			Msg("WhatsApp bot commands enabled")
	}

	// Wire Telegram alerts for WhatsApp connection failures
	if c.Config.Reports.Telegram.Enabled && c.Config.Reports.Telegram.BotToken != "" {
		telegramNotifier := notify.NewTelegramNotifier(notify.TelegramConfig{
			Enabled:  true,
			BotToken: c.Config.Reports.Telegram.BotToken,
			ChatIDs:  c.Config.Reports.Telegram.ChatIDs,
		}, c.Logger)
		alertAdapter := notify.NewTelegramAlertAdapter(telegramNotifier)
		c.WAManager.SetAlerter(alertAdapter)
		c.Logger.Info().Msg("WhatsApp admin alerts enabled via Telegram")
	}

	// Wire WhatsApp status to health endpoint
	c.Handlers.Health.SetWAStatusFunc(func() (state string, reconnectCount int, lastConnected time.Time, uptimeSeconds int64) {
		status := c.WAManager.GetConnectionStatus()
		return status.State.String(), status.ReconnectCount, status.LastConnectedAt, status.UptimeSeconds
	})

	// Start message feeding loop
	go func() {
		msgChan := listener.MessageChannel()
		for msg := range msgChan {
			c.Parser.ProcessMessage(context.Background(), msg)
		}
	}()

	// Start WhatsApp connection
	go func() {
		if err := c.WAManager.Connect(ctx); err != nil {
			c.Logger.Error().Err(err).Msg("WhatsApp connection error")
		}
	}()

	// Auto-sync groups after connection
	go c.autoSyncGroups(ctx)

	// Start parser
	c.Parser.Start(ctx)

	// Create and start HTTP server
	c.Server = &http.Server{
		Addr:    fmt.Sprintf(":%d", c.Config.Server.Port),
		Handler: c.Router,
	}

	go func() {
		c.Logger.Info().Int("port", c.Config.Server.Port).Msg("Starting HTTP server")
		if err := c.Server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			c.Logger.Fatal().Err(err).Msg("HTTP server error")
		}
	}()

	// Wait for shutdown signal
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	c.Logger.Info().Msg("Shutting down...")
	c.Shutdown()

	return nil
}

// Shutdown gracefully stops all services
func (c *Container) Shutdown() {
	c.Logger.Info().Msg("Shutting down services...")

	// Stop background services
	if c.Parser != nil {
		c.Parser.Stop()
	}
	if c.Janitor != nil {
		c.Janitor.Stop()
	}
	if c.ReportScheduler != nil {
		c.ReportScheduler.Stop()
	}
	if c.LearningScheduler != nil {
		c.LearningScheduler.Stop()
	}

	// Shutdown SSE hub (prevents goroutine leaks)
	if c.SSEHub != nil {
		c.SSEHub.Shutdown()
	}

	// Disconnect WhatsApp
	if c.WAManager != nil {
		c.WAManager.Disconnect()
	}

	// Graceful HTTP server shutdown with timeout
	if c.Server != nil {
		ctx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
		defer cancel()
		if err := c.Server.Shutdown(ctx); err != nil {
			c.Logger.Error().Err(err).Msg("HTTP server shutdown error")
			c.Server.Close() // Force close if graceful shutdown fails
		}
	}

	c.Logger.Info().Msg("All services stopped")
}

// Close cleans up all resources
func (c *Container) Close() error {
	var lastErr error
	for i := len(c.cleanups) - 1; i >= 0; i-- {
		if err := c.cleanups[i](); err != nil {
			lastErr = err
			c.Logger.Error().Err(err).Msg("Cleanup error")
		}
	}
	return lastErr
}

// autoSyncGroups syncs WhatsApp groups after connection
func (c *Container) autoSyncGroups(ctx context.Context) {
	defer func() {
		if r := recover(); r != nil {
			c.Logger.Error().Interface("panic", r).Msg("Panic in auto-sync goroutine")
		}
	}()

	timeout := time.After(autoSyncTimeout)
	for !c.WAManager.IsConnected() {
		select {
		case <-ctx.Done():
			return
		case <-timeout:
			c.Logger.Warn().Msg("Timeout waiting for WhatsApp connection - skipping auto-sync")
			return
		case <-time.After(autoSyncPollInterval):
		}
	}

	c.Logger.Info().Msg("Auto-syncing groups from WhatsApp...")
	if err := c.WAManager.SyncGroups(ctx, func(jid, name, desc string) error {
		return c.Repos.Groups.SaveFromSync(ctx, jid, name, desc)
	}); err != nil {
		c.Logger.Warn().Err(err).Msg("Failed to auto-sync groups")
	} else {
		c.Logger.Info().Msg("Groups synced successfully")
	}

	if len(c.Config.WhatsApp.MonitoredGroups) > 0 {
		enabled, err := c.Repos.Groups.EnableFromConfig(ctx, c.Config.WhatsApp.MonitoredGroups)
		if err != nil {
			c.Logger.Warn().Err(err).Msg("Failed to enable some groups from config")
		} else if enabled > 0 {
			c.Logger.Info().Int("count", enabled).Msg("Enabled groups from config")
		}
	}
}

// SeedMedications loads and seeds medication mappings if empty
func (c *Container) SeedMedications(ctx context.Context) error {
	count, err := c.Repos.Mappings.Count(ctx)
	if err != nil {
		return fmt.Errorf("checking medication count: %w", err)
	}
	if count > 0 {
		c.Logger.Debug().Int("count", count).Msg("Medications already seeded")
		return nil
	}

	meds, err := entity.LoadRichMedicationMappings("medications.json")
	if err != nil {
		c.Logger.Warn().Err(err).Msg("Failed to load medications.json - seeding skipped")
		return nil // Not a fatal error, file may not exist
	}

	c.Logger.Info().Int("count", len(meds)).Msg("Seeding medication mappings...")

	// Collect for batch embedding
	var arabics []string
	for _, m := range meds {
		arabics = append(arabics, m.ArabicName)
	}

	embeddings, err := c.AIProvider.EmbedBatch(ctx, arabics)
	if err != nil {
		c.Logger.Warn().Err(err).Msg("Batch embedding failed")
	}

	for i, m := range meds {
		mapping := &entity.MedicationMapping{
			ArabicName:  m.ArabicName,
			EnglishName: m.EnglishName,
			Synonyms:    m.Synonyms,
			CreatedAt:   time.Now(),
		}
		if embeddings != nil && i < len(embeddings) {
			mapping.Embedding = embeddings[i]
		}
		if err := c.Repos.Mappings.Save(ctx, mapping); err != nil {
			c.Logger.Warn().Err(err).Str("name", m.EnglishName).Msg("Failed to seed mapping")
		}
	}

	c.Logger.Info().Msg("Medication seeding complete")
	return nil
}

// SeedAdminPhone seeds admin phone from config if not set
func (c *Container) SeedAdminPhone(ctx context.Context) {
	cfg, err := c.Repos.Config.GetAll(ctx)
	if err != nil {
		return
	}
	if cfg.AdminPhone == "" && c.Config.Monitor.AdminPhone != "" {
		c.Logger.Info().Str("phone", c.Config.Monitor.AdminPhone).Msg("Seeding AdminPhone from config")
		c.Repos.Config.UpdateFromMap(ctx, map[string]any{"admin_phone": c.Config.Monitor.AdminPhone})
	}
}

// Helper functions

func slogFromZerolog(zlog zerolog.Logger) *slog.Logger {
	return slog.New(&zerologSlogHandler{zlog: zlog})
}

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

func (h *zerologSlogHandler) WithAttrs(attrs []slog.Attr) slog.Handler { return h }
func (h *zerologSlogHandler) WithGroup(name string) slog.Handler       { return h }

func updateLearningMetrics(scheduler *matching.LearningScheduler) {
	if scheduler == nil {
		return
	}
	status := scheduler.Status()
	if status.Enabled {
		metrics.LearningSchedulerEnabled.Set(1)
	} else {
		metrics.LearningSchedulerEnabled.Set(0)
	}
	if status.PendingApply != nil {
		metrics.PendingWeightsAvailable.Set(1)
	} else {
		metrics.PendingWeightsAvailable.Set(0)
	}
	if !status.LastRun.IsZero() {
		metrics.LastLearningJobTimestamp.Set(float64(status.LastRun.Unix()))
	}
	if status.LastMetrics != nil {
		metrics.ConfirmationRate.Set(status.LastMetrics.ConfirmationRate)
		metrics.ScoreSeparation.Set(status.LastMetrics.AvgScoreConfirmed - status.LastMetrics.AvgScoreRejected)
		metrics.FeedbackSamplesAnalyzed.Set(float64(status.LastMetrics.SampleSize))
	}
}

// Compile-time checks
var _ repository.OfferRepository = (*storageGorm.OfferRepo)(nil)
