package config

import (
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/joho/godotenv"
	"github.com/spf13/viper"
)

// Config holds all application configuration
type Config struct {
	// AI provider selection: "gemini" or "docker"
	AI AIConfig `mapstructure:"ai"`

	// WhatsApp settings for WhatsApp Web connection
	WhatsApp WhatsAppConfig `mapstructure:"whatsapp"`

	// Gemini AI settings for the language model (used when ai.provider = "gemini")
	Gemini GeminiConfig `mapstructure:"gemini"`

	// Docker Model Runner settings (used when ai.provider = "docker")
	DockerModel DockerModelConfig `mapstructure:"docker_model"`

	// Parser settings for message processing and matching
	Parser ParserConfig `mapstructure:"parser"`

	// API settings for the HTTP server and handlers
	API APIConfig `mapstructure:"api"`

	// Database settings for SQLite storage
	Database DatabaseConfig `mapstructure:"database"`

	// Monitor settings for system alerts
	Monitor MonitorConfig `mapstructure:"monitor"`

	// Server settings for network configuration
	Server ServerConfig `mapstructure:"server"`

	// Reports settings for automated match reports
	Reports ReportsConfig `mapstructure:"reports"`

	// AdaptiveLearning settings for automatic weight optimization
	AdaptiveLearning AdaptiveLearningConfig `mapstructure:"adaptive_learning"`

	// Experimental settings for feature flags during migration
	Experimental ExperimentalConfig `mapstructure:"experimental"`
}

// AIConfig selects which AI provider to use
type AIConfig struct {
	// Provider specifies which AI backend to use.
	// Options: "gemini" (Google Gemini API), "docker" (Docker Model Runner)
	// Default: gemini
	Provider string `mapstructure:"provider"`
}

// DockerModelConfig configures Docker Model Runner (OpenAI-compatible API)
type DockerModelConfig struct {
	// BaseURL is the Docker Model Runner endpoint URL.
	// When using Compose models, this is auto-injected via LLM_URL environment variable.
	// Default: http://localhost:12434/engines/llama.cpp/v1
	BaseURL string `mapstructure:"base_url"`

	// EmbeddingModelName is the name of the model to use for embeddings
	// e.g., "ai/embeddinggemma"
	EmbeddingModelName string `mapstructure:"embedding_model_name"`

	// Model is the model identifier (e.g., "ai/qwen3-vl:latest")
	// When using Compose models, this is auto-injected via LLM_MODEL environment variable.
	Model string `mapstructure:"model"`

	// MaxRetries is the number of retry attempts for failed API calls.
	MaxRetries int `mapstructure:"max_retries"`

	// RetryBaseDelay is the initial delay for exponential backoff retries.
	RetryBaseDelay time.Duration `mapstructure:"retry_base_delay"`

	// RequestTimeout is the maximum time to wait for a response.
	RequestTimeout time.Duration `mapstructure:"request_timeout"`

	// MaxMessageLines is the maximum number of lines in a message before splitting.
	// Default: 20
	MaxMessageLines int `mapstructure:"max_message_lines"`

	// Circuit Breaker Configuration
	// CBMaxRequests is the max number of requests in half-open state.
	// Default: 3
	CBMaxRequests uint32 `mapstructure:"cb_max_requests"`

	// CBInterval is the cyclic period of the closed state for clearing internal counts.
	// Default: 60s
	CBInterval time.Duration `mapstructure:"cb_interval"`

	// CBTimeout is the period of the open state before moving to half-open.
	// Default: 30s
	CBTimeout time.Duration `mapstructure:"cb_timeout"`

	// CBFailureRatio is the failure ratio to trip the circuit breaker.
	// Default: 0.6
	CBFailureRatio float64 `mapstructure:"cb_failure_ratio"`
}

// WhatsAppConfig configures the WhatsApp Web connection
type WhatsAppConfig struct {
	// SessionDir is the directory where WhatsApp session data (keys, credentials) is stored.
	// This allows persistent login without re-scanning QR code.
	// Default: ./data/whatsapp
	SessionDir string `mapstructure:"session_dir"`

	// MonitoredGroups is a list of group JIDs to monitor.
	// If empty, all groups are discovered and can be enabled via API.
	// Format: "1234567890@g.us"
	MonitoredGroups []string `mapstructure:"monitored_groups"`

	// ReconnectDelay is the time to wait before attempting to reconnect
	// after a connection failure or disconnect.
	// Default: 5s
	ReconnectDelay time.Duration `mapstructure:"reconnect_delay"`

	// OperationTimeout is the timeout for WhatsApp operations like
	// fetching groups or sending messages.
	// Default: 30s
	OperationTimeout time.Duration `mapstructure:"operation_timeout"`

	// BotCommands configures WhatsApp bot command handling
	BotCommands BotCommandsConfig `mapstructure:"bot_commands"`
}

// BotCommandsConfig configures WhatsApp bot commands
type BotCommandsConfig struct {
	// Enabled controls whether bot commands are processed
	// Default: false
	Enabled bool `mapstructure:"enabled"`

	// AuthorizedPhones is a list of phone numbers authorized to use bot commands
	// Format: "+201234567890" or "201234567890"
	AuthorizedPhones []string `mapstructure:"authorized_phones"`
}

// GeminiConfig configures the Google Gemini AI integration
type GeminiConfig struct {
	// APIKey is the Gemini API key from Google AI Studio.
	// REQUIRED - the application will not start without this.
	// Can be set via GEMINI_API_KEY environment variable.
	APIKey string `mapstructure:"api_key"`

	// Model specifies which Gemini model to use for parsing.
	// Recommended: "gemini-2.5-flash" for speed, "gemini-2.5-pro" for accuracy.
	// Default: gemini-2.5-flash
	Model string `mapstructure:"model"`

	// MaxMessagesPerRequest is the maximum number of messages to include
	// in a single API request. Larger batches are more efficient but
	// may hit token limits.
	// Default: 10
	MaxMessagesPerRequest int `mapstructure:"max_messages_per_request"`

	// RateLimitPerHour is the maximum number of API requests allowed per hour.
	// Gemini free tier allows ~60/min, but we limit to avoid hitting quotas.
	// Default: 100
	RateLimitPerHour int `mapstructure:"rate_limit_per_hour"`

	// RequestTimeout is the maximum time to wait for a Gemini API response.
	// Default: 30s
	RequestTimeout time.Duration `mapstructure:"request_timeout"`

	// MaxRetries is the number of retry attempts for failed API calls.
	// Uses exponential backoff: 1s, 2s, 4s, etc.
	// Default: 3
	MaxRetries int `mapstructure:"max_retries"`

	// RetryBaseDelay is the initial delay for exponential backoff retries.
	// Subsequent retries double this delay.
	// Default: 1s
	RetryBaseDelay time.Duration `mapstructure:"retry_base_delay"`
}

// ParserConfig configures the message parser and matching engine
type ParserConfig struct {
	// BatchInterval is how often the parser processes accumulated messages.
	// Shorter intervals mean faster processing, longer intervals mean more
	// efficient batching.
	// Default: 5s
	BatchInterval time.Duration `mapstructure:"batch_interval"`

	// MatchThreshold is the minimum similarity score (0.0-1.0) required
	// to create a match between an offer and request.
	// 0.5 = 50% similarity, 0.8 = 80% similarity
	// Default: 0.5
	MatchThreshold float64 `mapstructure:"match_threshold"`

	// MessageBufferSize is the channel buffer size for incoming messages.
	// Larger buffers handle bursts better but use more memory.
	// Default: 1000
	MessageBufferSize int `mapstructure:"message_buffer_size"`

	// MatchWorkerPoolSize controls concurrent match job processing.
	// Higher values process matches faster but use more resources.
	// Default: 5
	MatchWorkerPoolSize int `mapstructure:"match_worker_pool_size"`

	// CircuitBreakerThreshold is the number of consecutive AI failures
	// before the circuit breaker opens and requests are blocked.
	// Default: 5
	CircuitBreakerThreshold int `mapstructure:"circuit_breaker_threshold"`

	// CircuitBreakerResetTimeout is how long to wait before allowing
	// test requests after the circuit opens.
	// Default: 30s
	CircuitBreakerResetTimeout time.Duration `mapstructure:"circuit_breaker_reset_timeout"`
}

// APIConfig configures the HTTP API server and handlers
type APIConfig struct {
	// RequestTimeout is the context timeout for standard API requests.
	// Keep short to avoid hanging connections.
	// Default: 5s
	RequestTimeout time.Duration `mapstructure:"request_timeout"`

	// ExportTimeout is the timeout for export operations (CSV download).
	// Longer because exports may process large datasets.
	// Default: 30s
	ExportTimeout time.Duration `mapstructure:"export_timeout"`

	// SSEHeartbeat is the interval between Server-Sent Events heartbeat pings.
	// Keeps connections alive and detects disconnected clients.
	// Default: 30s
	SSEHeartbeat time.Duration `mapstructure:"sse_heartbeat"`

	// ConfigCacheTTL is how long to cache the dynamic config before re-reading
	// from the database. Reduces DB load for frequently checked settings.
	// Default: 30s
	ConfigCacheTTL time.Duration `mapstructure:"config_cache_ttl"`

	// DefaultPageLimit is the default number of items per page in list APIs
	// when no limit is specified in the request.
	// Default: 50
	DefaultPageLimit int `mapstructure:"default_page_limit"`

	// MaxExportRecords is the maximum number of records to include in exports.
	// Prevents memory issues with very large exports.
	// Default: 1000
	MaxExportRecords int `mapstructure:"max_export_records"`

	// MaxPageSize is the maximum allowed page size for pagination.
	// Requests exceeding this will be capped.
	// Default: 100
	MaxPageSize int `mapstructure:"max_page_size"`

	// RateLimitRPS is the maximum requests per second allowed per client.
	// Default: 10
	RateLimitRPS float64 `mapstructure:"rate_limit_rps"`

	// RateLimitBurst is the maximum burst size for rate limiting.
	// Default: 20
	RateLimitBurst int `mapstructure:"rate_limit_burst"`

	// MaxSSEClients is the maximum number of concurrent SSE connections.
	// Default: 100
	MaxSSEClients int `mapstructure:"max_sse_clients"`
}

// DatabaseConfig configures PostgreSQL database settings
type DatabaseConfig struct {
	// DSN is the PostgreSQL connection string.
	// Format: postgres://user:password@host:port/database?sslmode=disable
	// Default: postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable
	DSN string `mapstructure:"dsn"`

	// MaxOpenConns is the maximum number of open connections to the database.
	// Default: 25
	MaxOpenConns int `mapstructure:"max_open_conns"`

	// MaxIdleConns is the maximum number of idle connections in the pool.
	// Default: 5
	MaxIdleConns int `mapstructure:"max_idle_conns"`

	// ConnMaxLifetime is the maximum lifetime of a connection in minutes.
	// Default: 5
	ConnMaxLifetimeMins int `mapstructure:"conn_max_lifetime_mins"`

	// RawRetentionDays is the number of days to keep raw messages before archiving.
	// Default: 30
	RawRetentionDays int `mapstructure:"raw_retention_days"`
}

// MonitorConfig configures system monitoring and alerting
type MonitorConfig struct {
	// AdminPhone is the WhatsApp number to receive critical alerts.
	// Used as initial seed for the database configuration.
	AdminPhone string `mapstructure:"admin_phone"`
}

// ServerConfig configures network settings
type ServerConfig struct {
	// Port is the HTTP server port for the main API and dashboard.
	// Default: 8080
	Port int `mapstructure:"port"`

	// HealthPort is the port for the health check endpoint (/health).
	// Separate port allows health checks without exposing main API.
	// Default: 5050
	HealthPort int `mapstructure:"health_port"`
}

// ReportsConfig configures automated match reporting
type ReportsConfig struct {
	// Enabled controls whether scheduled reports are generated
	// Default: false
	Enabled bool `mapstructure:"enabled"`

	// IntervalMins is the interval between reports in minutes
	// Default: 60 (hourly)
	IntervalMins int `mapstructure:"interval_mins"`

	// MinScore is the minimum match score to include in reports
	// Default: 0.5
	MinScore float64 `mapstructure:"min_score"`

	// Limit is the maximum number of matches per report
	// Default: 100
	Limit int `mapstructure:"limit"`

	// Telegram Bot configuration
	Telegram TelegramNotifyConfig `mapstructure:"telegram"`

	// Email SMTP configuration
	Email EmailNotifyConfig `mapstructure:"email"`
}

// TelegramNotifyConfig configures Telegram notifications
type TelegramNotifyConfig struct {
	Enabled  bool     `mapstructure:"enabled"`
	BotToken string   `mapstructure:"bot_token"`
	ChatIDs  []string `mapstructure:"chat_ids"`
}

// EmailNotifyConfig configures email notifications
type EmailNotifyConfig struct {
	Enabled    bool     `mapstructure:"enabled"`
	SMTPHost   string   `mapstructure:"smtp_host"`
	SMTPPort   int      `mapstructure:"smtp_port"`
	Username   string   `mapstructure:"username"`
	Password   string   `mapstructure:"password"`
	FromName   string   `mapstructure:"from_name"`
	FromEmail  string   `mapstructure:"from_email"`
	Recipients []string `mapstructure:"recipients"`
}

// AdaptiveLearningConfig configures automatic weight optimization
type AdaptiveLearningConfig struct {
	// Enabled controls whether adaptive learning is active.
	// Default: false (manual management only)
	Enabled bool `mapstructure:"enabled"`

	// Schedule is a cron expression for when to run learning.
	// Examples: "0 3 * * *" (3 AM daily), "0 */6 * * *" (every 6 hours)
	// Default: "0 3 * * *"
	Schedule string `mapstructure:"schedule"`

	// Algorithm contains weight adjustment parameters
	Algorithm LearningAlgorithmConfig `mapstructure:"algorithm"`

	// AutoApply controls automatic weight application
	AutoApply AutoApplyConfig `mapstructure:"auto_apply"`

	// Notifications configures alerting for learning events
	Notifications LearningNotificationsConfig `mapstructure:"notifications"`
}

// LearningAlgorithmConfig configures the weight learning algorithm
type LearningAlgorithmConfig struct {
	// LearningRate controls how quickly weights adjust (alpha).
	// Lower values = slower but more stable learning.
	// Range: 0.01 to 0.5, Default: 0.1
	LearningRate float64 `mapstructure:"learning_rate"`

	// MinWeight is the minimum allowed weight for any factor.
	// Prevents factors from becoming irrelevant.
	// Range: 0.01 to 0.20, Default: 0.05
	MinWeight float64 `mapstructure:"min_weight"`

	// MaxWeight is the maximum allowed weight for any factor.
	// Prevents single factor dominance.
	// Range: 0.50 to 0.90, Default: 0.70
	MaxWeight float64 `mapstructure:"max_weight"`

	// MinChange is the minimum weight change to apply.
	// Changes smaller than this are ignored as noise.
	// Range: 0.005 to 0.10, Default: 0.02
	MinChange float64 `mapstructure:"min_change"`

	// MinSamples is the minimum feedback count required for learning.
	// Prevents learning from insufficient data.
	// Range: 10 to 1000, Default: 100
	MinSamples int `mapstructure:"min_samples"`

	// AnalysisWindowDays is how many days of feedback to analyze.
	// Default: 30
	AnalysisWindowDays int `mapstructure:"analysis_window_days"`
}

// AutoApplyConfig controls automatic weight application
type AutoApplyConfig struct {
	// Enabled controls whether new weights are applied automatically.
	// When false, weights are calculated but require manual approval.
	// Default: false (safer for initial deployment)
	Enabled bool `mapstructure:"enabled"`

	// RequireImprovement only applies weights if performance improves.
	// Default: true
	RequireImprovement bool `mapstructure:"require_improvement"`

	// MinSeparationGain is the minimum separation gain required.
	// Separation = avg_confirmed_score - avg_rejected_score
	// Default: 0.01
	MinSeparationGain float64 `mapstructure:"min_separation_gain"`

	// MaxConfirmationRateDrop is the maximum allowed drop in confirmation rate.
	// If confirmation rate drops more than this, weights are not applied.
	// Default: 0.05 (5%)
	MaxConfirmationRateDrop float64 `mapstructure:"max_confirmation_rate_drop"`
}

// LearningNotificationsConfig configures learning event notifications
type LearningNotificationsConfig struct {
	// OnSuccess notifies when weights are successfully applied.
	// Default: true
	OnSuccess bool `mapstructure:"on_success"`

	// OnFailure notifies when learning fails (errors, insufficient data).
	// Default: true
	OnFailure bool `mapstructure:"on_failure"`

	// OnRecommendation notifies when new weights are calculated but not applied.
	// Default: true
	OnRecommendation bool `mapstructure:"on_recommendation"`

	// LogLevel controls verbosity: "debug", "info", "warn", "error"
	// Default: "info"
	LogLevel string `mapstructure:"log_level"`
}

// ExperimentalConfig contains feature flags for gradual migration
type ExperimentalConfig struct {
	// UseNewRepos enables the new storage/gorm repository implementations.
	// When true, uses pharmabroker/storage/gorm repos instead of internal/storage.
	// Default: false (use legacy internal/storage repos)
	UseNewRepos bool `mapstructure:"use_new_repos"`
}

// Load loads configuration from file, environment, and defaults.
// Priority order: Environment variables > config file > defaults
func Load() *Config {
	// Load .env file if present (ignores error if not found)
	_ = godotenv.Load()

	v := viper.New()

	// Set config file information
	v.SetConfigName("config")
	v.SetConfigType("yaml")
	v.AddConfigPath(".")
	v.AddConfigPath("./config")
	v.AddConfigPath("/etc/pharmabroker")

	// Enable environment variable overrides
	v.SetEnvPrefix("PB") // PB_GEMINI_API_KEY, PB_SERVER_PORT, etc.
	v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	v.AutomaticEnv()

	// Set sensible defaults
	setDefaults(v)

	// Try to read config file (not required)
	if err := v.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			fmt.Printf("Warning: Error reading config file: %v\n", err)
		}
	}

	// Unmarshal to Config struct
	cfg := &Config{}
	if err := v.Unmarshal(cfg); err != nil {
		fmt.Printf("Warning: Error unmarshaling config: %v\n", err)
		return loadFallback()
	}

	// Override from direct environment variables (Docker Compose injects these)
	if cfg.Gemini.APIKey == "" {
		cfg.Gemini.APIKey = os.Getenv("GEMINI_API_KEY")
	}
	if cfg.DockerModel.BaseURL == "" {
		if url := os.Getenv("LLM_URL"); url != "" {
			cfg.DockerModel.BaseURL = url
		}
	}

	// Fallback defaulting logic for BaseURLs handled by viper/loadFallback already,
	// but if environment vars explicitly set empty string we might need care.
	// Here we only set if empty struct field AND env var exists.

	if cfg.DockerModel.Model == "" {
		if model := os.Getenv("LLM_MODEL"); model != "" {
			cfg.DockerModel.Model = model
		}
	}
	if cfg.DockerModel.EmbeddingModelName == "" {
		if model := os.Getenv("EMBEDDING_MODEL"); model != "" {
			cfg.DockerModel.EmbeddingModelName = model
		}
	}

	return cfg
}

// setDefaults configures all default values
func setDefaults(v *viper.Viper) {
	// AI provider defaults
	v.SetDefault("ai.provider", "gemini")

	// Docker Model Runner defaults
	v.SetDefault("docker_model.base_url", "http://model-runner.docker.internal/engines/llama.cpp/v1")
	v.SetDefault("docker_model.embedding_model_name", "ai/embeddinggemma")
	v.SetDefault("docker_model.model", "ai/qwen3-vl:latest")
	v.SetDefault("docker_model.max_retries", 3)
	v.SetDefault("docker_model.retry_base_delay", "1s")
	v.SetDefault("docker_model.request_timeout", "60s")
	v.SetDefault("docker_model.max_message_lines", 20)
	v.SetDefault("docker_model.cb_max_requests", 3)
	v.SetDefault("docker_model.cb_interval", "60s")
	v.SetDefault("docker_model.cb_timeout", "30s")
	v.SetDefault("docker_model.cb_failure_ratio", 0.6)

	// WhatsApp defaults
	v.SetDefault("whatsapp.session_dir", "./data/whatsapp")
	v.SetDefault("whatsapp.reconnect_delay", "5s")
	v.SetDefault("whatsapp.operation_timeout", "30s")

	// Gemini defaults
	v.SetDefault("gemini.model", "gemini-2.5-flash")
	v.SetDefault("gemini.max_messages_per_request", 10)
	v.SetDefault("gemini.rate_limit_per_hour", 100)
	v.SetDefault("gemini.request_timeout", "30s")
	v.SetDefault("gemini.max_retries", 3)
	v.SetDefault("gemini.retry_base_delay", "1s")

	// Parser defaults
	v.SetDefault("parser.batch_interval", "5s")
	v.SetDefault("parser.match_threshold", 0.5)
	v.SetDefault("parser.message_buffer_size", 1000)

	// API defaults
	v.SetDefault("api.request_timeout", "5s")
	v.SetDefault("api.export_timeout", "30s")
	v.SetDefault("api.sse_heartbeat", "30s")
	v.SetDefault("api.config_cache_ttl", "30s")
	v.SetDefault("api.default_page_limit", 50)
	v.SetDefault("api.max_export_records", 1000)
	v.SetDefault("api.max_page_size", 100)
	v.SetDefault("api.rate_limit_rps", 10.0)
	v.SetDefault("api.rate_limit_burst", 20)
	v.SetDefault("api.max_sse_clients", 100)

	// Database defaults
	v.SetDefault("database.path", "./data/pharmabroker.db")
	v.SetDefault("database.enable_wal", true)
	v.SetDefault("database.raw_retention_days", 30)
	v.SetDefault("database.archive_path", "./data/archive.db")
	v.SetDefault("database.max_read_conns", 5)

	// Server defaults
	v.SetDefault("server.port", 8080)
	v.SetDefault("server.health_port", 5050)

	// Adaptive Learning defaults (conservative for safe initial deployment)
	v.SetDefault("adaptive_learning.enabled", false)
	v.SetDefault("adaptive_learning.schedule", "0 3 * * *") // 3 AM daily

	// Algorithm defaults
	v.SetDefault("adaptive_learning.algorithm.learning_rate", 0.1)
	v.SetDefault("adaptive_learning.algorithm.min_weight", 0.05)
	v.SetDefault("adaptive_learning.algorithm.max_weight", 0.70)
	v.SetDefault("adaptive_learning.algorithm.min_change", 0.02)
	v.SetDefault("adaptive_learning.algorithm.min_samples", 100)
	v.SetDefault("adaptive_learning.algorithm.analysis_window_days", 30)

	// Auto-apply defaults (disabled by default for safety)
	v.SetDefault("adaptive_learning.auto_apply.enabled", false)
	v.SetDefault("adaptive_learning.auto_apply.require_improvement", true)
	v.SetDefault("adaptive_learning.auto_apply.min_separation_gain", 0.01)
	v.SetDefault("adaptive_learning.auto_apply.max_confirmation_rate_drop", 0.05)

	// Notification defaults
	v.SetDefault("adaptive_learning.notifications.on_success", true)
	v.SetDefault("adaptive_learning.notifications.on_failure", true)
	v.SetDefault("adaptive_learning.notifications.on_recommendation", true)
	v.SetDefault("adaptive_learning.notifications.log_level", "info")
}

// loadFallback creates a config with hardcoded fallback values
// Used when viper fails to load
func loadFallback() *Config {
	return &Config{
		AI: AIConfig{
			Provider: "gemini",
		},
		DockerModel: DockerModelConfig{
			BaseURL:            "http://localhost:12434/engines/llama.cpp/v1",
			EmbeddingModelName: "ai/embeddinggemma",
			Model:              "ai/qwen3-vl:latest",
			MaxRetries:         3,
			RetryBaseDelay:     1 * time.Second,
			RequestTimeout:     300 * time.Second,
			MaxMessageLines:    20,
			CBMaxRequests:      3,
			CBInterval:         60 * time.Second,
			CBTimeout:          30 * time.Second,
			CBFailureRatio:     0.6,
		},
		WhatsApp: WhatsAppConfig{
			SessionDir:       "./data/whatsapp",
			ReconnectDelay:   5 * time.Second,
			OperationTimeout: 30 * time.Second,
		},
		Gemini: GeminiConfig{
			APIKey:                os.Getenv("GEMINI_API_KEY"),
			Model:                 "gemini-2.5-flash",
			MaxMessagesPerRequest: 10,
			RateLimitPerHour:      100,
			RequestTimeout:        30 * time.Second,
			MaxRetries:            3,
			RetryBaseDelay:        1 * time.Second,
		},
		Parser: ParserConfig{
			BatchInterval:     5 * time.Second,
			MatchThreshold:    0.5,
			MessageBufferSize: 1000,
		},
		API: APIConfig{
			RequestTimeout:   5 * time.Second,
			ExportTimeout:    30 * time.Second,
			SSEHeartbeat:     30 * time.Second,
			ConfigCacheTTL:   30 * time.Second,
			DefaultPageLimit: 50,
			MaxExportRecords: 1000,
			MaxPageSize:      100,
			RateLimitRPS:     10.0,
			RateLimitBurst:   20,
			MaxSSEClients:    100,
		},
		Database: DatabaseConfig{
			DSN:                 "postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable",
			MaxOpenConns:        25,
			MaxIdleConns:        5,
			ConnMaxLifetimeMins: 5,
			RawRetentionDays:    30,
		},
		Server: ServerConfig{
			Port:       8080,
			HealthPort: 5050,
		},
	}
}

// Validate checks required configuration based on selected provider
func (c *Config) Validate() error {
	var errs []string

	// AI Provider validation
	switch c.AI.Provider {
	case "gemini":
		if c.Gemini.APIKey == "" {
			errs = append(errs, "gemini.api_key is required when using gemini provider")
		}
	case "docker":
		if c.DockerModel.BaseURL == "" {
			errs = append(errs, "docker_model.base_url is required when using docker provider")
		}
		if c.DockerModel.Model == "" {
			errs = append(errs, "docker_model.model is required when using docker provider")
		}
	case "":
		errs = append(errs, "ai.provider is required (use 'gemini' or 'docker')")
	default:
		errs = append(errs, fmt.Sprintf("unknown AI provider: %s (use 'gemini' or 'docker')", c.AI.Provider))
	}

	// Database validation
	if c.Database.DSN == "" {
		errs = append(errs, "database.dsn is required")
	}

	// Server port validation
	if c.Server.Port <= 0 || c.Server.Port > 65535 {
		errs = append(errs, fmt.Sprintf("server.port must be between 1 and 65535, got %d", c.Server.Port))
	}
	if c.Server.HealthPort != 0 && (c.Server.HealthPort <= 0 || c.Server.HealthPort > 65535) {
		errs = append(errs, fmt.Sprintf("server.health_port must be between 1 and 65535, got %d", c.Server.HealthPort))
	}

	// Parser threshold validation
	if c.Parser.MatchThreshold < 0 || c.Parser.MatchThreshold > 1 {
		errs = append(errs, fmt.Sprintf("parser.match_threshold must be between 0 and 1, got %.2f", c.Parser.MatchThreshold))
	}

	// API limits validation - set defaults if not configured
	if c.API.RateLimitRPS <= 0 {
		c.API.RateLimitRPS = 10.0 // Default if not set
	}
	if c.API.MaxPageSize <= 0 {
		c.API.MaxPageSize = 100 // Default if not set
	}

	// Return collected errors
	if len(errs) > 0 {
		return fmt.Errorf("configuration errors:\n  - %s", strings.Join(errs, "\n  - "))
	}
	return nil
}

// ValidateAndLog validates config and logs warnings for optional but recommended fields
func (c *Config) ValidateAndLog() error {
	if err := c.Validate(); err != nil {
		return err
	}

	// Log warnings for recommended but optional fields (caller should use these)
	var warnings []string

	if c.WhatsApp.SessionDir == "" {
		warnings = append(warnings, "whatsapp.session_dir not set - using default")
	}

	if c.Reports.Enabled && c.Reports.Telegram.BotToken == "" && !c.Reports.Email.Enabled {
		warnings = append(warnings, "reports.enabled but no notification channels configured")
	}

	// Return warnings as a special error type if needed (or nil)
	_ = warnings // Could be used for logging in caller
	return nil
}
