// Package domain contains core domain models for the bridge.
package domain

// Message represents a WhatsApp message in the domain layer.
// This is the canonical representation used throughout the application.
// Uses strong types for type safety and self-documenting code.
type Message struct {
	ID          MessageID
	ExternalID  MessageID
	GroupJID    JID
	GroupName   string
	SenderJID   JID
	SenderPhone Phone
	SenderName  string
	Content     string
	Timestamp   UnixTimestamp
	IsFromMe    bool
	IsGroup     bool
}

// ExtractContent extracts text content from WhatsApp message payloads.
// Handles both simple conversation and extended text messages.
func ExtractContent(conversation string, extendedText string) string {
	if conversation != "" {
		return conversation
	}
	return extendedText
}
