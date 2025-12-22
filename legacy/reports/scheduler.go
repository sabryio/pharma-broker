package reports

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/pkg/cronjob"
)

// SchedulerConfig holds scheduler settings
type SchedulerConfig struct {
	Enabled      bool   `yaml:"enabled"`
	Schedule     string `yaml:"schedule"`      // Cron expression (e.g., "0 * * * *" for hourly)
	IntervalMins int    `yaml:"interval_mins"` // Simple interval in minutes (fallback if no cron)
	Timezone     string `yaml:"timezone"`
}

// NotificationSender interface for sending reports
type NotificationSender interface {
	SendReport(ctx context.Context, summaryText, htmlReport string, csvData []byte, csvFilename string) error
}

// SchedulerStatus represents the current status of the scheduler.
type SchedulerStatus struct {
	Enabled      bool       `json:"enabled"`
	Running      bool       `json:"running"`
	IntervalMins int        `json:"interval_mins,omitempty"`
	Schedule     string     `json:"schedule,omitempty"`
	JobID        string     `json:"job_id"`
	NextRun      *time.Time `json:"next_run,omitempty"`
}

// Scheduler runs reports on a schedule using the cronjob package.
type Scheduler struct {
	generator    *Generator
	notifier     NotificationSender
	config       SchedulerConfig
	reportConfig ReportConfig
	log          zerolog.Logger

	cronScheduler cronjob.Scheduler
	jobID         string
	mu            sync.Mutex
	running       bool
}

// NewScheduler creates a new report scheduler.
func NewScheduler(
	generator *Generator,
	notifier NotificationSender,
	schedulerConfig SchedulerConfig,
	reportConfig ReportConfig,
	log zerolog.Logger,
) *Scheduler {
	logger := cronjob.NewZerologAdapter(log)
	metrics := cronjob.NewPrometheusMetricsAdapter()

	return &Scheduler{
		generator:     generator,
		notifier:      notifier,
		config:        schedulerConfig,
		reportConfig:  reportConfig,
		log:           log.With().Str("component", "report-scheduler").Logger(),
		cronScheduler: cronjob.NewScheduler(logger, metrics),
		jobID:         "report-generator",
	}
}

// Start begins the scheduled report generation.
func (s *Scheduler) Start(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.running {
		return fmt.Errorf("scheduler already running")
	}

	if !s.config.Enabled {
		s.log.Info().Msg("Scheduler disabled, not starting")
		return nil
	}

	// Determine schedule expression
	schedule := s.config.Schedule
	if schedule == "" {
		// Fallback to interval-based schedule
		interval := s.config.IntervalMins
		if interval <= 0 {
			interval = 60 // Default to hourly
		}
		schedule = fmt.Sprintf("@every %dm", interval)
	}

	// Create the report job
	job := cronjob.NewFuncJob(s.jobID, schedule, func(ctx context.Context) error {
		s.executeReport(ctx)
		return nil
	})

	// Schedule the job
	if _, err := s.cronScheduler.ScheduleJob(job); err != nil {
		return fmt.Errorf("failed to schedule report job: %w", err)
	}

	// Start the cron scheduler
	s.cronScheduler.Start()
	s.running = true

	s.log.Info().
		Str("schedule", schedule).
		Bool("enabled", s.config.Enabled).
		Msg("Report scheduler started")

	return nil
}

// Stop stops the scheduler gracefully.
func (s *Scheduler) Stop() {
	s.mu.Lock()
	defer s.mu.Unlock()

	if !s.running {
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	s.cronScheduler.Stop(ctx)
	s.running = false

	s.log.Info().Msg("Report scheduler stopped")
}

func (s *Scheduler) executeReport(ctx context.Context) {
	s.log.Info().Msg("Executing scheduled report")

	startTime := time.Now()

	// Generate report
	report, err := s.generator.GenerateHourlyReport(ctx, s.reportConfig)
	if err != nil {
		s.log.Error().Err(err).Msg("Failed to generate report")
		return
	}

	// Skip if no matches
	if len(report.Matches) == 0 && len(report.Alerts) == 0 {
		s.log.Info().Msg("No matches or alerts, skipping notification")
		return
	}

	// Generate outputs
	csvData, err := s.generator.ExportToCSV(report)
	if err != nil {
		s.log.Error().Err(err).Msg("Failed to export CSV")
		csvData = nil
	}

	summaryText := s.generator.GenerateSummaryText(report)
	htmlReport := s.generator.GenerateHTMLReport(report)

	// Create filename with timestamp
	csvFilename := fmt.Sprintf("pharmabroker_report_%s.csv", time.Now().Format("2006-01-02_15-04"))

	// Send notifications
	if err := s.notifier.SendReport(ctx, summaryText, htmlReport, csvData, csvFilename); err != nil {
		s.log.Error().Err(err).Msg("Failed to send notifications")
	} else {
		s.log.Info().
			Int("matches", len(report.Matches)).
			Int("alerts", len(report.Alerts)).
			Dur("duration", time.Since(startTime)).
			Msg("Report sent successfully")
	}
}

// RunNow triggers an immediate report generation (for manual testing).
func (s *Scheduler) RunNow(ctx context.Context) error {
	s.log.Info().Msg("Manual report trigger")
	s.executeReport(ctx)
	return nil
}

// GetStatus returns scheduler status.
func (s *Scheduler) GetStatus() SchedulerStatus {
	s.mu.Lock()
	defer s.mu.Unlock()

	status := SchedulerStatus{
		Enabled:      s.config.Enabled,
		Running:      s.running,
		IntervalMins: s.config.IntervalMins,
		Schedule:     s.config.Schedule,
		JobID:        s.jobID,
	}

	// Add next run time if available
	if cs, ok := s.cronScheduler.(*cronjob.CronScheduler); ok {
		if nextRun, found := cs.NextRun(s.jobID); found {
			status.NextRun = &nextRun
		}
	}

	return status
}

// IsRunning returns whether the scheduler is currently running.
func (s *Scheduler) IsRunning() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.running
}
