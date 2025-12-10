// Package cronjob provides a production-ready cron scheduler with observability,
// job registry, and graceful shutdown support.
package cronjob

import (
	"context"
	"time"
)

// Job represents a scheduled task. Implementations define their own ID, schedule, and execution logic.
type Job interface {
	// ID returns a unique identifier for this job.
	ID() string
	// Schedule returns a cron expression (e.g., "0 * * * *" for hourly).
	Schedule() string
	// Run executes the job. Context can be used for cancellation.
	Run(ctx context.Context) error
}

// Scheduler manages job scheduling and execution.
type Scheduler interface {
	// Start begins the scheduler's background processing.
	Start()
	// Stop gracefully shuts down the scheduler, waiting up to ctx deadline.
	Stop(ctx context.Context)
	// ScheduleJob adds a job to the scheduler and returns an entry ID.
	ScheduleJob(job Job) (string, error)
	// RemoveJob removes a job by its ID.
	RemoveJob(id string) error
	// GetEntryID returns the cron entry ID for a job, if scheduled.
	GetEntryID(id string) (string, bool)
}

// Logger abstracts logging operations.
type Logger interface {
	Info(msg string, keyvals ...any)
	Error(msg string, err error, keyvals ...any)
	Debug(msg string, keyvals ...any)
}

// MetricsCollector abstracts metrics operations.
type MetricsCollector interface {
	Increment(name string, labels map[string]string)
	ObserveDuration(name string, labels map[string]string, seconds float64)
}

// JobResult contains the outcome of a job execution.
type JobResult struct {
	JobID     string
	StartTime time.Time
	EndTime   time.Time
	Duration  time.Duration
	Error     error
	Success   bool
}

// JobHook is called before or after job execution.
type JobHook func(ctx context.Context, job Job, result *JobResult)

// SchedulerOption configures the scheduler.
type SchedulerOption func(*schedulerOptions)

type schedulerOptions struct {
	withSeconds bool
	beforeHooks []JobHook
	afterHooks  []JobHook
	location    *time.Location
}

// WithSeconds enables second-level precision in cron expressions.
func WithSeconds() SchedulerOption {
	return func(o *schedulerOptions) {
		o.withSeconds = true
	}
}

// WithBeforeHook adds a hook called before each job execution.
func WithBeforeHook(hook JobHook) SchedulerOption {
	return func(o *schedulerOptions) {
		o.beforeHooks = append(o.beforeHooks, hook)
	}
}

// WithAfterHook adds a hook called after each job execution.
func WithAfterHook(hook JobHook) SchedulerOption {
	return func(o *schedulerOptions) {
		o.afterHooks = append(o.afterHooks, hook)
	}
}

// WithLocation sets the timezone for schedule interpretation.
func WithLocation(loc *time.Location) SchedulerOption {
	return func(o *schedulerOptions) {
		o.location = loc
	}
}
