//go:build unix

package whatsapp

import (
	"os"
	"os/signal"
	"syscall"
)

// SetupSignalHandlers sets up OS signal handlers for Docker/Unix systems.
// Sending SIGUSR1 to the process will trigger a reconnection attempt.
func (m *Manager) SetupSignalHandlers() {
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGUSR1)
	go func() {
		for range sigChan {
			m.log.Info().Msg("Received SIGUSR1, triggering reconnect")
			m.ForceReconnect()
		}
	}()
}
