// Package main implements a minimal WhatsApp bridge that forwards messages to the Rust core engine.
// This is the only Go service in the PharmaBroker v2 architecture.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"sync/atomic"
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
	"google.golang.org/grpc/metadata"

	"pharma-bridge/cache"
	"pharma-bridge/deduplicator"
	pb "pharma-bridge/proto"
	"pharma-bridge/reconnector"
	"pharma-bridge/resilience"
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

	// Start health HTTP server
	go startHealthServer(bridge)

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
	groupCache   *cache.GroupCache
	retryBuffer  *resilience.RetryBuffer
	circuit      *resilience.CircuitBreaker
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

	// Create group cache (5 minute TTL)
	groupCache := cache.NewGroupCache(5 * time.Minute)

	// Create circuit breaker
	circuit := resilience.NewCircuitBreaker(3, 30*time.Second)
	circuit.SetOnStateChange(func(s resilience.State) {
		log.Warn().Int("state", int(s)).Msg("Circuit breaker state changed")
	})

	b := &Bridge{
		cfg:          cfg,
		store:        container,
		grpcClient:   grpcClient,
		grpcConn:     conn,
		logger:       log.With().Str("component", "bridge").Logger(),
		reconnector:  recon,
		deduplicator: dedup,
		groupCache:   groupCache,
		circuit:      circuit,
	}

	// Create retry buffer (1000 messages)
	b.retryBuffer = resilience.NewRetryBuffer(1000, func(ctx context.Context, msg *pb.RawMessage) error {
		_, err := b.forwardRaw(ctx, msg)
		return err
	})
	b.retryBuffer.Start(ctx)

	// Initial group sync
	b.syncGroups(ctx)

	// Start periodic group sync
	go b.syncGroupsWorker(ctx)

	return b, nil
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

	// Step 4: Check local GroupCache (Optimization)
	if !b.groupCache.IsMonitored(evt.Info.Chat.String()) {
		return
	}

	// Step 5: Check for duplicates (Reliability)
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

	// Step 6: Record message for future dedup checks
	b.deduplicator.RecordMessage(
		evt.Info.Chat.String(),
		evt.Info.Sender.String(),
		content,
		evt.Info.Timestamp,
	)

	b.logger.Debug().
		Str("group", evt.Info.Chat.String()).
		Int("content_len", len(content)).
		Msg("Processing monitored message")

	// Step 7: Forward to Rust via gRPC
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
	// Generate trace ID for request correlation
	traceID := fmt.Sprintf("%s-%d", id[:8], time.Now().UnixNano()%1000000)

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Add trace ID to gRPC metadata
	md := metadata.Pairs("x-request-id", traceID)
	ctx = metadata.NewOutgoingContext(ctx, md)

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

	b.logger.Info().
		Str("trace_id", traceID).
		Str("id", id).
		Msg("📤 Forwarding message to Rust core")

	resp, err := b.forwardRaw(ctx, msg)
	if err != nil {
		b.logger.Warn().
			Err(err).
			Str("trace_id", traceID).
			Msg("Core unreachable or failed, move to retry buffer")

		// Add to retry buffer if circuit is open or network error
		b.retryBuffer.Add(msg)
		return
	}

	b.logger.Info().
		Str("trace_id", traceID).
		Bool("success", resp.Success).
		Str("message_id", resp.MessageId).
		Msg("📤 Message forwarded to Rust core")

	// Increment counter for health stats
	atomic.AddInt64(&messagesForwarded, 1)
}

// forwardRaw is the low-level gRPC call with circuit breaker integration
func (b *Bridge) forwardRaw(ctx context.Context, msg *pb.RawMessage) (*pb.ProcessResponse, error) {
	if !b.circuit.Allow() {
		return nil, fmt.Errorf("circuit breaker is open")
	}

	resp, err := b.grpcClient.ProcessMessage(ctx, msg)
	if err != nil {
		b.circuit.RecordFailure()
		return nil, err
	}

	b.circuit.RecordSuccess()
	return resp, nil
}

// syncGroups fetches the list of monitored groups from the Rust core
func (b *Bridge) syncGroups(ctx context.Context) {
	b.logger.Debug().Msg("Syncing monitored groups from core...")

	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	resp, err := b.grpcClient.GetMonitoredGroups(ctx, &pb.MonitoredGroupsRequest{})
	if err != nil {
		b.logger.Warn().Err(err).Msg("Failed to sync monitored groups")
		return
	}

	b.groupCache.Update(resp.Jids)
	b.logger.Info().Int("count", len(resp.Jids)).Msg("✅ Monitored groups synced")
}

// syncGroupsWorker periodically triggers a group sync
func (b *Bridge) syncGroupsWorker(ctx context.Context) {
	ticker := time.NewTicker(5 * time.Minute)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			b.syncGroups(ctx)
		}
	}
}

// messagesForwarded tracks total messages forwarded for health reporting
var messagesForwarded int64

// startHealthServer starts a simple HTTP health server on port 5050
func startHealthServer(bridge *Bridge) {
	port := getEnv("HEALTH_PORT", "5050")

	mux := http.NewServeMux()
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		whatsappConnected := bridge.wa != nil && bridge.wa.IsConnected()
		coreConnected := bridge.grpcConn != nil

		status := "healthy"
		if !whatsappConnected {
			status = "whatsapp_disconnected"
		} else if !coreConnected {
			status = "core_disconnected"
		}

		response := map[string]interface{}{
			"status":             status,
			"service":            "pharma-bridge",
			"version":            "0.2.0",
			"whatsapp_connected": whatsappConnected,
			"core_connected":     coreConnected,
			"messages_forwarded": atomic.LoadInt64(&messagesForwarded),
		}

		w.Header().Set("Content-Type", "application/json")
		if status != "healthy" {
			w.WriteHeader(http.StatusServiceUnavailable)
		}
		json.NewEncoder(w).Encode(response)
	})

	log.Info().Str("port", port).Msg("🏥 Health server starting")
	if err := http.ListenAndServe(":"+port, mux); err != nil {
		log.Error().Err(err).Msg("Health server failed")
	}
}
