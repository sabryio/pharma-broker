package reports

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/rs/zerolog"
)

// SchedulerConfig holds scheduler settings
type SchedulerConfig struct {
	Enabled      bool   `yaml:"enabled"`
	Schedule     string `yaml:"schedule"`      // Cron expression or simple interval
	IntervalMins int    `yaml:"interval_mins"` // Simple interval in minutes (if no cron)
	Timezone     string `yaml:"timezone"`
}

// NotificationSender interface for sending reports
type NotificationSender interface {
	SendReport(ctx context.Context, summaryText, htmlReport string, csvData []byte, csvFilename string) error
}

// Scheduler runs reports on a schedule
type Scheduler struct {
	generator    *Generator
	notifier     NotificationSender
	config       SchedulerConfig
	reportConfig ReportConfig
	log          zerolog.Logger

	stopChan chan struct{}
	wg       sync.WaitGroup
	running  bool
	mu       sync.Mutex
}

// NewScheduler creates a new report scheduler
func NewScheduler(
	generator *Generator,
	notifier NotificationSender,
	schedulerConfig SchedulerConfig,
	reportConfig ReportConfig,
	log zerolog.Logger,
) *Scheduler {
	return &Scheduler{
		generator:    generator,
		notifier:     notifier,
		config:       schedulerConfig,
		reportConfig: reportConfig,
		log:          log.With().Str("component", "scheduler").Logger(),
		stopChan:     make(chan struct{}),
	}
}

// Start begins the scheduled report generation
func (s *Scheduler) Start(ctx context.Context) error {
	s.mu.Lock()
	if s.running {
		s.mu.Unlock()
		return fmt.Errorf("scheduler already running")
	}
	s.running = true
	s.mu.Unlock()

	if !s.config.Enabled {
		s.log.Info().Msg("Scheduler disabled, not starting")
		return nil
	}

	// Determine interval
	interval := time.Duration(s.config.IntervalMins) * time.Minute
	if interval <= 0 {
		interval = time.Hour // Default to hourly
	}

	s.log.Info().
		Dur("interval", interval).
		Bool("enabled", s.config.Enabled).
		Msg("Starting report scheduler")

	s.wg.Add(1)
	go s.run(ctx, interval)

	return nil
}

// Stop stops the scheduler
func (s *Scheduler) Stop() {
	s.mu.Lock()
	if !s.running {
		s.mu.Unlock()
		return
	}
	s.mu.Unlock()

	close(s.stopChan)
	s.wg.Wait()

	s.mu.Lock()
	s.running = false
	s.stopChan = make(chan struct{})
	s.mu.Unlock()

	s.log.Info().Msg("Scheduler stopped")
}

func (s *Scheduler) run(ctx context.Context, interval time.Duration) {
	defer s.wg.Done()

	// Calculate next run time (align to hour if hourly)
	var nextRun time.Time
	if interval >= time.Hour {
		now := time.Now()
		nextRun = now.Truncate(time.Hour).Add(time.Hour)
	} else {
		nextRun = time.Now().Add(interval)
	}

	s.log.Info().Time("next_run", nextRun).Msg("First report scheduled")

	for {
		waitDuration := time.Until(nextRun)
		if waitDuration < 0 {
			waitDuration = interval
			nextRun = time.Now().Add(interval)
		}

		timer := time.NewTimer(waitDuration)

		select {
		case <-ctx.Done():
			timer.Stop()
			s.log.Info().Msg("Scheduler context cancelled")
			return
		case <-s.stopChan:
			timer.Stop()
			s.log.Info().Msg("Scheduler stop requested")
			return
		case <-timer.C:
			s.executeReport(ctx)
			nextRun = nextRun.Add(interval)
			s.log.Debug().Time("next_run", nextRun).Msg("Next report scheduled")
		}
	}
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

// RunNow triggers an immediate report generation (for manual testing)
func (s *Scheduler) RunNow(ctx context.Context) error {
	s.log.Info().Msg("Manual report trigger")
	s.executeReport(ctx)
	return nil
}

// GetStatus returns scheduler status
func (s *Scheduler) GetStatus() map[string]interface{} {
	s.mu.Lock()
	defer s.mu.Unlock()

	return map[string]interface{}{
		"enabled":       s.config.Enabled,
		"running":       s.running,
		"interval_mins": s.config.IntervalMins,
	}
}
