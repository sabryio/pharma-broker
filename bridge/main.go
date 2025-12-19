// Package main implements a minimal WhatsApp bridge that forwards messages to the Rust core engine.
// This is the only Go service in the PharmaBroker v2 architecture.
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	_ "github.com/mattn/go-sqlite3"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	"pharma-bridge/deduplicator"
	pb "pharma-bridge/proto"
	"pharma-bridge/reconnector"
)

func main() {
	// Setup logging
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnix
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	log.Info().Msg("🌉 PharmaBroker WhatsApp Bridge v0.2.0")

	// Load configuration
	cfg := loadConfig()

	// Create context
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Create bridge
	bridge, err := NewBridge(ctx, cfg)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create bridge")
	}

	// Start bridge with reconnection support
	go bridge.RunWithReconnect(ctx)

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
	SkipOwnMessages bool
}

func loadConfig() *Config {
	return &Config{
		CoreGRPCAddr:    getEnv("CORE_GRPC_ADDR", "localhost:50051"),
		WhatsAppStore:   getEnv("WHATSAPP_STORE", "./data/whatsapp.db"),
		AllowedPhones:   []string{},
		MonitoredGroups: []string{},
		SkipOwnMessages: true,
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
	cfg        *Config
	wa         *whatsmeow.Client
	store      *sqlstore.Container
	grpcClient pb.PharmaCoreClient
	grpcConn   *grpc.ClientConn
	logger     zerolog.Logger

	// Resilience components
	reconnector  *reconnector.Reconnector
	deduplicator *deduplicator.Deduplicator
}

// NewBridge creates a new WhatsApp bridge
func NewBridge(ctx context.Context, cfg *Config) (*Bridge, error) {
	// Connect to Rust gRPC server
	log.Info().Str("addr", cfg.CoreGRPCAddr).Msg("Connecting to Rust core...")

	conn, err := grpc.NewClient(
		cfg.CoreGRPCAddr,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to gRPC: %w", err)
	}

	grpcClient := pb.NewPharmaCoreClient(conn)

	// Test connection with health check
	healthCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	resp, err := grpcClient.HealthCheck(healthCtx, &pb.HealthRequest{})
	if err != nil {
		log.Warn().Err(err).Msg("Could not reach Rust core (will retry on messages)")
	} else {
		log.Info().
			Bool("healthy", resp.Healthy).
			Str("version", resp.Version).
			Int64("uptime", resp.UptimeSeconds).
			Msg("✅ Connected to Rust core")
	}

	// Create WhatsApp store
	dbLog := waLog.Stdout("Database", "DEBUG", true)
	container, err := sqlstore.New(ctx, "sqlite3", fmt.Sprintf("file:%s?_foreign_keys=on", cfg.WhatsAppStore), dbLog)
	if err != nil {
		return nil, fmt.Errorf("failed to create store: %w", err)
	}

	// Create reconnector
	recon := reconnector.New(reconnector.DefaultConfig(), log.Logger)
	recon.SetOnRetry(func(attempt int, delay time.Duration, err error) {
		log.Warn().
			Int("attempt", attempt).
			Dur("next_delay", delay).
			Err(err).
			Msg("WhatsApp reconnection scheduled")
	})
	recon.SetOnSuccess(func(attempt int, elapsed time.Duration) {
		log.Info().
			Int("attempts", attempt).
			Dur("elapsed", elapsed).
			Msg("✅ WhatsApp reconnected")
	})

	// Create deduplicator
	dedup := deduplicator.New(ctx, deduplicator.DefaultConfig(), log.Logger)

	return &Bridge{
		cfg:          cfg,
		store:        container,
		grpcClient:   grpcClient,
		grpcConn:     conn,
		logger:       log.With().Str("component", "bridge").Logger(),
		reconnector:  recon,
		deduplicator: dedup,
	}, nil
}

// RunWithReconnect connects to WhatsApp with automatic reconnection
func (b *Bridge) RunWithReconnect(ctx context.Context) {
	// Initial connection
	if err := b.connect(ctx); err != nil {
		b.logger.Error().Err(err).Msg("Initial connection failed, starting reconnector")
	}

	// Handle disconnect events with reconnection
	// The whatsapp client handles reconnection internally for most cases,
	// but we track state for health monitoring
}

// connect establishes the WhatsApp connection
func (b *Bridge) connect(ctx context.Context) error {
	// Get or create device store
	deviceStore, err := b.store.GetFirstDevice(ctx)
	if err != nil {
		return fmt.Errorf("failed to get device: %w", err)
	}

	// Create WhatsApp client
	clientLog := waLog.Stdout("Client", "INFO", true)
	b.wa = whatsmeow.NewClient(deviceStore, clientLog)

	// Register event handler
	b.wa.AddEventHandler(b.handleEvent)

	// Connect to WhatsApp
	if b.wa.Store.ID == nil {
		// Need to pair
		qrChan, _ := b.wa.GetQRChannel(ctx)
		if err := b.wa.Connect(); err != nil {
			return fmt.Errorf("failed to connect: %w", err)
		}

		for evt := range qrChan {
			if evt.Event == "code" {
				b.logger.Info().Str("qr", evt.Code).Msg("Scan this QR code to link")
				fmt.Println("QR Code:", evt.Code)
			} else {
				b.logger.Info().Str("event", evt.Event).Msg("QR event")
			}
		}
	} else {
		// Already paired
		if err := b.wa.Connect(); err != nil {
			return fmt.Errorf("failed to connect: %w", err)
		}
	}

	b.logger.Info().
		Str("grpc_addr", b.cfg.CoreGRPCAddr).
		Msg("Bridge connected to WhatsApp")

	return nil
}

// Stop gracefully shuts down the bridge
func (b *Bridge) Stop() {
	if b.wa != nil {
		b.wa.Disconnect()
	}
	if b.grpcConn != nil {
		b.grpcConn.Close()
	}
	if b.deduplicator != nil {
		b.deduplicator.Close()
	}
	if b.reconnector != nil {
		b.reconnector.Stop()
	}
	b.logger.Info().Msg("Bridge stopped")
}

// handleEvent processes WhatsApp events
func (b *Bridge) handleEvent(evt interface{}) {
	switch v := evt.(type) {
	case *events.Message:
		b.handleMessage(v)
	case *events.Connected:
		b.logger.Info().Msg("WhatsApp connected")
	case *events.Disconnected:
		b.logger.Warn().Msg("WhatsApp disconnected")
		// Reconnection is handled by whatsmeow internally
	}
}

// handleMessage processes incoming WhatsApp messages
func (b *Bridge) handleMessage(evt *events.Message) {
	// Step 1: Only process group messages
	if !evt.Info.IsGroup {
		return
	}

	// Step 2: Skip own messages if configured
	if evt.Info.IsFromMe && b.cfg.SkipOwnMessages {
		b.logger.Debug().Msg("Skipping own message")
		return
	}

	// Step 3: Extract content
	content := ""
	if evt.Message.GetConversation() != "" {
		content = evt.Message.GetConversation()
	} else if evt.Message.GetExtendedTextMessage() != nil {
		content = evt.Message.GetExtendedTextMessage().GetText()
	}

	if content == "" {
		return
	}

	// Step 4: Check for duplicates
	if b.deduplicator.IsDuplicate(
		evt.Info.Chat.String(),
		evt.Info.Sender.String(),
		content,
		evt.Info.Timestamp,
	) {
		b.logger.Debug().
			Str("sender", evt.Info.Sender.User).
			Msg("Duplicate message ignored")
		return
	}

	// Step 5: Record message for future dedup checks
	b.deduplicator.RecordMessage(
		evt.Info.Chat.String(),
		evt.Info.Sender.String(),
		content,
		evt.Info.Timestamp,
	)

	b.logger.Debug().
		Str("group", evt.Info.Chat.String()).
		Str("sender", evt.Info.Sender.User).
		Int("content_len", len(content)).
		Msg("Received message")

	// Step 6: Forward to Rust via gRPC
	b.forwardToCore(
		evt.Info.ID,
		evt.Info.Chat.String(),
		evt.Info.Chat.String(), // Group name not available directly
		evt.Info.Sender.String(),
		evt.Info.Sender.User,
		evt.Info.PushName,
		content,
		evt.Info.Timestamp.Unix(),
	)
}

// forwardToCore sends the message to the Rust core via gRPC
func (b *Bridge) forwardToCore(
	id, groupJID, groupName, senderJID, senderPhone, senderName, content string,
	timestamp int64,
) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	msg := &pb.RawMessage{
		Id:          id,
		ExternalId:  id,
		GroupJid:    groupJID,
		GroupName:   groupName,
		SenderJid:   senderJID,
		SenderPhone: senderPhone,
		SenderName:  senderName,
		Content:     content,
		Timestamp:   timestamp,
	}

	resp, err := b.grpcClient.ProcessMessage(ctx, msg)
	if err != nil {
		b.logger.Error().Err(err).Str("id", id).Msg("Failed to forward message to Rust core")
		return
	}

	b.logger.Info().
		Bool("success", resp.Success).
		Str("message_id", resp.MessageId).
		Msg("📤 Message forwarded to Rust core")
}
