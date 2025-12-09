package monitor

import (
	"context"
	"fmt"
	"pharmabroker/domain/entity"
	"pharmabroker/internal/whatsapp"
	"strings"
	"sync"
	"time"

	"github.com/rs/zerolog"
)

// WarRoom monitors system health and sends alerts
type WarRoom struct {
	waManager  *whatsapp.Manager
	configRepo interface {
		GetAll(ctx context.Context) (*entity.AppConfig, error)
	}
	log zerolog.Logger

	mu          sync.Mutex
	errorCounts []time.Time // timestamps of recent errors

	alertCooldown time.Time
}

// NewWarRoom creates a new monitor
func NewWarRoom(wa *whatsapp.Manager, cfgRepo interface {
	GetAll(ctx context.Context) (*entity.AppConfig, error)
}, log zerolog.Logger) *WarRoom {
	return &WarRoom{
		waManager:   wa,
		configRepo:  cfgRepo,
		log:         log,
		errorCounts: make([]time.Time, 0),
	}
}

// NotifyError records an error and checks thresholds
func (w *WarRoom) NotifyError(err error) {
	w.mu.Lock()
	defer w.mu.Unlock()

	now := time.Now()
	w.errorCounts = append(w.errorCounts, now)
	w.cleanupOld(now)

	count := len(w.errorCounts)
	w.log.Warn().Err(err).Int("recent_errors", count).Msg("WarRoom received error report")

	// Threshold: 5 errors in 1 minute
	if count >= 5 {
		w.triggerAlert(context.Background(), count)
	}
}

func (w *WarRoom) cleanupOld(now time.Time) {
	// Keep errors only from last 1 minute
	cutoff := now.Add(-1 * time.Minute)
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
	if time.Now().Before(w.alertCooldown) {
		return
	}

	cfg, err := w.configRepo.GetAll(ctx)
	if err != nil || cfg.AdminPhone == "" {
		w.log.Warn().Msg("Cannot send alert: AdminPhone not configured")
		return
	}

	msg := fmt.Sprintf("🚨 *CRITICAL ALERT* 🚨\n\nHigh error rate detected: %d errors in the last minute.\nCheck logs immediately.", count)

	// Send via WhatsApp
	// Note: AdminPhone should be in JID format (e.g. 1234567890@s.whatsapp.net)
	// If it's just a number, we might need to append suffix, but let's assume config has full JID or handle it.
	targetJID := cfg.AdminPhone
	if len(targetJID) < 15 && !strings.Contains(targetJID, "@") {
		targetJID = targetJID + "@s.whatsapp.net"
	}

	w.log.Error().Str("admin", targetJID).Msg("Triggering WhatsApp Alert")
	if err := w.waManager.SendMessage(ctx, targetJID, msg); err != nil {
		w.log.Error().Err(err).Msg("Failed to deliver WhatsApp alert")
	}

	// Set cooldown to avoid spamming (10 minutes)
	w.alertCooldown = time.Now().Add(10 * time.Minute)
}
