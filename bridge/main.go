// Package main implements a minimal WhatsApp bridge that forwards messages to the Rust core engine.
// This is the only Go service in the PharmaBroker v2 architecture.
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

func main() {
	// Setup logging
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnix
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	log.Info().Msg("🌉 PharmaBroker WhatsApp Bridge starting...")

	// Load configuration
	cfg := loadConfig()

	// Create bridge
	bridge, err := NewBridge(cfg)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create bridge")
	}

	// Start bridge
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	if err := bridge.Start(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to start bridge")
	}

	// Wait for shutdown signal
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	log.Info().Msg("Shutting down bridge...")
	bridge.Stop()
}

// Config holds bridge configuration
type Config struct {
	CoreGRPCAddr    string
	WhatsAppStore   string
	AllowedPhones   []string
	MonitoredGroups []string
}

func loadConfig() *Config {
	return &Config{
		CoreGRPCAddr:    getEnv("CORE_GRPC_ADDR", "localhost:50051"),
		WhatsAppStore:   getEnv("WHATSAPP_STORE", "./data/whatsapp"),
		AllowedPhones:   []string{}, // TODO: Load from env
		MonitoredGroups: []string{}, // TODO: Load from env
	}
}

func getEnv(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}

// Bridge connects WhatsApp to the Rust core engine
type Bridge struct {
	cfg *Config
	// wa     *whatsmeow.Client // TODO: Uncomment when whatsmeow is added
	// grpc   pb.PharmaCoreClient // TODO: Add gRPC client
	logger zerolog.Logger
}

// NewBridge creates a new WhatsApp bridge
func NewBridge(cfg *Config) (*Bridge, error) {
	return &Bridge{
		cfg:    cfg,
		logger: log.With().Str("component", "bridge").Logger(),
	}, nil
}

// Start begins the bridge operations
func (b *Bridge) Start(ctx context.Context) error {
	b.logger.Info().
		Str("grpc_addr", b.cfg.CoreGRPCAddr).
		Str("store", b.cfg.WhatsAppStore).
		Msg("Bridge started")

	// TODO: Connect to Rust gRPC server
	// TODO: Initialize WhatsApp client
	// TODO: Register message handler

	fmt.Println("WhatsApp Bridge is running...")
	fmt.Println("Press Ctrl+C to stop")

	return nil
}

// Stop gracefully shuts down the bridge
func (b *Bridge) Stop() {
	b.logger.Info().Msg("Bridge stopped")
	// TODO: Disconnect WhatsApp
	// TODO: Close gRPC connection
}

// handleMessage processes incoming WhatsApp messages
// This is the core function that forwards messages to Rust
func (b *Bridge) handleMessage(groupJID, senderPhone, senderName, content string) {
	b.logger.Debug().
		Str("group", groupJID).
		Str("sender", senderPhone).
		Str("content", content[:min(50, len(content))]).
		Msg("Received message")

	// TODO: Forward to Rust via gRPC
	// b.grpc.ProcessMessage(ctx, &pb.RawMessage{
	// 	GroupJid:    groupJID,
	// 	SenderPhone: senderPhone,
	// 	SenderName:  senderName,
	// 	Content:     content,
	// 	Timestamp:   time.Now().Unix(),
	// })
}
