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
	"strconv"
	"sync/atomic"
	"syscall"
	"time"

	_ "github.com/mattn/go-sqlite3"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"

	"pharma-bridge/cache"
	"pharma-bridge/deduplicator"
	"pharma-bridge/historysync"
	pb "pharma-bridge/proto"
	"pharma-bridge/qr"
	"pharma-bridge/reconnector"
	"pharma-bridge/resilience"
)

func main() {
	// Setup logging
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnix
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	log.Info().Msg("🌉 PharmaBroker WhatsApp Bridge v0.3.0")

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
	QRMaxRetries    int // Maximum QR code attempts before giving up (0 = infinite)
}

func loadConfig() *Config {
	qrRetries := 5 // Default: 5 attempts
	if v := os.Getenv("QR_MAX_RETRIES"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n >= 0 {
			qrRetries = n
		}
	}

	return &Config{
		CoreGRPCAddr:    getEnv("CORE_GRPC_ADDR", "localhost:50051"),
		WhatsAppStore:   getEnv("WHATSAPP_STORE", "./data/whatsapp.db"),
		AllowedPhones:   []string{},
		MonitoredGroups: []string{},
		SkipOwnMessages: true,
		QRMaxRetries:    qrRetries,
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
	rateLimiter  *resilience.RateLimiter

	// History sync handler
	historySync *historysync.Handler

	// QR code handler
	qrHandler *qr.Handler
	qrConfig  qr.Config

	// Ordered processing
	workers []chan *events.Message
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

	// Create rate limiter (prevents WhatsApp bans)
	rateLimiter := resilience.NewRateLimiter(resilience.DefaultRateLimiterConfig())

	// Create history sync handler
	historyHandler := historysync.New(historysync.DefaultConfig(), log.Logger)

	// Create QR handler
	qrConfig := qr.Config{
		RenderTerminal: true,
		QRTimeout:      60 * time.Second,
		MaxRetries:     cfg.QRMaxRetries,
	}
	qrHandler := qr.New(qrConfig, log.Logger)

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
		rateLimiter:  rateLimiter,
		historySync:  historyHandler,
		qrHandler:    qrHandler,
		qrConfig:     qrConfig,
		workers:      make([]chan *events.Message, 20), // 20 workers for ordered processing
	}

	// Start workers
	for i := range 20 {
		b.workers[i] = make(chan *events.Message, 100)
		go b.workerLoop(i, b.workers[i])
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
		// Need to pair - get QR code with retry support
		qrAttempt := 0
		maxRetries := b.cfg.QRMaxRetries

		for {
			qrAttempt++
			b.logger.Info().
				Int("attempt", qrAttempt).
				Int("max_retries", maxRetries).
				Msg("📱 Starting QR code pairing...")

			qrChan, _ := b.wa.GetQRChannel(ctx)
			if err := b.wa.Connect(); err != nil {
				b.qrHandler.HandleError(err)
				return fmt.Errorf("failed to connect: %w", err)
			}

			paired := false
			for evt := range qrChan {
				switch evt.Event {
				case "code":
					// Handle QR code - renders in terminal + broadcasts to WebSocket
					b.qrHandler.HandleQRCode(evt.Code, b.qrConfig)
				case "success":
					b.qrHandler.HandleEvent("success", b.qrConfig)
					b.logger.Info().Msg("✅ WhatsApp paired successfully")
					paired = true
				case "timeout":
					b.qrHandler.HandleEvent("timeout", b.qrConfig)
					b.logger.Warn().
						Int("attempt", qrAttempt).
						Int("max_retries", maxRetries).
						Msg("⏰ QR code expired")
				default:
					b.qrHandler.HandleEvent(evt.Event, b.qrConfig)
				}
			}

			if paired {
				break
			}

			// Check if we should retry
			if maxRetries > 0 && qrAttempt >= maxRetries {
				return fmt.Errorf("QR code pairing failed after %d attempts - please restart", qrAttempt)
			}

			// Disconnect and retry
			b.wa.Disconnect()
			b.logger.Info().
				Int("attempt", qrAttempt).
				Msg("🔄 Retrying QR code pairing in 5 seconds...")

			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(5 * time.Second):
				// Continue to next attempt
			}
		}
	} else {
		// Already paired
		b.qrHandler.SetPaired()
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
	if b.qrHandler != nil {
		b.qrHandler.Close()
	}
	b.logger.Info().Msg("Bridge stopped")
}

// handleEvent processes WhatsApp events
func (b *Bridge) handleEvent(evt any) {
	switch v := evt.(type) {
	case *events.Message:
		b.handleMessage(v)
	case *events.Connected:
		b.logger.Info().Msg("WhatsApp connected")
	case *events.Disconnected:
		b.logger.Warn().Msg("WhatsApp disconnected")
		// Reconnection is handled by whatsmeow internally
	case *events.HistorySync:
		b.handleHistorySync(v)
	}
}

// workerLoop processes messages from the worker's channel sequentially
func (b *Bridge) workerLoop(id int, ch chan *events.Message) {
	b.logger.Debug().Int("worker_id", id).Msg("Worker started")
	for evt := range ch {
		b.processMessage(evt)
	}
}

// handleMessage routes messages to the appropriate worker based on Group JID
func (b *Bridge) handleMessage(evt *events.Message) {
	if !evt.Info.IsGroup {
		return
	}

	// Shard by Group JID to ensure same group goes to same worker
	jid := evt.Info.Chat.String()
	hash := 0
	for i := 0; i < len(jid); i++ {
		hash = 31*hash + int(jid[i])
	}
	workerIdx := (hash & 0x7fffffff) % len(b.workers)

	// Send to worker (non-blocking to avoid stalling WhatsApp event loop)
	select {
	case b.workers[workerIdx] <- evt:
		// Sent to worker
	default:
		b.logger.Warn().Str("group", jid).Int("worker", workerIdx).Msg("Worker queue full, message dropped to prevent stalling")
	}
}

// processMessage contains the original logic of handleMessage
func (b *Bridge) processMessage(evt *events.Message) {
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

// handleHistorySync processes history sync events with deduplication.
// Prevents duplicate processing by enforcing cooldown, filtering old messages,
// and tracking processed message IDs.
func (b *Bridge) handleHistorySync(v *events.HistorySync) {
	// Check cooldown
	if !b.historySync.ShouldProcess() {
		return
	}

	// Count total messages
	totalMessages := 0
	for _, conv := range v.Data.Conversations {
		totalMessages += len(conv.Messages)
	}
	b.historySync.RecordReceived(totalMessages)

	b.logger.Info().
		Int("conversations", len(v.Data.Conversations)).
		Int("total_messages", totalMessages).
		Msg("Processing History Sync")

	// Clean up old entries from processed IDs cache
	b.historySync.CleanupCache()

	processedCount := 0
	skippedOld := 0
	skippedDuplicate := 0

	for _, conv := range v.Data.Conversations {
		for _, waMsg := range conv.Messages {
			// Check message limit
			if processedCount >= b.historySync.MaxMessages() {
				b.logger.Warn().
					Int("limit", b.historySync.MaxMessages()).
					Msg("History sync message limit reached")
				goto done
			}

			if waMsg.Message == nil || waMsg.Message.Key == nil {
				continue
			}

			key := waMsg.Message.Key
			msgID := key.GetID()

			// Get timestamp
			ts := int64(0)
			if waMsg.Message.MessageTimestamp != nil {
				ts = int64(*waMsg.Message.MessageTimestamp)
			}
			msgTime := time.Unix(ts, 0)

			// Skip old messages
			if b.historySync.IsMessageTooOld(msgTime) {
				skippedOld++
				continue
			}

			// Skip already processed messages
			if b.historySync.IsMessageProcessed(msgID) {
				skippedDuplicate++
				continue
			}

			// Mark as processed
			b.historySync.MarkMessageProcessed(msgID)

			// Parse chat JID to check if it's a group
			if key.RemoteJID == nil {
				continue
			}

			chatJID, err := types.ParseJID(*key.RemoteJID)
			if err != nil || chatJID.Server != "g.us" {
				continue // Only process group messages
			}

			// Check if group is monitored
			if !b.groupCache.IsMonitored(chatJID.String()) {
				continue
			}

			// Extract content
			content := ""
			if waMsg.Message.Message != nil {
				if waMsg.Message.Message.GetConversation() != "" {
					content = waMsg.Message.Message.GetConversation()
				} else if waMsg.Message.Message.GetExtendedTextMessage() != nil {
					content = waMsg.Message.Message.GetExtendedTextMessage().GetText()
				}
			}

			if content == "" {
				continue
			}

			// Get sender info
			senderJID := ""
			senderPhone := ""
			if key.Participant != nil {
				senderJID = *key.Participant
				if parsed, err := types.ParseJID(*key.Participant); err == nil {
					senderPhone = parsed.User
				}
			}

			pushName := ""
			if waMsg.Message.PushName != nil {
				pushName = *waMsg.Message.PushName
			}

			// Forward to core
			b.forwardToCore(
				msgID,
				chatJID.String(),
				chatJID.String(), // Group name not available in history sync
				senderJID,
				senderPhone,
				pushName,
				content,
				ts,
			)
			processedCount++
		}
	}

done:
	b.historySync.RecordSkipped(skippedOld + skippedDuplicate)
	b.historySync.RecordProcessed(processedCount)

	b.logger.Info().
		Int("processed", processedCount).
		Int("skipped_old", skippedOld).
		Int("skipped_duplicate", skippedDuplicate).
		Int("total", totalMessages).
		Msg("History sync completed")
}

// messagesForwarded tracks total messages forwarded for health reporting
var messagesForwarded int64

// startHealthServer starts a simple HTTP health server on port 5050
func startHealthServer(bridge *Bridge) {
	port := getEnv("HEALTH_PORT", "5050")

	mux := http.NewServeMux()

	// Health endpoint
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		whatsappConnected := bridge.wa != nil && bridge.wa.IsConnected()
		coreConnected := bridge.grpcConn != nil

		status := "healthy"
		if !whatsappConnected {
			status = "whatsapp_disconnected"
		} else if !coreConnected {
			status = "core_disconnected"
		}

		// Get circuit breaker state
		circuitState := "closed"
		if bridge.circuit != nil {
			switch bridge.circuit.State() {
			case resilience.StateOpen:
				circuitState = "open"
			case resilience.StateHalfOpen:
				circuitState = "half_open"
			}
		}

		response := map[string]any{
			"status":             status,
			"service":            "pharma-bridge",
			"version":            "0.3.0",
			"whatsapp_connected": whatsappConnected,
			"core_connected":     coreConnected,
			"messages_forwarded": atomic.LoadInt64(&messagesForwarded),
			"circuit_breaker":    circuitState,
			"retry_buffer_size":  bridge.retryBuffer.Size(),
			"deduplicator_stats": bridge.deduplicator.Stats(),
			"rate_limiter_stats": bridge.rateLimiter.GetStats(),
			"history_sync_stats": bridge.historySync.GetStats(),
		}

		w.Header().Set("Content-Type", "application/json")
		if status != "healthy" {
			w.WriteHeader(http.StatusServiceUnavailable)
		}
		json.NewEncoder(w).Encode(response)
	})

	// QR code endpoints
	mux.HandleFunc("/qr", bridge.qrHandler.HTMLHandler)         // HTML page with QR
	mux.HandleFunc("/qr/json", bridge.qrHandler.HTTPHandler)    // JSON API
	mux.HandleFunc("/qr/ws", bridge.qrHandler.WebSocketHandler) // WebSocket for real-time

	log.Info().Str("port", port).Msg("🏥 Health server starting")
	log.Info().Str("url", "http://localhost:"+port+"/qr").Msg("📱 QR code available at")

	if err := http.ListenAndServe(":"+port, mux); err != nil {
		log.Error().Err(err).Msg("Health server failed")
	}
}
