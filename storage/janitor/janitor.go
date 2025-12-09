package janitor

import (
	"context"
	"sync"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
	"pharmabroker/pkg/config"
)

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

func (j *Janitor) Start() {
	j.mu.Lock()
	if j.running {
		j.mu.Unlock()
		return
	}
	j.running = true
	j.mu.Unlock()

	j.wg.Add(1)
	go j.runLoop()
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
		Str("archive_path", j.cfg.ArchivePath).
		Msg("🧹 Janitor started")

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
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute) // Long timeout for DB operations
	defer cancel()

	cutoff := time.Now().AddDate(0, 0, -j.cfg.RawRetentionDays)

	j.logger.Debug().Time("cutoff", cutoff).Msg("Starting daily archival...")

	count, err := j.repo.ArchiveOldMessages(ctx, j.cfg.ArchivePath, cutoff)
	if err != nil {
		j.logger.Error().Err(err).Msg("Failed to archive messages")
		return
	}

	if count > 0 {
		j.logger.Info().Int64("archived_count", count).Msg("✅ Archived old messages successfully")
	} else {
		j.logger.Debug().Msg("No messages to archive today")
	}
}
