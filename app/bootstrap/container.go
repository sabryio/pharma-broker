// Package bootstrap provides dependency injection and application wiring.
package bootstrap

import (
	"context"

	"github.com/rs/zerolog"

	"pharmabroker/ai"
	"pharmabroker/api"
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	storageGorm "pharmabroker/storage/gorm"
)

// Container holds all application dependencies
type Container struct {
	// Configuration
	Config *Config

	// Infrastructure
	DB     *storageGorm.DB
	Logger zerolog.Logger

	// Repositories
	Repos *Repositories

	// Services
	AIProvider ai.Provider
	SSEHub     api.SSEHub

	// Cleanup functions
	cleanups []func() error
}

// Config holds application configuration
type Config struct {
	// Database
	Database DatabaseConfig

	// API
	API api.Config

	// AI
	AI AIConfig
}

// DatabaseConfig mirrors storage config
type DatabaseConfig struct {
	Path string
}

// AIConfig holds AI provider configuration
type AIConfig struct {
	Provider string // "gemini" or "docker"
}

// Repositories bundles all repository implementations
type Repositories struct {
	Offers   repository.OfferRepository
	Requests repository.RequestRepository
	Matches  repository.MatchRepository
	Groups   repository.GroupRepository
	Stats    repository.StatsRepository
	Messages repository.RawMessageRepository
	Mappings repository.MedicationMappingRepository
	Queue    repository.MatchQueueRepository
	Review   repository.ReviewQueueRepository
}

// New creates a new application container with all dependencies
func New(ctx context.Context, cfg *Config, log zerolog.Logger) (*Container, error) {
	c := &Container{
		Config: cfg,
		Logger: log,
	}

	// Initialize database
	db, err := storageGorm.NewDB(&storageGorm.Config{
		Path: cfg.Database.Path,
	})
	if err != nil {
		return nil, err
	}
	c.DB = db
	c.cleanups = append(c.cleanups, db.Close)

	log.Info().Str("path", cfg.Database.Path).Msg("Database initialized")

	// Initialize all repositories using new implementations
	c.Repos = &Repositories{
		Offers:   storageGorm.NewOfferRepo(db),
		Requests: storageGorm.NewRequestRepo(db),
		Matches:  storageGorm.NewMatchRepo(db),
		Groups:   storageGorm.NewGroupRepo(db),
		Stats:    storageGorm.NewStatsRepo(db),
		Messages: storageGorm.NewRawMessageRepo(db),
		Mappings: storageGorm.NewMedicationMappingRepo(db),
		Queue:    storageGorm.NewMatchQueueRepo(db),
		Review:   storageGorm.NewReviewQueueRepo(db),
	}

	log.Info().Msg("Repositories initialized")

	return c, nil
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

// SetAIProvider sets the AI provider (called during migration)
func (c *Container) SetAIProvider(p ai.Provider) {
	c.AIProvider = p
}

// SetSSEHub sets the SSE hub (called during migration)
func (c *Container) SetSSEHub(hub api.SSEHub) {
	c.SSEHub = hub
}

// Compile-time checks
var (
	_                            = entity.Offer{}
	_ repository.OfferRepository = nil
)
