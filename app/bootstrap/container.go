// Package bootstrap provides dependency injection and application wiring.
package bootstrap

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"regexp"
	"syscall"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/ai"
	"pharmabroker/api"
	apiHandlers "pharmabroker/api/handlers"
	"pharmabroker/api/monitor"
	"pharmabroker/api/sse"
	_ "pharmabroker/bot/commands"
	"pharmabroker/bot/core"
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
	shutdownTimeout      = 10 * time.Second
	autoSyncTimeout      = 2 * time.Minute
	autoSyncPollInterval = 500 * time.Millisecond

	defaultConnectionStabilizationDelay = 2 * time.Second
	defaultMaxSyncAttempts              = 3
	defaultBackgroundSyncInterval       = 5 * time.Minute
	defaultMaxBackoff                   = 30 * time.Second
	defaultReportIntervalMins           = 60
	defaultSyncRetryBackoff             = 1 * time.Second
	backgroundSyncRetryAttempts         = 2
)

// Pre-compiled regex for DSN masking (performance optimization)
var dsnMaskRegex = regexp.MustCompile(`(postgres://[^:]+:)([^@]+)(@.+)`)

// Closer interface for cleanup resources
type Closer interface {
	Close() error
}

// Stoppable interface for services that can be stopped
type Stoppable interface {
	Stop()
}

// closerFunc wraps a function to implement Closer
type closerFunc func() error

func (f closerFunc) Close() error { return f() }

// SeedResult contains the result of medication seeding operation
type SeedResult struct {
	Total   int
	Seeded  int
	Failed  int
	Skipped bool
	Errors  []error
}

// ContainerOptions provides optional configuration for Container creation
type ContainerOptions struct {
	DBFactory func(*storageGorm.Config) (*storageGorm.DB, error)
}

// ServiceRegistry holds core application services
type ServiceRegistry struct {
	AIProvider         ai.Provider
	Parser             *parsing.Parser
	WAManager          *whatsapp.Manager
	SSEHub             *sse.SSEHub
	SequencedSSEHub    *sse.SequencedSSEHub
	SSEHealthMonitor   *sse.ClientHealthMonitor
	SSESubscriptionMgr *sse.SubscriptionManager
	SSEAuthHub         *sse.AuthenticatedSSEHub
	SSETokenValidator  *sse.HMACTokenValidator
	WarRoom            *monitor.WarRoom
	ABTestManager      *matching.ABTestManager
	WarmStartManager   *matching.WarmStartManager
	OutlierDetector    *matching.OutlierDetector
}

// SchedulerRegistry holds background schedulers
type SchedulerRegistry struct {
	Learning *matching.LearningScheduler
	Report   *reports.Scheduler
	Janitor  *janitor.Janitor
}

// Repositories bundles all repository implementations
type Repositories struct {
	Offers      repository.OfferRepository
	Requests    repository.RequestRepository
	Matches     repository.MatchRepository
	Groups      repository.GroupRepository
	Stats       repository.StatsRepository
	Messages    repository.RawMessageRepository
	Mappings    repository.MedicationMappingRepository
	Unmapped    repository.UnmappedMedicationRepo
	Queue       repository.MatchQueueRepository
	Review      repository.ReviewQueueRepository
	Config      repository.ConfigRepository
	Audit       repository.AuditRepository
	Feedback    repository.FeedbackRepository
	Leaderboard repository.LeaderboardRepository
	BotUsers    repository.BotUserRepository
}

// Container holds all application dependencies
type Container struct {
	Config *config.Config
	Logger zerolog.Logger

	DB         *storageGorm.DB
	Repos      *Repositories
	Services   *ServiceRegistry
	Schedulers *SchedulerRegistry

	Handlers *api.Handlers
	Router   http.Handler
	Server   *http.Server

	closers []Closer
}

// New creates a new application container with database and repositories
func New(ctx context.Context, cfg *config.Config, log zerolog.Logger) (*Container, error) {
	return NewWithOptions(ctx, cfg, log, nil)
}

// NewWithOptions creates a new application container with optional configuration
func NewWithOptions(ctx context.Context, cfg *config.Config, log zerolog.Logger, opts *ContainerOptions) (*Container, error) {
	c := &Container{
		Config:     cfg,
		Logger:     log,
		Services:   &ServiceRegistry{},
		Schedulers: &SchedulerRegistry{},
	}

	dbFactory := storageGorm.NewDB
	if opts != nil && opts.DBFactory != nil {
		dbFactory = opts.DBFactory
	}

	db, err := dbFactory(&storageGorm.Config{
		DSN:             cfg.Database.DSN,
		MaxOpenConns:    cfg.Database.MaxOpenConns,
		MaxIdleConns:    cfg.Database.MaxIdleConns,
		ConnMaxLifetime: time.Duration(cfg.Database.ConnMaxLifetimeMins) * time.Minute,
	})
	if err != nil {
		return nil, fmt.Errorf("database init: %w", err)
	}
	c.DB = db
	c.addCloser(closerFunc(db.Close))

	log.Info().Str("dsn", maskDSN(cfg.Database.DSN)).Msg("Database initialized")

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
		BotUsers:    storageGorm.NewBotUserRepo(db),
	}

	log.Info().Msg("Repositories initialized")
	return c, nil
}

// addCloser registers a resource for cleanup
func (c *Container) addCloser(closer Closer) {
	c.closers = append(c.closers, closer)
}

// InitServices initializes all core services in the correct order
func (c *Container) InitServices(ctx context.Context) error {
	c.InitSSE()
	c.InitWarRoom()

	if err := c.InitAI(ctx); err != nil {
		return fmt.Errorf("AI init: %w", err)
	}

	if err := c.InitWhatsApp(ctx); err != nil {
		return fmt.Errorf("WhatsApp init: %w", err)
	}

	if err := c.InitParser(ctx); err != nil {
		return fmt.Errorf("parser init: %w", err)
	}

	return nil
}

// InitAI initializes the AI provider
func (c *Container) InitAI(ctx context.Context) error {
	provider, err := ai.NewAIProvider(ctx, c.Config, c.Logger)
	if err != nil {
		return fmt.Errorf("AI provider: %w", err)
	}
	c.Services.AIProvider = provider
	c.Logger.Info().Str("provider", c.Config.AI.Provider).Msg("AI provider initialized")

	mappings, err := c.Repos.Mappings.GetAll(ctx)
	if err != nil {
		return fmt.Errorf("loading medication mappings: %w", err)
	}
	c.Services.AIProvider.SetMappings(mappings)
	c.Logger.Info().Int("count", len(mappings)).Msg("Configured hybrid RAG filtering")

	c.Services.AIProvider.SetUnmappedRepo(c.Repos.Unmapped)
	return nil
}

// InitWhatsApp initializes the WhatsApp manager
func (c *Container) InitWhatsApp(ctx context.Context) error {
	manager, err := whatsapp.NewManager(ctx, &c.Config.WhatsApp, c.Logger)
	if err != nil {
		return fmt.Errorf("WhatsApp manager: %w", err)
	}
	c.Services.WAManager = manager
	c.Logger.Info().Msg("WhatsApp manager initialized")
	return nil
}

// InitSSE initializes the SSE hub with enhanced features
func (c *Container) InitSSE() {
	// Initialize basic SSE hub (for backward compatibility)
	c.Services.SSEHub = sse.NewSSEHub()

	// Initialize sequenced SSE hub with event persistence
	c.Services.SequencedSSEHub = sse.NewSequencedSSEHub(
		sse.DefaultMaxClients,
		1000, // Event log capacity
		c.Logger,
	)

	// Initialize client health monitor for slow client detection
	c.Services.SSEHealthMonitor = sse.NewClientHealthMonitor(
		sse.DefaultClientHealthConfig(),
		c.Logger,
	)
	c.Services.SSEHealthMonitor.Start()

	// Initialize subscription manager for event filtering
	c.Services.SSESubscriptionMgr = sse.NewSubscriptionManager(c.Logger)

	// Initialize SSE authentication
	sseSecret := c.Config.API.JWT.Secret
	if sseSecret == "" {
		sseSecret = "pharmabroker-sse-default-secret" // Fallback for dev
		c.Logger.Warn().Msg("Using default SSE secret - set API.JWT.Secret in production")
	}
	c.Services.SSETokenValidator = sse.NewHMACTokenValidator(sseSecret, c.Logger)

	// Create authenticated SSE hub
	authConfig := sse.DefaultAuthConfig()
	authConfig.Enabled = c.Config.API.JWT.Enabled // Use JWT enabled flag for SSE auth
	c.Services.SSEAuthHub = sse.NewAuthenticatedSSEHub(
		c.Services.SSEHub,
		c.Services.SSETokenValidator,
		authConfig,
		c.Logger,
	)

	c.Logger.Info().
		Bool("auth_enabled", authConfig.Enabled).
		Msg("SSE hub initialized with sequencing, health monitoring, subscriptions, and authentication")
}

// InitWarRoom initializes the monitoring/alerting system
func (c *Container) InitWarRoom() {
	c.Services.WarRoom = monitor.NewWarRoom(c.Services.WAManager, c.Repos.Config, c.Logger)
	c.Logger.Info().Msg("WarRoom monitor initialized")
}

// InitParser initializes the message parser
func (c *Container) InitParser(ctx context.Context) error {
	if c.Services.AIProvider == nil {
		return errors.New("AI provider must be initialized before parser")
	}
	if c.Services.SSEHub == nil {
		return errors.New("SSE hub must be initialized before parser")
	}

	c.Services.Parser = parsing.NewParser(
		c.Repos.Messages,
		c.Services.AIProvider,
		c.Repos.Offers,
		c.Repos.Requests,
		c.Repos.Matches,
		c.Repos.Mappings,
		c.Repos.Queue,
		c.Repos.Config,
		c.Services.WarRoom,
		c.Services.SSEHub,
		c.Logger,
	)

	c.Services.Parser.SetReviewQueueRepo(c.Repos.Review)

	// Use background context to avoid capturing a potentially cancelled context
	c.Services.Parser.SetAutoParseChecker(func() bool {
		cfg, err := c.Repos.Config.GetAll(context.Background())
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
	matchHandler := apiHandlers.NewMatchHandler(c.Repos.Matches, c.Repos.Offers, c.Repos.Requests, c.Repos.Audit, c.Repos.Feedback, c.Services.SSEHub, c.Logger)
	groupHandler := apiHandlers.NewGroupHandler(c.Repos.Groups, c.Logger)
	statsHandler := apiHandlers.NewStatsHandler(c.Repos.Stats, c.Logger)
	configHandler := apiHandlers.NewConfigHandler(c.Repos.Config, c.Logger)
	feedbackHandler := apiHandlers.NewFeedbackHandler(c.Repos.Feedback, c.Repos.Matches, c.Logger)
	leaderboardHandler := apiHandlers.NewLeaderboardHandler(c.Repos.Leaderboard, c.Logger)
	auditHandler := apiHandlers.NewAuditHandler(c.Repos.Audit, c.Logger)
	healthChecker := apiHandlers.NewHealthChecker()

	groupHandler.SetSyncFunc(func() error {
		return c.Services.WAManager.SyncGroups(context.Background(), func(jid, name, desc string) error {
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
		SSE:         c.Services.SSEHub,
		Health:      healthChecker,
	}

	c.Logger.Info().Msg("API handlers initialized")
	return nil
}

// InitRouter creates the HTTP router
func (c *Container) InitRouter(ctx context.Context) {
	router, resources := api.NewGinRouter(ctx, c.Handlers, &c.Config.API, c.Logger)
	c.Router = router

	// Register server resources for cleanup
	c.addCloser(closerFunc(func() error {
		resources.Stop()
		return nil
	}))

	c.Logger.Info().Msg("Gin HTTP router initialized")
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

	// Initialize A/B Test Manager
	baseWeights := scorer.GetWeights()
	c.Services.ABTestManager = matching.NewABTestManager(baseWeights, c.Logger)
	c.Logger.Info().Msg("A/B Test Manager initialized")

	// Initialize Warm Start Manager for cold start handling
	c.Services.WarmStartManager = matching.NewWarmStartManager(
		matching.DefaultWarmStartConfig(),
		c.Logger,
	)
	c.Logger.Info().Msg("Warm Start Manager initialized")

	// Initialize Outlier Detector for feedback filtering
	c.Services.OutlierDetector = matching.NewOutlierDetector(
		matching.DefaultOutlierDetectorConfig(),
		c.Logger,
	)
	c.Logger.Info().Msg("Outlier Detector initialized")

	c.Schedulers.Learning = matching.NewLearningScheduler(
		learner,
		c.Config.AdaptiveLearning,
		slogFromZerolog(c.Logger),
	)

	if err := c.Schedulers.Learning.Start(); err != nil {
		return fmt.Errorf("learning scheduler start: %w", err)
	}

	publicScheduler := ai.WrapLearningScheduler(c.Schedulers.Learning)
	learningHandler := apiHandlers.NewLearningHandler(
		publicScheduler,
		feedbackRecordRepo,
		weightHistoryRepo,
		c.Logger,
	)
	c.Handlers.Learning = learningHandler

	updateLearningMetrics(c.Schedulers.Learning)

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

	c.Schedulers.Report = reports.NewScheduler(reportGenerator, notifier, schedulerConfig, reportConfig, c.Logger)
	if err := c.Schedulers.Report.Start(ctx); err != nil {
		return fmt.Errorf("report scheduler start: %w", err)
	}

	c.Logger.Info().Int("interval_mins", schedulerConfig.IntervalMins).Msg("Report scheduler started")
	return nil
}

// InitJanitor starts the data archival service
func (c *Container) InitJanitor() error {
	c.Schedulers.Janitor = janitor.NewJanitor(c.Repos.Messages, c.Config.Database, c.Logger)
	if err := c.Schedulers.Janitor.Start(); err != nil {
		return fmt.Errorf("janitor start: %w", err)
	}
	c.Logger.Info().Msg("Janitor service started")
	return nil
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

// recoverAndLog handles panic recovery with logging
func (c *Container) recoverAndLog(goroutineName string) {
	if r := recover(); r != nil {
		c.Logger.Error().
			Interface("panic", r).
			Str("goroutine", goroutineName).
			Msg("Panic recovered")
	}
}

// Run starts all services and blocks until shutdown signal
func (c *Container) Run(ctx context.Context) error {
	listener := whatsapp.NewListener(c.Logger, c.Repos.Messages, c.Repos.Groups)
	c.Services.WAManager.RegisterHandler(listener)

	listener.SetSkipOwnMessagesChecker(func() bool {
		cfg, err := c.Repos.Config.GetAll(context.Background())
		if err != nil {
			return true
		}
		return cfg.SkipOwnMessages
	})

	if c.Config.WhatsApp.BotCommands.Enabled {
		c.setupBotCommands()
	}

	c.setupTelegramAlerts()
	c.wireHealthEndpoint()

	listener.SetMessageHandler(func(ctx context.Context, msg *entity.RawMessage) error {
		c.Services.Parser.ProcessMessage(ctx, msg)
		return nil
	})
	listener.StartQueue()

	go func() {
		if err := c.Services.WAManager.Connect(ctx); err != nil {
			c.Logger.Error().Err(err).Msg("WhatsApp connection error")
		}
	}()

	go c.autoSyncGroups(ctx)

	c.Services.Parser.Start(ctx)

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

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	c.Logger.Info().Msg("Shutting down...")
	c.Shutdown()

	return nil
}

// setupBotCommands configures WhatsApp bot commands
func (c *Container) setupBotCommands() {
	bot := whatsappbot.NewBot(whatsappbot.Config{
		AuthorizedPhones: c.Config.WhatsApp.BotCommands.AuthorizedPhones,
	}, c.Logger)

	deps := core.Dependencies{
		Stats:    c.Repos.Stats,
		Matches:  c.Repos.Matches,
		Offers:   c.Repos.Offers,
		Requests: c.Repos.Requests,
		Groups:   c.Repos.Groups,
		Audit:    c.Repos.Audit,
	}

	for _, handler := range core.BuildCommands(deps) {
		bot.RegisterCommand(handler)
	}

	c.Services.WAManager.SetBotHandler(bot)
	c.Logger.Info().
		Int("authorized_phones", len(c.Config.WhatsApp.BotCommands.AuthorizedPhones)).
		Msg("WhatsApp bot commands enabled")
}

// setupTelegramAlerts wires Telegram alerts for WhatsApp connection failures
func (c *Container) setupTelegramAlerts() {
	if !c.Config.Reports.Telegram.Enabled || c.Config.Reports.Telegram.BotToken == "" {
		return
	}

	telegramNotifier := notify.NewTelegramNotifier(notify.TelegramConfig{
		Enabled:  true,
		BotToken: c.Config.Reports.Telegram.BotToken,
		ChatIDs:  c.Config.Reports.Telegram.ChatIDs,
	}, c.Logger)
	alertAdapter := notify.NewTelegramAlertAdapter(telegramNotifier)
	c.Services.WAManager.SetAlerter(alertAdapter)
	c.Logger.Info().Msg("WhatsApp admin alerts enabled via Telegram")
}

// wireHealthEndpoint connects WhatsApp status to health endpoint
func (c *Container) wireHealthEndpoint() {
	c.Handlers.Health.SetWAStatusFunc(func() (state string, reconnectCount int, lastConnected time.Time, uptimeSeconds int64) {
		status := c.Services.WAManager.GetConnectionStatus()
		return status.State.String(), status.ReconnectCount, status.LastConnectedAt, status.UptimeSeconds
	})
}

// Shutdown gracefully stops all services
func (c *Container) Shutdown() {
	c.Logger.Info().Msg("Shutting down services...")

	// Stop all stoppable services (check nil before passing to avoid nil interface issue)
	if c.Services.Parser != nil {
		c.stopService(c.Services.Parser)
	}
	if c.Schedulers.Janitor != nil {
		c.stopService(c.Schedulers.Janitor)
	}
	if c.Schedulers.Report != nil {
		c.stopService(c.Schedulers.Report)
	}
	if c.Schedulers.Learning != nil {
		c.stopService(c.Schedulers.Learning)
	}

	// SSEHub has Shutdown() instead of Stop()
	if c.Services.SSEHub != nil {
		c.Services.SSEHub.Shutdown()
	}

	// Sequenced SSE Hub shutdown
	if c.Services.SequencedSSEHub != nil {
		c.Services.SequencedSSEHub.Shutdown()
	}

	// SSE Health Monitor stop
	if c.Services.SSEHealthMonitor != nil {
		c.Services.SSEHealthMonitor.Stop()
	}

	// WAManager has Disconnect() instead of Stop()
	if c.Services.WAManager != nil {
		c.Services.WAManager.Disconnect()
	}

	if c.Server != nil {
		ctx, cancel := context.WithTimeout(context.Background(), shutdownTimeout)
		defer cancel()
		if err := c.Server.Shutdown(ctx); err != nil {
			c.Logger.Error().Err(err).Msg("HTTP server shutdown error")
			c.Server.Close()
		}
	}

	c.Logger.Info().Msg("All services stopped")
}

// stopService stops a single Stoppable service
func (c *Container) stopService(svc Stoppable) {
	svc.Stop()
}

// Close cleans up all resources in reverse order
func (c *Container) Close() error {
	var errs []error
	for i := len(c.closers) - 1; i >= 0; i-- {
		if err := c.closers[i].Close(); err != nil {
			errs = append(errs, err)
			c.Logger.Error().Err(err).Msg("Cleanup error")
		}
	}
	return errors.Join(errs...)
}

// autoSyncGroups syncs WhatsApp groups after connection with retry logic
func (c *Container) autoSyncGroups(ctx context.Context) {
	defer c.recoverAndLog("auto-sync-groups")

	if !c.waitForConnection(ctx) {
		return
	}

	c.Logger.Debug().Dur("delay", defaultConnectionStabilizationDelay).Msg("Waiting for connection to stabilize...")
	time.Sleep(defaultConnectionStabilizationDelay)

	c.Logger.Info().Msg("Auto-syncing groups from WhatsApp...")
	groupCount, err := c.syncGroupsWithRetry(ctx, defaultMaxSyncAttempts)
	if err != nil {
		c.Logger.Error().Err(err).Msg("Initial group sync failed - scheduling background retry")
		metrics.WhatsAppGroupSyncFailure.WithLabelValues("max_retries").Inc()
		go c.scheduleBackgroundSync(ctx)
		return
	}

	c.Logger.Info().Int("groups", groupCount).Msg("Groups synced successfully")
	metrics.WhatsAppGroupSyncSuccess.Inc()
	metrics.WhatsAppGroupsSynced.Set(float64(groupCount))

	c.enableConfiguredGroups(ctx)
}

// waitForConnection waits for WhatsApp connection with timeout
func (c *Container) waitForConnection(ctx context.Context) bool {
	timeout := time.After(autoSyncTimeout)
	for !c.Services.WAManager.IsConnected() {
		select {
		case <-ctx.Done():
			metrics.WhatsAppGroupSyncFailure.WithLabelValues("cancelled").Inc()
			return false
		case <-timeout:
			c.Logger.Warn().Msg("Timeout waiting for WhatsApp connection - skipping auto-sync")
			metrics.WhatsAppGroupSyncFailure.WithLabelValues("timeout").Inc()
			return false
		case <-time.After(autoSyncPollInterval):
		}
	}
	return true
}

// syncGroupsWithRetry attempts to sync groups with exponential backoff
func (c *Container) syncGroupsWithRetry(ctx context.Context, maxAttempts int) (int, error) {
	var lastErr error
	backoff := defaultSyncRetryBackoff

	for attempt := 1; attempt <= maxAttempts; attempt++ {
		start := time.Now()

		var groupCount int
		err := c.Services.WAManager.SyncGroups(ctx, func(jid, name, desc string) error {
			groupCount++
			return c.Repos.Groups.SaveFromSync(ctx, jid, name, desc)
		})

		metrics.WhatsAppGroupSyncDuration.Observe(time.Since(start).Seconds())

		if err == nil {
			return groupCount, nil
		}

		lastErr = err
		c.Logger.Warn().
			Err(err).
			Int("attempt", attempt).
			Int("max_attempts", maxAttempts).
			Dur("next_backoff", backoff).
			Msg("Group sync failed, retrying...")

		metrics.WhatsAppGroupSyncFailure.WithLabelValues("transient").Inc()

		if attempt < maxAttempts {
			select {
			case <-ctx.Done():
				return 0, ctx.Err()
			case <-time.After(backoff):
				backoff *= 2
				if backoff > defaultMaxBackoff {
					backoff = defaultMaxBackoff
				}
			}
		}
	}

	return 0, fmt.Errorf("group sync failed after %d attempts: %w", maxAttempts, lastErr)
}

// scheduleBackgroundSync retries group sync periodically until success
func (c *Container) scheduleBackgroundSync(ctx context.Context) {
	defer c.recoverAndLog("background-sync")

	c.Logger.Info().Dur("interval", defaultBackgroundSyncInterval).Msg("Background group sync scheduled")

	ticker := time.NewTicker(defaultBackgroundSyncInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			c.Logger.Debug().Msg("Background sync cancelled")
			return
		case <-ticker.C:
			if !c.Services.WAManager.IsConnected() {
				c.Logger.Debug().Msg("WhatsApp not connected - skipping background sync")
				continue
			}

			c.Logger.Info().Msg("Attempting background group sync...")
			groupCount, err := c.syncGroupsWithRetry(ctx, backgroundSyncRetryAttempts)
			if err == nil {
				c.Logger.Info().Int("groups", groupCount).Msg("Background group sync successful")
				metrics.WhatsAppGroupSyncSuccess.Inc()
				metrics.WhatsAppGroupsSynced.Set(float64(groupCount))
				c.enableConfiguredGroups(ctx)
				return
			}
			c.Logger.Warn().Err(err).Msg("Background sync failed - will retry")
		}
	}
}

// enableConfiguredGroups enables groups from config after sync
func (c *Container) enableConfiguredGroups(ctx context.Context) {
	if len(c.Config.WhatsApp.MonitoredGroups) == 0 {
		return
	}

	enabled, err := c.Repos.Groups.EnableFromConfig(ctx, c.Config.WhatsApp.MonitoredGroups)
	if err != nil {
		c.Logger.Warn().Err(err).Msg("Failed to enable some groups from config")
	} else if enabled > 0 {
		c.Logger.Info().Int("count", enabled).Msg("Enabled groups from config")
	}
}

// SeedMedications loads and seeds medication mappings if empty
func (c *Container) SeedMedications(ctx context.Context) (*SeedResult, error) {
	result := &SeedResult{}

	count, err := c.Repos.Mappings.Count(ctx)
	if err != nil {
		return nil, fmt.Errorf("checking medication count: %w", err)
	}
	if count > 0 {
		c.Logger.Debug().Int("count", count).Msg("Medications already seeded")
		result.Skipped = true
		return result, nil
	}

	meds, err := entity.LoadRichMedicationMappings("medications.json")
	if err != nil {
		c.Logger.Warn().Err(err).Msg("Failed to load medications.json - seeding skipped")
		result.Skipped = true
		return result, nil
	}

	result.Total = len(meds)
	c.Logger.Info().Int("count", len(meds)).Msg("Seeding medication mappings...")

	var arabics []string
	for _, m := range meds {
		arabics = append(arabics, m.ArabicName)
	}

	embeddings, err := c.Services.AIProvider.EmbedBatch(ctx, arabics)
	if err != nil {
		c.Logger.Warn().Err(err).Msg("Batch embedding failed - continuing without embeddings")
		result.Errors = append(result.Errors, fmt.Errorf("batch embedding: %w", err))
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
			result.Failed++
			result.Errors = append(result.Errors, fmt.Errorf("save %s: %w", m.EnglishName, err))
		} else {
			result.Seeded++
		}
	}

	c.Logger.Info().
		Int("seeded", result.Seeded).
		Int("failed", result.Failed).
		Msg("Medication seeding complete")

	return result, nil
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
	return slog.New(newZerologSlogHandler(zlog))
}

// zerologSlogHandler adapts zerolog to slog.Handler interface
type zerologSlogHandler struct {
	zlog  zerolog.Logger
	attrs []slog.Attr
	group string
}

func newZerologSlogHandler(zlog zerolog.Logger) *zerologSlogHandler {
	return &zerologSlogHandler{zlog: zlog}
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

	// Add pre-configured attrs
	for _, attr := range h.attrs {
		event = event.Interface(attr.Key, attr.Value.Any())
	}

	// Add record attrs
	record.Attrs(func(attr slog.Attr) bool {
		key := attr.Key
		if h.group != "" {
			key = h.group + "." + key
		}
		event = event.Interface(key, attr.Value.Any())
		return true
	})

	event.Msg(record.Message)
	return nil
}

func (h *zerologSlogHandler) WithAttrs(attrs []slog.Attr) slog.Handler {
	newHandler := &zerologSlogHandler{
		zlog:  h.zlog,
		attrs: make([]slog.Attr, len(h.attrs)+len(attrs)),
		group: h.group,
	}
	copy(newHandler.attrs, h.attrs)
	copy(newHandler.attrs[len(h.attrs):], attrs)
	return newHandler
}

func (h *zerologSlogHandler) WithGroup(name string) slog.Handler {
	newGroup := name
	if h.group != "" {
		newGroup = h.group + "." + name
	}
	return &zerologSlogHandler{
		zlog:  h.zlog,
		attrs: h.attrs,
		group: newGroup,
	}
}

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

// maskDSN masks the password in a PostgreSQL DSN for safe logging
func maskDSN(dsn string) string {
	return dsnMaskRegex.ReplaceAllString(dsn, "${1}***${3}")
}

// Compile-time checks
var _ repository.OfferRepository = (*storageGorm.OfferRepo)(nil)
