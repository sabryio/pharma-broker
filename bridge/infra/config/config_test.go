package config

import (
	"os"
	"testing"
	"time"
)

func TestLoad_Defaults(t *testing.T) {
	// Clear any existing env vars
	os.Unsetenv("BRIDGE_GRPC_CORE_ADDR")
	os.Unsetenv("BRIDGE_WHATSAPP_STORE_PATH")
	os.Unsetenv("BRIDGE_HTTP_PORT")

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	// gRPC defaults
	if cfg.GRPC.CoreAddr != "localhost:50051" {
		t.Errorf("Expected GRPC.CoreAddr 'localhost:50051', got '%s'", cfg.GRPC.CoreAddr)
	}
	if cfg.GRPC.ConnectTimeout != 5*time.Second {
		t.Errorf("Expected GRPC.ConnectTimeout 5s, got %v", cfg.GRPC.ConnectTimeout)
	}

	// WhatsApp defaults
	if cfg.WhatsApp.StorePath != "./data/whatsapp.db" {
		t.Errorf("Expected WhatsApp.StorePath './data/whatsapp.db', got '%s'", cfg.WhatsApp.StorePath)
	}
	if cfg.WhatsApp.QRRetries != 5 {
		t.Errorf("Expected WhatsApp.QRRetries 5, got %d", cfg.WhatsApp.QRRetries)
	}
	if cfg.WhatsApp.QRTimeout != 60*time.Second {
		t.Errorf("Expected WhatsApp.QRTimeout 60s, got %v", cfg.WhatsApp.QRTimeout)
	}
	if !cfg.WhatsApp.QRTerminal {
		t.Error("Expected WhatsApp.QRTerminal to be true")
	}

	// HTTP defaults
	if cfg.HTTP.Port != "5050" {
		t.Errorf("Expected HTTP.Port '5050', got '%s'", cfg.HTTP.Port)
	}
	if cfg.HTTP.Mode != "release" {
		t.Errorf("Expected HTTP.Mode 'release', got '%s'", cfg.HTTP.Mode)
	}

	// Processing defaults
	if !cfg.Processing.SkipOwnMessages {
		t.Error("Expected Processing.SkipOwnMessages to be true")
	}
	if cfg.Processing.WorkerCount != 20 {
		t.Errorf("Expected Processing.WorkerCount 20, got %d", cfg.Processing.WorkerCount)
	}
	if cfg.Processing.WorkerQueueSize != 100 {
		t.Errorf("Expected Processing.WorkerQueueSize 100, got %d", cfg.Processing.WorkerQueueSize)
	}

	// Resilience defaults
	if cfg.Resilience.CircuitBreaker.MaxFailures != 3 {
		t.Errorf("Expected CircuitBreaker.MaxFailures 3, got %d", cfg.Resilience.CircuitBreaker.MaxFailures)
	}
	if cfg.Resilience.CircuitBreaker.Timeout != 30*time.Second {
		t.Errorf("Expected CircuitBreaker.Timeout 30s, got %v", cfg.Resilience.CircuitBreaker.Timeout)
	}
	if cfg.Resilience.RetryBuffer.MaxSize != 1000 {
		t.Errorf("Expected RetryBuffer.MaxSize 1000, got %d", cfg.Resilience.RetryBuffer.MaxSize)
	}

	// Rate limit defaults
	if cfg.RateLimit.PerMinute != 20 {
		t.Errorf("Expected RateLimit.PerMinute 20, got %f", cfg.RateLimit.PerMinute)
	}
	if cfg.RateLimit.BurstSize != 5 {
		t.Errorf("Expected RateLimit.BurstSize 5, got %d", cfg.RateLimit.BurstSize)
	}
	if !cfg.RateLimit.Enabled {
		t.Error("Expected RateLimit.Enabled to be true")
	}

	// Group sync defaults
	if cfg.GroupSync.Interval != 5*time.Minute {
		t.Errorf("Expected GroupSync.Interval 5m, got %v", cfg.GroupSync.Interval)
	}

	// Dedup defaults
	if cfg.Dedup.Window != 10*time.Second {
		t.Errorf("Expected Dedup.Window 10s, got %v", cfg.Dedup.Window)
	}
	if cfg.Dedup.CacheSize != 10000 {
		t.Errorf("Expected Dedup.CacheSize 10000, got %d", cfg.Dedup.CacheSize)
	}

	// History sync defaults
	if cfg.HistorySync.Cooldown != 5*time.Minute {
		t.Errorf("Expected HistorySync.Cooldown 5m, got %v", cfg.HistorySync.Cooldown)
	}
	if cfg.HistorySync.MaxAge != 24*time.Hour {
		t.Errorf("Expected HistorySync.MaxAge 24h, got %v", cfg.HistorySync.MaxAge)
	}

	// Reconnector defaults
	if cfg.Reconnector.InitialInterval != 5*time.Second {
		t.Errorf("Expected Reconnector.InitialInterval 5s, got %v", cfg.Reconnector.InitialInterval)
	}
	if cfg.Reconnector.Multiplier != 2.0 {
		t.Errorf("Expected Reconnector.Multiplier 2.0, got %v", cfg.Reconnector.Multiplier)
	}
}

func TestLoad_EnvOverride(t *testing.T) {
	// Set env vars with BRIDGE_ prefix
	os.Setenv("BRIDGE_GRPC_CORE_ADDR", "core:9000")
	os.Setenv("BRIDGE_HTTP_PORT", "8080")
	os.Setenv("BRIDGE_RATE_LIMIT_PER_MINUTE", "30")

	defer func() {
		os.Unsetenv("BRIDGE_GRPC_CORE_ADDR")
		os.Unsetenv("BRIDGE_HTTP_PORT")
		os.Unsetenv("BRIDGE_RATE_LIMIT_PER_MINUTE")
	}()

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if cfg.GRPC.CoreAddr != "core:9000" {
		t.Errorf("Expected GRPC.CoreAddr 'core:9000', got '%s'", cfg.GRPC.CoreAddr)
	}
	if cfg.HTTP.Port != "8080" {
		t.Errorf("Expected HTTP.Port '8080', got '%s'", cfg.HTTP.Port)
	}
	if cfg.RateLimit.PerMinute != 30 {
		t.Errorf("Expected RateLimit.PerMinute 30, got %f", cfg.RateLimit.PerMinute)
	}
}
