// Package cronjob provides scheduled tasks for PharmaBroker.
package cronjob

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/pkg/metrics"

	"github.com/rs/zerolog"
)

// MatchEscalationConfig configures the escalation job behavior
type MatchEscalationConfig struct {
	// MaxAge is the duration after which a pending/review match is considered stale
	MaxAge time.Duration
	// BatchSize limits the number of matches processed per run
	BatchSize int
	// Statuses defines which match statuses to escalate
	Statuses []entity.MatchStatus
}

// DefaultMatchEscalationConfig returns sensible defaults
func DefaultMatchEscalationConfig() MatchEscalationConfig {
	return MatchEscalationConfig{
		MaxAge:    24 * time.Hour, // Escalate after 24 hours
		BatchSize: 50,
		Statuses: []entity.MatchStatus{
			entity.MatchStatusPending, // Includes both SUGGEST and REVIEW confidence bands
		},
	}
}

// MatchEscalationJob periodically checks for stale matches and escalates them.
// Escalation means sending reminder notifications and optionally updating priority.
type MatchEscalationJob struct {
	matchRepo repository.MatchRepository
	notifier  EscalationNotifier
	config    MatchEscalationConfig
	log       zerolog.Logger
}

// EscalationNotifier defines the interface for sending escalation alerts
type EscalationNotifier interface {
	NotifyStaleMatches(ctx context.Context, count int, oldestAge time.Duration) error
}

// NewMatchEscalationJob creates a new match escalation cron job
func NewMatchEscalationJob(
	matchRepo repository.MatchRepository,
	notifier EscalationNotifier,
	config MatchEscalationConfig,
	log zerolog.Logger,
) *MatchEscalationJob {
	return &MatchEscalationJob{
		matchRepo: matchRepo,
		notifier:  notifier,
		config:    config,
		log:       log.With().Str("job", "match_escalation").Logger(),
	}
}

// ID returns the unique job identifier
func (j *MatchEscalationJob) ID() string {
	return "match_escalation"
}

// Schedule returns the cron expression (run every hour at minute 15)
func (j *MatchEscalationJob) Schedule() string {
	return "15 * * * *" // Every hour at :15
}

// Run executes the escalation check
func (j *MatchEscalationJob) Run(ctx context.Context) error {
	j.log.Info().Msg("Starting match escalation check")

	// Find stale matches
	staleMatches, err := j.matchRepo.GetStaleMatches(
		ctx,
		j.config.Statuses,
		j.config.MaxAge,
		j.config.BatchSize,
	)
	if err != nil {
		j.log.Error().Err(err).Msg("Failed to get stale matches")
		return err
	}

	if len(staleMatches) == 0 {
		j.log.Debug().Msg("No stale matches found")
		return nil
	}

	// Calculate oldest match age
	var oldestAge time.Duration
	for _, match := range staleMatches {
		age := time.Since(match.CreatedAt)
		if age > oldestAge {
			oldestAge = age
		}
	}

	j.log.Warn().
		Int("count", len(staleMatches)).
		Dur("oldest_age", oldestAge).
		Dur("threshold", j.config.MaxAge).
		Msg("Found stale matches requiring attention")

	// Record metrics
	metrics.MatchesEscalated.Add(float64(len(staleMatches)))

	// Send notification
	if j.notifier != nil {
		if err := j.notifier.NotifyStaleMatches(ctx, len(staleMatches), oldestAge); err != nil {
			j.log.Error().Err(err).Msg("Failed to send escalation notification")
			// Don't fail the job, notification failure shouldn't block
		}
	}

	return nil
}
