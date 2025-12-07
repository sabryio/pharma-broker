package storage

import (
	"context"
	"fmt"

	"pharmabroker/internal/storage/models"
)

// GormConfigRepo implements configuration storage using GORM
type GormConfigRepo struct {
	db *GormDB
}

// NewGormConfigRepo creates a new GORM-based config repository
func NewGormConfigRepo(db *GormDB) *GormConfigRepo {
	return &GormConfigRepo{db: db}
}

// GetAll retrieves all configuration as AppConfig
func (r *GormConfigRepo) GetAll(ctx context.Context) (*AppConfig, error) {
	var configs []models.Config
	err := r.db.DB.WithContext(ctx).Find(&configs).Error
	if err != nil {
		return nil, err
	}

	cfg := &AppConfig{
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
func (r *GormConfigRepo) Set(ctx context.Context, key, value string) error {
	return r.db.DB.WithContext(ctx).Save(&models.Config{
		Key:   key,
		Value: value,
	}).Error
}

// Get retrieves a configuration value by key
func (r *GormConfigRepo) Get(ctx context.Context, key string) (string, error) {
	var config models.Config
	err := r.db.DB.WithContext(ctx).Where("key = ?", key).First(&config).Error
	if err != nil {
		return "", err
	}
	return config.Value, nil
}

// Delete removes a configuration key
func (r *GormConfigRepo) Delete(ctx context.Context, key string) error {
	return r.db.DB.WithContext(ctx).
		Where("key = ?", key).
		Delete(&models.Config{}).Error
}

// UpdateFromMap updates multiple config values from a map
func (r *GormConfigRepo) UpdateFromMap(ctx context.Context, values map[string]any) error {
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
