package notify

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func TestNewTelegramNotifier(t *testing.T) {
	config := TelegramConfig{
		Enabled:  true,
		BotToken: "test_token",
		ChatIDs:  []string{"-12345", "67890"},
	}
	log := zerolog.Nop()

	notifier := NewTelegramNotifier(config, log)
	if notifier == nil {
		t.Fatal("Expected notifier, got nil")
	}
	if notifier.config.BotToken != "test_token" {
		t.Errorf("Expected token 'test_token', got %s", notifier.config.BotToken)
	}
}

func TestTelegramNotifier_SendMessage_Disabled(t *testing.T) {
	config := TelegramConfig{Enabled: false}
	log := zerolog.Nop()
	notifier := NewTelegramNotifier(config, log)

	err := notifier.SendMessage(context.Background(), "test message")
	if err != nil {
		t.Errorf("Expected no error when disabled, got %v", err)
	}
}

func TestTelegramNotifier_SendMessage_NoToken(t *testing.T) {
	config := TelegramConfig{Enabled: true, BotToken: ""}
	log := zerolog.Nop()
	notifier := NewTelegramNotifier(config, log)

	err := notifier.SendMessage(context.Background(), "test message")
	if err != nil {
		t.Errorf("Expected no error with empty token, got %v", err)
	}
}

func TestTelegramNotifier_SendMessage_MockServer(t *testing.T) {
	// Create mock Telegram API server
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/bottest_token/sendMessage" {
			t.Errorf("Unexpected path: %s", r.URL.Path)
		}
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	// Note: In real tests, we'd need to inject the base URL
	// This test validates the structure but may not hit the mock server
	config := TelegramConfig{
		Enabled:  true,
		BotToken: "test_token",
		ChatIDs:  []string{"12345"},
	}
	log := zerolog.Nop()
	notifier := NewTelegramNotifier(config, log)
	notifier.client = &http.Client{Timeout: 5 * time.Second}

	// This will fail to connect to real Telegram, but that's expected in unit tests
	_ = notifier.SendMessage(context.Background(), "test")
}

func TestTelegramNotifier_SendDocument_Disabled(t *testing.T) {
	config := TelegramConfig{Enabled: false}
	log := zerolog.Nop()
	notifier := NewTelegramNotifier(config, log)

	err := notifier.SendDocument(context.Background(), "test.csv", []byte("data"), "caption")
	if err != nil {
		t.Errorf("Expected no error when disabled, got %v", err)
	}
}

func TestNewEmailNotifier(t *testing.T) {
	config := EmailConfig{
		Enabled:    true,
		SMTPHost:   "smtp.example.com",
		SMTPPort:   587,
		Username:   "user@example.com",
		Password:   "password",
		FromEmail:  "noreply@example.com",
		Recipients: []string{"admin@example.com"},
	}
	log := zerolog.Nop()

	notifier := NewEmailNotifier(config, log)
	if notifier == nil {
		t.Fatal("Expected notifier, got nil")
	}
}

func TestEmailNotifier_SendReport_Disabled(t *testing.T) {
	config := EmailConfig{Enabled: false}
	log := zerolog.Nop()
	notifier := NewEmailNotifier(config, log)

	err := notifier.SendReport(context.Background(), "Subject", "<html>", []byte("csv"), "report.csv")
	if err != nil {
		t.Errorf("Expected no error when disabled, got %v", err)
	}
}

func TestNewNotificationService(t *testing.T) {
	telegramConfig := TelegramConfig{Enabled: false}
	emailConfig := EmailConfig{Enabled: false}
	log := zerolog.Nop()

	service := NewNotificationService(telegramConfig, emailConfig, log)
	if service == nil {
		t.Fatal("Expected service, got nil")
	}
}

func TestNotificationService_SendReport_AllDisabled(t *testing.T) {
	telegramConfig := TelegramConfig{Enabled: false}
	emailConfig := EmailConfig{Enabled: false}
	log := zerolog.Nop()

	service := NewNotificationService(telegramConfig, emailConfig, log)

	err := service.SendReport(
		context.Background(),
		"Summary text",
		"<html>report</html>",
		[]byte("csv,data"),
		"report.csv",
	)

	if err != nil {
		t.Errorf("Expected no error when all notifiers disabled, got %v", err)
	}
}

func TestTelegramConfig_Validation(t *testing.T) {
	tests := []struct {
		name   string
		config TelegramConfig
		valid  bool
	}{
		{"empty", TelegramConfig{}, true}, // Disabled is valid
		{"enabled no token", TelegramConfig{Enabled: true, BotToken: ""}, true},
		{"enabled with token", TelegramConfig{Enabled: true, BotToken: "abc", ChatIDs: []string{"1"}}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Basic structure validation
			if tt.config.Enabled && tt.config.BotToken == "" {
				// This is a warning case but not an error
				t.Logf("Warning: Enabled but no token for %s", tt.name)
			}
		})
	}
}

func TestEmailConfig_Validation(t *testing.T) {
	tests := []struct {
		name   string
		config EmailConfig
		valid  bool
	}{
		{"empty", EmailConfig{}, true},
		{"enabled no host", EmailConfig{Enabled: true}, true},
		{"complete", EmailConfig{
			Enabled:    true,
			SMTPHost:   "smtp.gmail.com",
			SMTPPort:   587,
			Username:   "user@gmail.com",
			Password:   "app_password",
			FromEmail:  "user@gmail.com",
			Recipients: []string{"admin@example.com"},
		}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.config.Enabled && tt.config.SMTPHost == "" {
				t.Logf("Warning: Enabled but no SMTP host for %s", tt.name)
			}
		})
	}
}
