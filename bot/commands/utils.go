package commands

import "strings"

// extractPhone extracts phone number from platform-specific sender ID.
// Handles WhatsApp JID format: 201234567890@s.whatsapp.net
// Handles Telegram format: numeric user ID
func extractPhone(senderID string) string {
	// Handle WhatsApp JID format
	if idx := strings.Index(senderID, "@"); idx > 0 {
		return senderID[:idx]
	}
	return senderID
}
