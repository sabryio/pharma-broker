// Package config provides configuration loading for the bridge using Viper.
package config

import (
	"fmt"
	"strings"
	"time"

	"github.com/rs/zerolog/log"
	"github.com/spf13/viper"
)

// Config holds all bridge configuration.
type Config struct {
	LogLevel    string            `mapstructure:"log_level"`
	GRPC        GRPCConfig        `mapstructure:"grpc"`
	WhatsApp    WhatsAppConfig    `mapstructure:"whatsapp"`
	HTTP        HTTPConfig        `mapstructure:"http"`
	Processing  ProcessingConfig  `mapstructure:"processing"`
	Resilience  ResilienceConfig  `mapstructure:"resilience"`
	RateLimit   RateLimitConfig   `mapstructure:"rate_limit"`
	GroupSync   GroupSyncConfig   `mapstructure:"group_sync"`
	Dedup       DedupConfig       `mapstructure:"dedup"`
	HistorySync HistorySyncConfig `mapstructure:"history_sync"`
	Reconnector ReconnectorConfig `mapstructure:"reconnector"`
}

// GRPCConfig holds gRPC client configuration.
type GRPCConfig struct {
	CoreAddr       string        `mapstructure:"core_addr"`
	ConnectTimeout time.Duration `mapstructure:"connect_timeout"`
}

// WhatsAppConfig holds WhatsApp client configuration.
type WhatsAppConfig struct {
	StorePath  string        `mapstructure:"store_path"`
	QRRetries  int           `mapstructure:"qr_retries"`
	QRTimeout  time.Duration `mapstructure:"qr_timeout"`
	QRTerminal bool          `mapstructure:"qr_terminal"`
}

// HTTPConfig holds HTTP server configuration.
type HTTPConfig struct {
	Port string `mapstructure:"port"`
	Mode string `mapstructure:"mode"`
}

// ProcessingConfig holds message processing configuration.
type ProcessingConfig struct {
	SkipOwnMessages bool `mapstructure:"skip_own_messages"`
	WorkerCount     int  `mapstructure:"worker_count"`
	WorkerQueueSize int  `mapstructure:"worker_queue_size"`
}

// ResilienceConfig holds resilience configuration.
type ResilienceConfig struct {
	CircuitBreaker CircuitBreakerConfig `mapstructure:"circuit_breaker"`
	RetryBuffer    RetryBufferConfig    `mapstructure:"retry_buffer"`
}

// CircuitBreakerConfig holds circuit breaker configuration.
type CircuitBreakerConfig struct {
	MaxFailures int           `mapstructure:"max_failures"`
	Timeout     time.Duration `mapstructure:"timeout"`
}

// RetryBufferConfig holds retry buffer configuration.
type RetryBufferConfig struct {
	MaxSize       int           `mapstructure:"max_size"`
	FlushInterval time.Duration `mapstructure:"flush_interval"`
}

// RateLimitConfig holds rate limiting configuration.
type RateLimitConfig struct {
	PerMinute float64 `mapstructure:"per_minute"`
	BurstSize int     `mapstructure:"burst_size"`
	Enabled   bool    `mapstructure:"enabled"`
}

// GroupSyncConfig holds group sync configuration.
type GroupSyncConfig struct {
	Interval time.Duration `mapstructure:"interval"`
}

// DedupConfig holds deduplication configuration.
type DedupConfig struct {
	Window          time.Duration `mapstructure:"window"`
	CacheSize       int           `mapstructure:"cache_size"`
	CacheTTL        time.Duration `mapstructure:"cache_ttl"`
	CleanupInterval time.Duration `mapstructure:"cleanup_interval"`
}

// HistorySyncConfig holds history sync configuration.
type HistorySyncConfig struct {
	Cooldown    time.Duration `mapstructure:"cooldown"`
	MaxAge      time.Duration `mapstructure:"max_age"`
	MaxMessages int           `mapstructure:"max_messages"`
	CacheSize   int           `mapstructure:"cache_size"`
	CacheTTL    time.Duration `mapstructure:"cache_ttl"`
}

// ReconnectorConfig holds reconnection configuration.
type ReconnectorConfig struct {
	InitialInterval     time.Duration `mapstructure:"initial_interval"`
	MaxInterval         time.Duration `mapstructure:"max_interval"`
	Multiplier          float64       `mapstructure:"multiplier"`
	RandomizationFactor float64       `mapstructure:"randomization_factor"`
	MaxElapsedTime      time.Duration `mapstructure:"max_elapsed_time"`
	MaxRetries          uint64        `mapstructure:"max_retries"`
}

// Load loads configuration from config.yml and environment variables.
func Load() (*Config, error) {
	v := viper.New()

	// Set defaults
	setDefaults(v)

	// Config file
	v.SetConfigName("config")
	v.SetConfigType("yaml")
	v.AddConfigPath(".")
	v.AddConfigPath("./config")

	// Environment variables override config file
	v.SetEnvPrefix("BRIDGE")
	v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	v.AutomaticEnv()

	// Read config file (optional - defaults are set)
	if err := v.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			return nil, fmt.Errorf("config file error: %w", err)
		}
		log.Debug().Msg("No config file found, using defaults")
	} else {
		log.Info().Str("file", v.ConfigFileUsed()).Msg("Loaded config file")
	}

	var cfg Config
	if err := v.Unmarshal(&cfg); err != nil {
		return nil, err
	}

	return &cfg, nil
}

func setDefaults(v *viper.Viper) {
	// gRPC
	v.SetDefault("grpc.core_addr", "localhost:50051")
	v.SetDefault("grpc.connect_timeout", 5*time.Second)

	// WhatsApp
	v.SetDefault("whatsapp.store_path", "./data/whatsapp.db")
	v.SetDefault("whatsapp.qr_retries", 5)
	v.SetDefault("whatsapp.qr_timeout", 60*time.Second)
	v.SetDefault("whatsapp.qr_terminal", true)

	// HTTP
	v.SetDefault("http.port", "5050")
	v.SetDefault("http.mode", "release")

	// Processing
	v.SetDefault("processing.skip_own_messages", true)
	v.SetDefault("processing.worker_count", 20)
	v.SetDefault("processing.worker_queue_size", 100)

	// Resilience - Circuit Breaker
	v.SetDefault("resilience.circuit_breaker.max_failures", 3)
	v.SetDefault("resilience.circuit_breaker.timeout", 30*time.Second)

	// Resilience - Retry Buffer
	v.SetDefault("resilience.retry_buffer.max_size", 1000)
	v.SetDefault("resilience.retry_buffer.flush_interval", 10*time.Second)

	// Rate Limit
	v.SetDefault("rate_limit.per_minute", 1000.0)
	v.SetDefault("rate_limit.burst_size", 100)
	v.SetDefault("rate_limit.enabled", true)

	// Group Sync
	v.SetDefault("group_sync.interval", 5*time.Minute)

	// Deduplication
	v.SetDefault("dedup.window", 10*time.Second)
	v.SetDefault("dedup.cache_size", 10000)
	v.SetDefault("dedup.cache_ttl", 30*time.Second)
	v.SetDefault("dedup.cleanup_interval", time.Minute)

	// History Sync
	v.SetDefault("history_sync.cooldown", 5*time.Minute)
	v.SetDefault("history_sync.max_age", 24*time.Hour)
	v.SetDefault("history_sync.max_messages", 1000)
	v.SetDefault("history_sync.cache_size", 10000)
	v.SetDefault("history_sync.cache_ttl", time.Hour)

	// Reconnector
	v.SetDefault("reconnector.initial_interval", 5*time.Second)
	v.SetDefault("reconnector.max_interval", 5*time.Minute)
	v.SetDefault("reconnector.multiplier", 2.0)
	v.SetDefault("reconnector.randomization_factor", 0.1)
	v.SetDefault("reconnector.max_elapsed_time", 0)
	v.SetDefault("reconnector.max_retries", 0)
}
