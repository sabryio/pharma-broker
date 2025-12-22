package domain

import (
	"testing"
)

func TestMessage_Fields(t *testing.T) {
	msg := Message{
		ID:          "msg123",
		ExternalID:  "ext123",
		GroupJID:    "group@g.us",
		GroupName:   "Test Group",
		SenderJID:   "sender@s.whatsapp.net",
		SenderPhone: "1234567890",
		SenderName:  "John Doe",
		Content:     "Hello, World!",
		Timestamp:   1234567890,
		IsFromMe:    false,
		IsGroup:     true,
	}

	if msg.ID != "msg123" {
		t.Errorf("Expected ID 'msg123', got '%s'", msg.ID)
	}
	if msg.GroupJID != "group@g.us" {
		t.Errorf("Expected GroupJID 'group@g.us', got '%s'", msg.GroupJID)
	}
	if msg.SenderPhone != "1234567890" {
		t.Errorf("Expected SenderPhone '1234567890', got '%s'", msg.SenderPhone)
	}
	if msg.Timestamp != 1234567890 {
		t.Errorf("Expected Timestamp 1234567890, got %d", msg.Timestamp)
	}
}

func TestExtractContent(t *testing.T) {
	tests := []struct {
		name         string
		conversation string
		extendedText string
		expected     string
	}{
		{"conversation only", "Hello", "", "Hello"},
		{"extended only", "", "Extended", "Extended"},
		{"both prefers conversation", "Conv", "Ext", "Conv"},
		{"both empty", "", "", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ExtractContent(tt.conversation, tt.extendedText)
			if result != tt.expected {
				t.Errorf("Expected '%s', got '%s'", tt.expected, result)
			}
		})
	}
}
