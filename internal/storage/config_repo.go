package storage

import (
	"context"
	"database/sql"
	"encoding/json"
	"sync"
	"time"
)

// Cache TTL for config values
const configCacheTTL = 30 * time.Second

// ConfigRepo stores and retrieves dynamic configuration with caching
type ConfigRepo struct {
	db *DB

	// Cache
	mu           sync.RWMutex
	cachedConfig *AppConfig
	cacheTime    time.Time
}

// NewConfigRepo creates a new config repository
func NewConfigRepo(db *DB) *ConfigRepo {
	return &ConfigRepo{db: db}
}

// Config represents system configuration
type Config struct {
	Key       string    `json:"key"`
	Value     string    `json:"value"`
	UpdatedAt time.Time `json:"updated_at"`
}

// AppConfig represents all configurable settings
type AppConfig struct {
	AutoParseEnabled       bool    `json:"auto_parse_enabled"`
	SkipOwnMessages        bool    `json:"skip_own_messages"`
	MatchThreshold         float64 `json:"match_threshold"`
	SemanticMatchThreshold float64 `json:"semantic_match_threshold"`
	BatchSize              int     `json:"batch_size"`
	ProcessDelaySeconds    int     `json:"process_delay_seconds"`
	SystemPrompt           string  `json:"system_prompt,omitempty"`
	ResponseFormat         string  `json:"response_format,omitempty"`
	AdminPhone             string  `json:"admin_phone,omitempty"`
}

// DefaultConfig returns sensible defaults
func DefaultConfig() *AppConfig {
	return &AppConfig{
		AutoParseEnabled:       true, // Parse messages by default
		SkipOwnMessages:        true, // Skip own messages by default
		MatchThreshold:         0.5,
		SemanticMatchThreshold: 0.85, // Strong semantic match by default
		BatchSize:              10,
		ProcessDelaySeconds:    5,
		SystemPrompt:           "",
		AdminPhone:             "",
	}
}

// Get retrieves a config value
func (r *ConfigRepo) Get(ctx context.Context, key string) (string, error) {
	var value string
	err := r.db.conn.QueryRowContext(ctx,
		"SELECT value FROM config WHERE key = ?", key).Scan(&value)
	if err == sql.ErrNoRows {
		return "", nil
	}
	return value, err
}

// Set stores a config value and invalidates cache
func (r *ConfigRepo) Set(ctx context.Context, key, value string) error {
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO config (key, value, updated_at) VALUES (?, ?, ?)
		ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
	`, key, value, time.Now())

	if err == nil {
		// Invalidate cache on successful update
		r.mu.Lock()
		r.cachedConfig = nil
		r.mu.Unlock()
	}

	return err
}

// GetAll retrieves all config as AppConfig (with caching)
func (r *ConfigRepo) GetAll(ctx context.Context) (*AppConfig, error) {
	// Check cache first
	r.mu.RLock()
	if r.cachedConfig != nil && time.Since(r.cacheTime) < configCacheTTL {
		cached := r.cachedConfig
		r.mu.RUnlock()
		return cached, nil
	}
	r.mu.RUnlock()

	// Cache miss or expired - fetch from database
	config := DefaultConfig()

	rows, err := r.db.conn.QueryContext(ctx, "SELECT key, value FROM config")
	if err != nil {
		return config, err
	}
	defer rows.Close()

	for rows.Next() {
		var key, value string
		if err := rows.Scan(&key, &value); err != nil {
			continue
		}

		switch key {
		case "auto_parse_enabled":
			var v bool
			if json.Unmarshal([]byte(value), &v) == nil {
				config.AutoParseEnabled = v
			}
		case "skip_own_messages":
			var v bool
			if json.Unmarshal([]byte(value), &v) == nil {
				config.SkipOwnMessages = v
			}
		case "match_threshold":
			var v float64
			if json.Unmarshal([]byte(value), &v) == nil {
				config.MatchThreshold = v
			}
		case "batch_size":
			var v int
			if json.Unmarshal([]byte(value), &v) == nil {
				config.BatchSize = v
			}
		case "process_delay_seconds":
			var v int
			if json.Unmarshal([]byte(value), &v) == nil {
				config.ProcessDelaySeconds = v
			}
		case "system_prompt":
			config.SystemPrompt = value
		case "response_format":
			config.ResponseFormat = value
		case "admin_phone":
			config.AdminPhone = value
		}
	}

	// Update cache
	r.mu.Lock()
	r.cachedConfig = config
	r.cacheTime = time.Now()
	r.mu.Unlock()

	return config, nil
}

// UpdateFromMap updates config from a map of key-value pairs
func (r *ConfigRepo) UpdateFromMap(ctx context.Context, updates map[string]interface{}) error {
	for key, value := range updates {
		var strValue string
		switch v := value.(type) {
		case string:
			strValue = v
		default:
			data, err := json.Marshal(v)
			if err != nil {
				continue
			}
			strValue = string(data)
		}

		if err := r.Set(ctx, key, strValue); err != nil {
			return err
		}
	}
	return nil
}
