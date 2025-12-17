package monitor

import (
	"context"
	"fmt"
	"pharmabroker/domain/entity"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// MessageSender abstracts message sending for testability
type MessageSender interface {
	SendMessage(ctx context.Context, jid, msg string) error
}

// ConfigProvider abstracts config retrieval for testability
type ConfigProvider interface {
	GetAll(ctx context.Context) (*entity.AppConfig, error)
}

// WarRoomConfig holds configurable thresholds
type WarRoomConfig struct {
	ErrorThreshold int           // number of errors to trigger alert (default: 5)
	ErrorWindow    time.Duration // time window for counting errors (default: 1 minute)
	AlertCooldown  time.Duration // minimum time between alerts (default: 10 minutes)
}

// DefaultWarRoomConfig returns sensible defaults
func DefaultWarRoomConfig() WarRoomConfig {
	return WarRoomConfig{
		ErrorThreshold: 5,
		ErrorWindow:    time.Minute,
		AlertCooldown:  10 * time.Minute,
	}
}

// WarRoom monitors system health and sends alerts
type WarRoom struct {
	// Metrics first for 64-bit alignment on 32-bit systems
	alertsSent   int64
	errorsLogged int64

	sender     MessageSender
	configRepo ConfigProvider
	log        zerolog.Logger
	cfg        WarRoomConfig

	mu            sync.Mutex
	errorCounts   []time.Time
	alertCooldown time.Time

	// Graceful shutdown
	done chan struct{}
}

// NewWarRoom creates a new monitor with default config
func NewWarRoom(sender MessageSender, cfgRepo ConfigProvider, log zerolog.Logger) *WarRoom {
	return NewWarRoomWithConfig(sender, cfgRepo, log, DefaultWarRoomConfig())
}

// NewWarRoomWithConfig creates a new monitor with custom config
func NewWarRoomWithConfig(sender MessageSender, cfgRepo ConfigProvider, log zerolog.Logger, cfg WarRoomConfig) *WarRoom {
	return &WarRoom{
		sender:      sender,
		configRepo:  cfgRepo,
		log:         log,
		cfg:         cfg,
		errorCounts: make([]time.Time, 0, 100), // pre-allocate
		done:        make(chan struct{}),
	}
}

// NotifyError records an error and checks thresholds
func (w *WarRoom) NotifyError(ctx context.Context, err error) {
	atomic.AddInt64(&w.errorsLogged, 1)

	shouldAlert, count := w.recordError()

	w.log.Warn().Err(err).Int("recent_errors", count).Msg("WarRoom received error report")

	if shouldAlert {
		w.triggerAlert(ctx, count)
	}
}

// recordError adds error timestamp and returns whether alert threshold is met
func (w *WarRoom) recordError() (shouldAlert bool, count int) {
	w.mu.Lock()
	defer w.mu.Unlock()

	now := time.Now()
	w.errorCounts = append(w.errorCounts, now)
	w.cleanupOld(now)
	count = len(w.errorCounts)

	return count >= w.cfg.ErrorThreshold, count
}

func (w *WarRoom) cleanupOld(now time.Time) {
	cutoff := now.Add(-w.cfg.ErrorWindow)
	valid := 0
	for _, t := range w.errorCounts {
		if t.After(cutoff) {
			w.errorCounts[valid] = t
			valid++
		}
	}
	w.errorCounts = w.errorCounts[:valid]
}

func (w *WarRoom) triggerAlert(ctx context.Context, count int) {
	// Check and set cooldown atomically
	if !w.trySetCooldown() {
		return
	}

	cfg, err := w.configRepo.GetAll(ctx)
	if err != nil || cfg.AdminPhone == "" {
		w.log.Warn().Msg("Cannot send alert: AdminPhone not configured")
		return
	}

	msg := fmt.Sprintf("🚨 *CRITICAL ALERT* 🚨\n\nHigh error rate detected: %d errors in the last %s.\nCheck logs immediately.",
		count, w.cfg.ErrorWindow)

	targetJID := formatJID(cfg.AdminPhone)

	w.log.Error().Str("admin", targetJID).Msg("Triggering WhatsApp Alert")
	if err := w.sender.SendMessage(ctx, targetJID, msg); err != nil {
		w.log.Error().Err(err).Msg("Failed to deliver WhatsApp alert")
		return
	}

	atomic.AddInt64(&w.alertsSent, 1)
}

// trySetCooldown checks if we can send an alert and sets the cooldown
func (w *WarRoom) trySetCooldown() bool {
	w.mu.Lock()
	defer w.mu.Unlock()

	if time.Now().Before(w.alertCooldown) {
		return false
	}
	w.alertCooldown = time.Now().Add(w.cfg.AlertCooldown)
	return true
}

// formatJID converts a phone number to WhatsApp JID format
func formatJID(phone string) string {
	if strings.Contains(phone, "@") {
		return phone
	}
	return phone + "@s.whatsapp.net"
}

// Metrics returns current metrics
func (w *WarRoom) Metrics() (alertsSent, errorsLogged int64) {
	return atomic.LoadInt64(&w.alertsSent), atomic.LoadInt64(&w.errorsLogged)
}

// Close gracefully shuts down the WarRoom
func (w *WarRoom) Close() {
	close(w.done)
}

// Done returns a channel that's closed when WarRoom is shut down
func (w *WarRoom) Done() <-chan struct{} {
	return w.done
}
