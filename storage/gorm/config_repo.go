// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"fmt"
)

// AppSettings holds aggregated application configuration
type AppSettings struct {
	AutoParseEnabled bool   `json:"auto_parse_enabled"`
	SkipOwnMessages  bool   `json:"skip_own_messages"`
	AdminPhone       string `json:"admin_phone"`
}

// ConfigRepo implements configuration storage
type ConfigRepo struct {
	db *DB
}

// NewConfigRepo creates a new config repository
func NewConfigRepo(db *DB) *ConfigRepo {
	return &ConfigRepo{db: db}
}

// GetAll retrieves all configuration as AppSettings
func (r *ConfigRepo) GetAll(ctx context.Context) (*AppSettings, error) {
	var configs []AppConfig
	err := r.db.Conn.WithContext(ctx).Find(&configs).Error
	if err != nil {
		return nil, err
	}

	cfg := &AppSettings{
		AutoParseEnabled: true, // Default
		SkipOwnMessages:  true, // Default
	}

	for _, c := range configs {
		switch c.Key {
		case "auto_parse_enabled":
			cfg.AutoParseEnabled = c.Value == "true"
		case "skip_own_messages":
			cfg.SkipOwnMessages = c.Value == "true"
		case "admin_phone":
			cfg.AdminPhone = c.Value
		}
	}

	return cfg, nil
}

// Set stores a configuration key-value pair
func (r *ConfigRepo) Set(ctx context.Context, key, value string) error {
	return r.db.Conn.WithContext(ctx).Save(&AppConfig{
		Key:   key,
		Value: value,
	}).Error
}

// Get retrieves a configuration value by key
func (r *ConfigRepo) Get(ctx context.Context, key string) (string, error) {
	var config AppConfig
	err := r.db.Conn.WithContext(ctx).Where("key = ?", key).First(&config).Error
	if err != nil {
		return "", err
	}
	return config.Value, nil
}

// Delete removes a configuration key
func (r *ConfigRepo) Delete(ctx context.Context, key string) error {
	return r.db.Conn.WithContext(ctx).
		Where("key = ?", key).
		Delete(&AppConfig{}).Error
}

// UpdateFromMap updates multiple config values from a map
func (r *ConfigRepo) UpdateFromMap(ctx context.Context, values map[string]any) error {
	for key, val := range values {
		var strVal string
		switch v := val.(type) {
		case string:
			strVal = v
		case bool:
			if v {
				strVal = "true"
			} else {
				strVal = "false"
			}
		default:
			strVal = fmt.Sprintf("%v", v)
		}
		if err := r.Set(ctx, key, strVal); err != nil {
			return err
		}
	}
	return nil
}
