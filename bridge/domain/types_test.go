package domain

import (
	"testing"
)

func TestMessageID(t *testing.T) {
	id := MessageID("ABC123DEF456")

	if id.String() != "ABC123DEF456" {
		t.Errorf("Expected 'ABC123DEF456', got '%s'", id.String())
	}

	if id.Short() != "ABC123DE" {
		t.Errorf("Expected 'ABC123DE', got '%s'", id.Short())
	}

	shortID := MessageID("ABC")
	if shortID.Short() != "ABC" {
		t.Errorf("Expected 'ABC', got '%s'", shortID.Short())
	}
}

func TestJID(t *testing.T) {
	groupJID := JID("123456789@g.us")
	userJID := JID("123456789@s.whatsapp.net")

	if !groupJID.IsGroup() {
		t.Error("Expected groupJID.IsGroup() to be true")
	}
	if groupJID.IsUser() {
		t.Error("Expected groupJID.IsUser() to be false")
	}

	if userJID.IsGroup() {
		t.Error("Expected userJID.IsGroup() to be false")
	}
	if !userJID.IsUser() {
		t.Error("Expected userJID.IsUser() to be true")
	}

	if userJID.Phone() != "123456789" {
		t.Errorf("Expected phone '123456789', got '%s'", userJID.Phone())
	}
}

func TestPhone(t *testing.T) {
	phone := Phone("1234567890")
	if phone.String() != "1234567890" {
		t.Errorf("Expected '1234567890', got '%s'", phone.String())
	}
}

func TestTraceID(t *testing.T) {
	msgID := MessageID("ABC123DEF456")
	traceID := NewTraceID(msgID, 123456789)

	// The trace ID format is: short_id-last_6_digits
	// 123456789 % 1000000 = 456789
	expected := "ABC123DE-456789"
	if traceID.String() != expected {
		t.Errorf("Expected '%s', got '%s'", expected, traceID.String())
	}
}

func TestUnixTimestamp(t *testing.T) {
	ts := UnixTimestamp(1234567890)
	if ts.Int64() != 1234567890 {
		t.Errorf("Expected 1234567890, got %d", ts.Int64())
	}
}

func TestVersion(t *testing.T) {
	if CurrentVersion.String() != "0.5.0" {
		t.Errorf("Expected '0.5.0', got '%s'", CurrentVersion.String())
	}
}

// Feature: send-message, Property 3: Invalid JID Format Rejected
// Validates: Requirements 1.3, 3.1

func TestParseJID_ValidFormats(t *testing.T) {
	validJIDs := []struct {
		input    string
		expected JID
	}{
		{"201234567890@s.whatsapp.net", JID("201234567890@s.whatsapp.net")},
		{"120363123456789012@g.us", JID("120363123456789012@g.us")},
		{"abc123@lid", JID("abc123@lid")},
		{"1@s.whatsapp.net", JID("1@s.whatsapp.net")},
	}

	for _, tc := range validJIDs {
		t.Run(tc.input, func(t *testing.T) {
			jid, err := ParseJID(tc.input)
			if err != nil {
				t.Errorf("ParseJID(%q) returned error: %v", tc.input, err)
			}
			if jid != tc.expected {
				t.Errorf("ParseJID(%q) = %q, want %q", tc.input, jid, tc.expected)
			}
		})
	}
}

func TestParseJID_InvalidFormats(t *testing.T) {
	invalidJIDs := []struct {
		input   string
		wantErr string
	}{
		{"", "cannot be empty"},
		{"invalid", "exactly one @"},
		{"@s.whatsapp.net", "identifier part cannot be empty"},
		{"123@", "server part cannot be empty"},
		{"123@unknown.net", "invalid server"},
		{"123@@s.whatsapp.net", "exactly one @"},
		{"a@b@c", "exactly one @"},
	}

	for _, tc := range invalidJIDs {
		t.Run(tc.input, func(t *testing.T) {
			_, err := ParseJID(tc.input)
			if err == nil {
				t.Errorf("ParseJID(%q) expected error, got nil", tc.input)
				return
			}
			if tc.wantErr != "" && !contains(err.Error(), tc.wantErr) {
				t.Errorf("ParseJID(%q) error = %q, want error containing %q", tc.input, err.Error(), tc.wantErr)
			}
		})
	}
}

func TestIsValidJID(t *testing.T) {
	if !IsValidJID("201234567890@s.whatsapp.net") {
		t.Error("Expected valid JID to return true")
	}
	if IsValidJID("invalid") {
		t.Error("Expected invalid JID to return false")
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(len(s) > 0 && len(substr) > 0 && findSubstring(s, substr)))
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
