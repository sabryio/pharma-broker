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
