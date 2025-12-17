package janitor

import (
	"context"
	"sync"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
	"pharmabroker/pkg/config"
)

// Janitor handles cleanup of old messages.
// With PostgreSQL, archival is done via pg_dump or DELETE queries
// rather than copying to a separate SQLite file.
type Janitor struct {
	repo    repository.RawMessageRepository
	cfg     config.DatabaseConfig
	logger  zerolog.Logger
	wg      sync.WaitGroup
	stop    chan struct{}
	running bool
	mu      sync.Mutex
}

func NewJanitor(repo repository.RawMessageRepository, cfg config.DatabaseConfig, logger zerolog.Logger) *Janitor {
	return &Janitor{
		repo:   repo,
		cfg:    cfg,
		logger: logger.With().Str("component", "Janitor").Logger(),
		stop:   make(chan struct{}),
	}
}

func (j *Janitor) Start() error {
	j.mu.Lock()
	if j.running {
		j.mu.Unlock()
		return nil
	}
	j.running = true
	j.mu.Unlock()

	j.wg.Add(1)
	go j.runLoop()
	return nil
}

func (j *Janitor) Stop() {
	j.mu.Lock()
	defer j.mu.Unlock()
	if !j.running {
		return
	}
	close(j.stop)
	j.wg.Wait()
	j.running = false
}

func (j *Janitor) runLoop() {
	defer j.wg.Done()
	j.logger.Info().
		Int("retention_days", j.cfg.RawRetentionDays).
		Msg("🧹 Janitor started (PostgreSQL mode)")

	// Run immediately on startup
	j.performCleanup()

	ticker := time.NewTicker(24 * time.Hour)
	defer ticker.Stop()

	for {
		select {
		case <-j.stop:
			j.logger.Info().Msg("Janitor stopped")
			return
		case <-ticker.C:
			j.performCleanup()
		}
	}
}

func (j *Janitor) performCleanup() {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()

	cutoff := time.Now().AddDate(0, 0, -j.cfg.RawRetentionDays)

	j.logger.Debug().Time("cutoff", cutoff).Msg("Starting daily cleanup...")

	// With PostgreSQL, we delete old messages directly instead of archiving to a file
	// For actual archival, use pg_dump or a separate archival process
	count, err := j.repo.DeleteOldMessages(ctx, cutoff)
	if err != nil {
		j.logger.Error().Err(err).Msg("Failed to delete old messages")
		return
	}

	if count > 0 {
		j.logger.Info().Int64("deleted_count", count).Msg("✅ Deleted old messages successfully")
	} else {
		j.logger.Debug().Msg("No messages to delete today")
	}
}
