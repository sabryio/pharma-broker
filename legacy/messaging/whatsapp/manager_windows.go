//go:build windows

package whatsapp

// SetupSignalHandlers is a no-op on Windows as SIGUSR1 is not available.
// Use ForceReconnect() directly or implement a different mechanism for Windows.
func (m *Manager) SetupSignalHandlers() {
	m.log.Info().Msg("Signal handlers not available on Windows, use ForceReconnect() API")
}
