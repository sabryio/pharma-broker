package cronjob

import (
	"context"
	"fmt"
	"sync"
	"time"

	cron "github.com/robfig/cron/v3"
)

// CronScheduler implements Scheduler using robfig/cron.
// It is exported to allow access to extended methods like NextRun.
type CronScheduler struct {
	cron      *cron.Cron
	logger    Logger
	metrics   MetricsCollector
	entries   map[string]cron.EntryID
	mu        sync.RWMutex
	opts      schedulerOptions
	running   bool
	runningMu sync.RWMutex
}

// NewScheduler creates a new cron-based Scheduler.
func NewScheduler(logger Logger, metrics MetricsCollector, options ...SchedulerOption) Scheduler {
	opts := schedulerOptions{}
	for _, opt := range options {
		opt(&opts)
	}

	cronOpts := []cron.Option{
		cron.WithChain(cron.Recover(cron.DefaultLogger)),
	}
	if opts.withSeconds {
		cronOpts = append(cronOpts, cron.WithSeconds())
	}
	if opts.location != nil {
		cronOpts = append(cronOpts, cron.WithLocation(opts.location))
	}

	c := cron.New(cronOpts...)
	return &CronScheduler{
		cron:    c,
		logger:  logger,
		metrics: metrics,
		entries: make(map[string]cron.EntryID),
		opts:    opts,
	}
}

// Start begins the scheduler's background processing.
func (s *CronScheduler) Start() {
	s.runningMu.Lock()
	defer s.runningMu.Unlock()

	if s.running {
		return
	}

	s.cron.Start()
	s.running = true
	s.logger.Info("scheduler.started", "jobs", len(s.entries))
}

// Stop gracefully shuts down the scheduler.
func (s *CronScheduler) Stop(ctx context.Context) {
	s.runningMu.Lock()
	defer s.runningMu.Unlock()

	if !s.running {
		return
	}

	waitCtx := s.cron.Stop()
	select {
	case <-waitCtx.Done():
		s.logger.Info("scheduler.stopped")
	case <-ctx.Done():
		s.logger.Error("scheduler.stop.timeout", ctx.Err())
	}
	s.running = false
}

// ScheduleJob adds a job to the scheduler.
func (s *CronScheduler) ScheduleJob(job Job) (string, error) {
	if job == nil {
		return "", fmt.Errorf("job cannot be nil")
	}

	expr := job.Schedule()
	if expr == "" {
		return "", fmt.Errorf("job %s: empty schedule", job.ID())
	}

	// Check if already scheduled
	s.mu.RLock()
	if _, exists := s.entries[job.ID()]; exists {
		s.mu.RUnlock()
		return "", fmt.Errorf("job %s: already scheduled", job.ID())
	}
	s.mu.RUnlock()

	// Build the runner with hooks and metrics
	runner := s.buildRunner(job)

	entryID, err := s.cron.AddFunc(expr, runner)
	if err != nil {
		return "", fmt.Errorf("failed to schedule job %s: %w", job.ID(), err)
	}

	s.mu.Lock()
	s.entries[job.ID()] = entryID
	s.mu.Unlock()

	s.logger.Info("job.scheduled", "job", job.ID(), "schedule", expr, "entry_id", entryID)
	return fmt.Sprintf("%d", entryID), nil
}

// RemoveJob removes a job from the scheduler.
func (s *CronScheduler) RemoveJob(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	entryID, exists := s.entries[id]
	if !exists {
		return fmt.Errorf("job %s: not found", id)
	}

	s.cron.Remove(entryID)
	delete(s.entries, id)

	s.logger.Info("job.removed", "job", id)
	return nil
}

// GetEntryID returns the cron entry ID for a job.
func (s *CronScheduler) GetEntryID(id string) (string, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	entryID, exists := s.entries[id]
	if !exists {
		return "", false
	}
	return fmt.Sprintf("%d", entryID), true
}

// buildRunner creates the execution wrapper for a job.
func (s *CronScheduler) buildRunner(job Job) func() {
	return func() {
		ctx := context.Background()
		start := time.Now()

		result := &JobResult{
			JobID:     job.ID(),
			StartTime: start,
		}

		// Execute before hooks
		for _, hook := range s.opts.beforeHooks {
			hook(ctx, job, result)
		}

		s.logger.Info("job.started", "job", job.ID())

		// Execute the job
		err := job.Run(ctx)

		// Record result
		result.EndTime = time.Now()
		result.Duration = result.EndTime.Sub(result.StartTime)
		result.Error = err
		result.Success = err == nil

		// Record metrics
		if err != nil {
			s.logger.Error("job.failed", err, "job", job.ID(), "duration_s", result.Duration.Seconds())
			s.metrics.Increment("cronjob_failed_total", map[string]string{"job": job.ID()})
		} else {
			s.logger.Info("job.completed", "job", job.ID(), "duration_s", result.Duration.Seconds())
			s.metrics.Increment("cronjob_success_total", map[string]string{"job": job.ID()})
		}
		s.metrics.ObserveDuration("cronjob_duration_seconds", map[string]string{"job": job.ID()}, result.Duration.Seconds())

		// Execute after hooks
		for _, hook := range s.opts.afterHooks {
			hook(ctx, job, result)
		}
	}
}

// IsRunning returns whether the scheduler is currently running.
func (s *CronScheduler) IsRunning() bool {
	s.runningMu.RLock()
	defer s.runningMu.RUnlock()
	return s.running
}

// Entries returns a list of all scheduled entry IDs.
func (s *CronScheduler) Entries() []cron.Entry {
	return s.cron.Entries()
}

// JobCount returns the number of scheduled jobs.
func (s *CronScheduler) JobCount() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.entries)
}

// NextRun returns the next scheduled run time for a job.
func (s *CronScheduler) NextRun(id string) (time.Time, bool) {
	s.mu.RLock()
	entryID, exists := s.entries[id]
	s.mu.RUnlock()

	if !exists {
		return time.Time{}, false
	}

	entry := s.cron.Entry(entryID)
	return entry.Next, !entry.Next.IsZero()
}
