package core

import (
	"testing"
)

func TestIsCommand(t *testing.T) {
	tests := []struct {
		text     string
		expected bool
	}{
		{"/status", true},
		{"/help", true},
		{" /status", true},
		{"status", false},
		{"hello /status", false},
		{"", false},
		{"البحث عن دواء", false},
	}

	for _, tt := range tests {
		t.Run(tt.text, func(t *testing.T) {
			result := IsCommand(tt.text)
			if result != tt.expected {
				t.Errorf("IsCommand(%q) = %v, want %v", tt.text, result, tt.expected)
			}
		})
	}
}

func TestParseCommand(t *testing.T) {
	tests := []struct {
		text        string
		expectedCmd string
		expectedLen int
	}{
		{"/status", "status", 0},
		{"/confirm abc123", "confirm", 1},
		{"/reject abc123 extra", "reject", 2},
		{"/HELP", "help", 0},
		{"not a command", "", 0},
		{"/", "", 0},
	}

	for _, tt := range tests {
		t.Run(tt.text, func(t *testing.T) {
			result := ParseCommand(tt.text)
			if tt.expectedCmd == "" {
				if result != nil {
					t.Errorf("ParseCommand(%q) = %v, want nil", tt.text, result)
				}
				return
			}
			if result == nil {
				t.Errorf("ParseCommand(%q) = nil, want command", tt.text)
				return
			}
			if result.Name != tt.expectedCmd {
				t.Errorf("ParseCommand(%q).Name = %q, want %q", tt.text, result.Name, tt.expectedCmd)
			}
			if len(result.Args) != tt.expectedLen {
				t.Errorf("ParseCommand(%q).Args len = %d, want %d", tt.text, len(result.Args), tt.expectedLen)
			}
		})
	}
}

func TestNormalizePhone(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"+201234567890", "201234567890"},
		{"20 123 456 7890", "201234567890"},
		{"20-123-456-7890", "201234567890"},
		{"(20) 123-456-7890", "201234567890"},
		{"201234567890", "201234567890"},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := NormalizePhone(tt.input)
			if result != tt.expected {
				t.Errorf("NormalizePhone(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}
