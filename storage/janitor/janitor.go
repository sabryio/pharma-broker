package janitor

import (
	"context"
	"sync"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
	"pharmabroker/pkg/config"
)

// Janitor handles cleanup of old messages and audit logs.
// With PostgreSQL, archival is done via pg_dump or DELETE queries
// rather than copying to a separate SQLite file.
type Janitor struct {
	repo      repository.RawMessageRepository
	auditRepo repository.AuditRepository
	cfg       config.DatabaseConfig
	logger    zerolog.Logger
	wg        sync.WaitGroup
	stop      chan struct{}
	running   bool
	mu        sync.Mutex
}

func NewJanitor(repo repository.RawMessageRepository, auditRepo repository.AuditRepository, cfg config.DatabaseConfig, logger zerolog.Logger) *Janitor {
	return &Janitor{
		repo:      repo,
		auditRepo: auditRepo,
		cfg:       cfg,
		logger:    logger.With().Str("component", "Janitor").Logger(),
		stop:      make(chan struct{}),
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
		Int("raw_retention_days", j.cfg.RawRetentionDays).
		Int("audit_retention_days", j.cfg.AuditRetentionDays).
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

	// Clean raw messages
	msgCutoff := time.Now().AddDate(0, 0, -j.cfg.RawRetentionDays)
	j.logger.Debug().Time("msg_cutoff", msgCutoff).Msg("Starting daily cleanup...")

	count, err := j.repo.DeleteOldMessages(ctx, msgCutoff)
	if err != nil {
		j.logger.Error().Err(err).Msg("Failed to delete old messages")
	} else if count > 0 {
		j.logger.Info().Int64("deleted_count", count).Msg("✅ Deleted old messages successfully")
	}

	// Clean audit logs (if repo available and retention > 0)
	if j.auditRepo != nil && j.cfg.AuditRetentionDays > 0 {
		auditCutoff := time.Now().AddDate(0, 0, -j.cfg.AuditRetentionDays)
		auditCount, err := j.auditRepo.DeleteOlderThan(ctx, auditCutoff)
		if err != nil {
			j.logger.Error().Err(err).Msg("Failed to delete old audit logs")
		} else if auditCount > 0 {
			j.logger.Info().Int64("deleted_count", auditCount).Msg("✅ Deleted old audit logs successfully")
		}
	}
}
