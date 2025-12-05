package config

import (
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/spf13/viper"
)

// Config holds all application configuration
type Config struct {
	// WhatsApp settings for WhatsApp Web connection
	WhatsApp WhatsAppConfig `mapstructure:"whatsapp"`

	// Gemini AI settings for the language model
	Gemini GeminiConfig `mapstructure:"gemini"`

	// Parser settings for message processing and matching
	Parser ParserConfig `mapstructure:"parser"`

	// API settings for the HTTP server and handlers
	API APIConfig `mapstructure:"api"`

	// Database settings for SQLite storage
	Database DatabaseConfig `mapstructure:"database"`

	// Server settings for network configuration
	Server ServerConfig `mapstructure:"server"`
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
}

// DatabaseConfig configures SQLite database settings
type DatabaseConfig struct {
	// Path is the file path to the SQLite database.
	// Will be created if it doesn't exist.
	// Default: ./data/pharmabroker.db
	Path string `mapstructure:"path"`

	// EnableWAL enables Write-Ahead Logging mode for SQLite.
	// Improves concurrent read/write performance significantly.
	// Default: true
	EnableWAL bool `mapstructure:"enable_wal"`
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

// Load loads configuration from file, environment, and defaults.
// Priority order: Environment variables > config file > defaults
func Load() *Config {
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

	// Override API key from direct env var if not set via viper
	if cfg.Gemini.APIKey == "" {
		cfg.Gemini.APIKey = os.Getenv("GEMINI_API_KEY")
	}

	return cfg
}

// setDefaults configures all default values
func setDefaults(v *viper.Viper) {
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

	// Database defaults
	v.SetDefault("database.path", "./data/pharmabroker.db")
	v.SetDefault("database.enable_wal", true)

	// Server defaults
	v.SetDefault("server.port", 8080)
	v.SetDefault("server.health_port", 5050)
}

// loadFallback creates a config with hardcoded fallback values
// Used when viper fails to load
func loadFallback() *Config {
	return &Config{
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
		},
		Database: DatabaseConfig{
			Path:      "./data/pharmabroker.db",
			EnableWAL: true,
		},
		Server: ServerConfig{
			Port:       8080,
			HealthPort: 5050,
		},
	}
}

// Validate checks required configuration
func (c *Config) Validate() error {
	if c.Gemini.APIKey == "" {
		return ErrMissingAPIKey
	}
	return nil
}
