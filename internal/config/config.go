package config

import (
	"os"
	"strconv"
	"time"
)

// Config holds all application configuration
type Config struct {
	// WhatsApp settings
	WhatsApp WhatsAppConfig

	// Gemini AI settings
	Gemini GeminiConfig

	// Database settings
	Database DatabaseConfig

	// Server settings
	Server ServerConfig
}

type WhatsAppConfig struct {
	// SessionDir is where WhatsApp session data is stored
	SessionDir string
	// MonitoredGroups is a list of group JIDs to monitor (empty = discover all)
	MonitoredGroups []string
	// ReconnectDelay is the delay between reconnection attempts
	ReconnectDelay time.Duration
}

type GeminiConfig struct {
	// APIKey for Gemini API
	APIKey string
	// Model to use (e.g., "gemini-2.5-flash")
	Model string
	// MaxMessagesPerRequest for batching
	MaxMessagesPerRequest int
	// RateLimitPerHour configurable rate limit
	RateLimitPerHour int
	// RequestTimeout for API calls
	RequestTimeout time.Duration
}

type DatabaseConfig struct {
	// Path to SQLite database file
	Path string
	// EnableWAL enables Write-Ahead Logging for better concurrency
	EnableWAL bool
}

type ServerConfig struct {
	// Port for HTTP server
	Port int
	// HealthPort for health check endpoint
	HealthPort int
}

// Load loads configuration from environment variables with sensible defaults
func Load() *Config {
	return &Config{
		WhatsApp: WhatsAppConfig{
			SessionDir:      getEnv("WA_SESSION_DIR", "./data/whatsapp"),
			MonitoredGroups: getEnvSlice("WA_MONITORED_GROUPS", nil),
			ReconnectDelay:  getEnvDuration("WA_RECONNECT_DELAY", 5*time.Second),
		},
		Gemini: GeminiConfig{
			APIKey:                getEnv("GEMINI_API_KEY", ""),
			Model:                 getEnv("GEMINI_MODEL", "gemini-2.5-flash"),
			MaxMessagesPerRequest: getEnvInt("GEMINI_BATCH_SIZE", 10),
			RateLimitPerHour:      getEnvInt("GEMINI_RATE_LIMIT_HOUR", 100),
			RequestTimeout:        getEnvDuration("GEMINI_TIMEOUT", 30*time.Second),
		},
		Database: DatabaseConfig{
			Path:      getEnv("DB_PATH", "./data/pharmabroker.db"),
			EnableWAL: getEnvBool("DB_ENABLE_WAL", true),
		},
		Server: ServerConfig{
			Port:       getEnvInt("SERVER_PORT", 8080),
			HealthPort: getEnvInt("HEALTH_PORT", 5050),
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

// Helper functions
func getEnv(key, defaultVal string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return defaultVal
}

func getEnvInt(key string, defaultVal int) int {
	if val := os.Getenv(key); val != "" {
		if i, err := strconv.Atoi(val); err == nil {
			return i
		}
	}
	return defaultVal
}

func getEnvBool(key string, defaultVal bool) bool {
	if val := os.Getenv(key); val != "" {
		if b, err := strconv.ParseBool(val); err == nil {
			return b
		}
	}
	return defaultVal
}

func getEnvDuration(key string, defaultVal time.Duration) time.Duration {
	if val := os.Getenv(key); val != "" {
		if d, err := time.ParseDuration(val); err == nil {
			return d
		}
	}
	return defaultVal
}

func getEnvSlice(key string, defaultVal []string) []string {
	if val := os.Getenv(key); val != "" {
		// Split by comma for simple slice parsing
		var result []string
		for _, v := range splitAndTrim(val, ",") {
			if v != "" {
				result = append(result, v)
			}
		}
		if len(result) > 0 {
			return result
		}
	}
	return defaultVal
}

func splitAndTrim(s, sep string) []string {
	var result []string
	start := 0
	for i := 0; i < len(s); i++ {
		if i+len(sep) <= len(s) && s[i:i+len(sep)] == sep {
			result = append(result, trim(s[start:i]))
			start = i + len(sep)
		}
	}
	result = append(result, trim(s[start:]))
	return result
}

func trim(s string) string {
	start, end := 0, len(s)
	for start < end && (s[start] == ' ' || s[start] == '\t') {
		start++
	}
	for end > start && (s[end-1] == ' ' || s[end-1] == '\t') {
		end--
	}
	return s[start:end]
}
