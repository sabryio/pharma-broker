package storage

import (
	"context"
	"database/sql"
	"encoding/json"
	"time"
)

// ConfigRepo stores and retrieves dynamic configuration
type ConfigRepo struct {
	db *DB
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
	MatchThreshold      float64 `json:"match_threshold"`
	BatchSize           int     `json:"batch_size"`
	ProcessDelaySeconds int     `json:"process_delay_seconds"`
	SystemPrompt        string  `json:"system_prompt,omitempty"`
	ResponseFormat      string  `json:"response_format,omitempty"`
}

// DefaultConfig returns sensible defaults
func DefaultConfig() *AppConfig {
	return &AppConfig{
		MatchThreshold:      0.5,
		BatchSize:           5,
		ProcessDelaySeconds: 5,
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

// Set stores a config value
func (r *ConfigRepo) Set(ctx context.Context, key, value string) error {
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO config (key, value, updated_at) VALUES (?, ?, ?)
		ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
	`, key, value, time.Now())
	return err
}

// GetAll retrieves all config as AppConfig
func (r *ConfigRepo) GetAll(ctx context.Context) (*AppConfig, error) {
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
		}
	}

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
