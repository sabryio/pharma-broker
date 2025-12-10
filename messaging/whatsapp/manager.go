package whatsapp

import (
	"context"
	"fmt"
	"math/rand/v2"
	"os"
	"path/filepath"
	"slices"
	"sync"
	"sync/atomic"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"
	"go.mau.fi/whatsmeow"
	"go.mau.fi/whatsmeow/proto/waE2E"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"
	"google.golang.org/protobuf/proto"

	"pharmabroker/bot/core"
	whatsappbot "pharmabroker/bot/whatsapp"
	"pharmabroker/pkg/config"
	"pharmabroker/pkg/metrics"
)

// Constants for timeouts and limits
const (
	defaultConnectTimeout = 30 * time.Second
	groupInfoTimeout      = 5 * time.Second
	botResponseTimeout    = 30 * time.Second
	qrChannelBufferSize   = 1

	// Default reconnection settings
	defaultMaxReconnectAttempts = 0 // 0 = infinite
	defaultBaseDelay            = 5 * time.Second
	defaultMaxDelay             = 5 * time.Minute
	defaultJitterFactor         = 0.1 // 10% jitter
)

// ConnectionState represents the current state of the WhatsApp connection.
type ConnectionState int32

const (
	StateDisconnected ConnectionState = iota
	StateConnecting
	StateConnected
	StateReconnecting
	StateFailed // Max attempts reached
)

// String returns the string representation of the connection state.
func (s ConnectionState) String() string {
	switch s {
	case StateDisconnected:
		return "DISCONNECTED"
	case StateConnecting:
		return "CONNECTING"
	case StateConnected:
		return "CONNECTED"
	case StateReconnecting:
		return "RECONNECTING"
	case StateFailed:
		return "FAILED"
	default:
		return "UNKNOWN"
	}
}

// ReconnectConfig holds configuration for reconnection behavior.
type ReconnectConfig struct {
	// MaxAttempts is the maximum number of reconnection attempts. 0 = infinite.
	MaxAttempts int
	// BaseDelay is the initial delay before first reconnection attempt.
	BaseDelay time.Duration
	// MaxDelay is the maximum delay between reconnection attempts.
	MaxDelay time.Duration
	// JitterFactor adds randomness to delays (0.1 = 10% jitter).
	JitterFactor float64
	// OnMaxAttempts is called when max attempts is reached (if MaxAttempts > 0).
	OnMaxAttempts func()
	// OnStateChange is called when connection state changes.
	OnStateChange func(from, to ConnectionState)
}

// DefaultReconnectConfig returns sensible defaults for reconnection.
func DefaultReconnectConfig() ReconnectConfig {
	return ReconnectConfig{
		MaxAttempts:  defaultMaxReconnectAttempts,
		BaseDelay:    defaultBaseDelay,
		MaxDelay:     defaultMaxDelay,
		JitterFactor: defaultJitterFactor,
	}
}

// AlertNotifier sends alerts to administrators.
type AlertNotifier interface {
	SendAlert(ctx context.Context, severity, title, message string) error
}

// Manager manages WhatsApp client connections with resilient reconnection.
type Manager struct {
	cfg      *config.WhatsAppConfig
	client   *whatsmeow.Client
	store    *sqlstore.Container
	log      zerolog.Logger
	mu       sync.RWMutex
	handlers []EventHandler

	// Bot command handler (optional)
	botHandler *whatsappbot.Bot

	// Admin alerter (optional)
	alerter AlertNotifier

	// Reconnection configuration
	reconnectCfg ReconnectConfig

	// State (atomic for lock-free reads)
	state           atomic.Int32 // ConnectionState
	reconnectCount  atomic.Int32
	lastConnectedAt atomic.Int64 // Unix timestamp

	// Channels
	qrChannel     chan string
	stopChan      chan struct{}
	reconnectChan chan struct{} // Signal to trigger reconnect
}

// EventHandler processes WhatsApp events
type EventHandler interface {
	HandleMessage(msg *IncomingMessage)
	HandleGroupJoined(group *GroupInfo)
}

// IncomingMessage represents a received WhatsApp message
type IncomingMessage struct {
	ID string // External WhatsApp ID

	GroupJID    string
	GroupName   string
	SenderJID   string
	SenderPhone string
	SenderName  string
	Content     string
	Timestamp   time.Time
	IsFromMe    bool

	// Reply context (for messages that are replies to other messages)
	ReplyToID      string // WhatsApp ID of the quoted message
	ReplyToContent string // Text content of the quoted message
	ReplyToSender  string // JID of the sender of the quoted message
}

// GroupInfo represents WhatsApp group information
type GroupInfo struct {
	JID         string
	Name        string
	Description string
}

// NewManager creates a new WhatsApp manager with default reconnection config.
func NewManager(ctx context.Context, cfg *config.WhatsAppConfig, log zerolog.Logger) (*Manager, error) {
	return NewManagerWithConfig(ctx, cfg, DefaultReconnectConfig(), log)
}

// NewManagerWithConfig creates a new WhatsApp manager with custom reconnection config.
func NewManagerWithConfig(ctx context.Context, cfg *config.WhatsAppConfig, reconnectCfg ReconnectConfig, log zerolog.Logger) (*Manager, error) {
	// Ensure session directory exists
	if err := os.MkdirAll(cfg.SessionDir, 0755); err != nil {
		return nil, fmt.Errorf("create session directory: %w", err)
	}

	// Initialize SQLite store for WhatsApp session
	dbPath := filepath.Join(cfg.SessionDir, "whatsapp.db")
	container, err := sqlstore.New(ctx, "sqlite", fmt.Sprintf("file:%s?_pragma=foreign_keys(1)", dbPath), waLog.Noop)
	if err != nil {
		return nil, fmt.Errorf("create session store: %w", err)
	}

	m := &Manager{
		cfg:           cfg,
		store:         container,
		log:           log.With().Str("component", "whatsapp").Logger(),
		reconnectCfg:  reconnectCfg,
		qrChannel:     make(chan string, qrChannelBufferSize),
		stopChan:      make(chan struct{}),
		reconnectChan: make(chan struct{}, 1),
	}
	m.setState(StateDisconnected)
	return m, nil
}

// RegisterHandler adds an event handler
func (m *Manager) RegisterHandler(h EventHandler) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.handlers = append(m.handlers, h)
}

// Connect establishes WhatsApp connection
func (m *Manager) Connect(ctx context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Get or create device
	device, err := m.store.GetFirstDevice(ctx)
	if err != nil {
		return fmt.Errorf("get device: %w", err)
	}

	if device == nil {
		device = m.store.NewDevice()
	}

	// Create client
	m.client = whatsmeow.NewClient(device, waLog.Noop)
	m.client.AddEventHandler(m.handleEvent)

	// Connect
	if m.client.Store.ID == nil {
		// Need to pair - get QR code
		qrChan, _ := m.client.GetQRChannel(ctx)
		if err := m.client.Connect(); err != nil {
			return fmt.Errorf("connect: %w", err)
		}

		// Wait for QR or success
		for evt := range qrChan {
			switch evt.Event {
			case "code":
				m.log.Info().Msg("QR code received, waiting for scan...")
				select {
				case m.qrChannel <- evt.Code:
				default:
				}
			case "success":
				m.log.Info().Msg("Successfully paired!")
				m.onConnected()
				return nil
			case "timeout":
				return fmt.Errorf("QR code timeout")
			}
		}
	} else {
		// Already paired, just connect
		if err := m.client.Connect(); err != nil {
			return fmt.Errorf("connect: %w", err)
		}
		m.onConnected()
		m.log.Info().Msg("Connected to existing session")
	}

	return nil
}

// GetQRChannel returns channel that receives QR codes for pairing
func (m *Manager) GetQRChannel() <-chan string {
	return m.qrChannel
}

// IsConnected returns connection status.
func (m *Manager) IsConnected() bool {
	return m.State() == StateConnected
}

// State returns the current connection state.
func (m *Manager) State() ConnectionState {
	return ConnectionState(m.state.Load())
}

// setState updates the state and triggers callback if configured.
func (m *Manager) setState(newState ConnectionState) {
	oldState := ConnectionState(m.state.Swap(int32(newState)))
	if oldState != newState {
		m.log.Info().
			Str("from", oldState.String()).
			Str("to", newState.String()).
			Msg("Connection state changed")

		metrics.WhatsAppConnectionState.Set(float64(newState))

		if m.reconnectCfg.OnStateChange != nil {
			go m.reconnectCfg.OnStateChange(oldState, newState)
		}
	}
}

// onConnected handles successful connection.
func (m *Manager) onConnected() {
	m.setState(StateConnected)
	m.lastConnectedAt.Store(time.Now().Unix())
	m.reconnectCount.Store(0)
	metrics.WhatsAppReconnectAttempts.Add(0) // Initialize if needed
}

// Disconnect closes the WhatsApp connection.
func (m *Manager) Disconnect() {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.client != nil {
		m.client.Disconnect()
		m.setState(StateDisconnected)
	}

	close(m.stopChan)
}

// GetJoinedGroups returns all groups the client is a member of
func (m *Manager) GetJoinedGroups(ctx context.Context) ([]*GroupInfo, error) {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		return nil, fmt.Errorf("not connected")
	}

	groups, err := client.GetJoinedGroups(ctx)
	if err != nil {
		return nil, fmt.Errorf("get groups: %w", err)
	}

	var result []*GroupInfo
	for _, g := range groups {
		result = append(result, &GroupInfo{
			JID:         g.JID.String(),
			Name:        g.Name,
			Description: g.Topic,
		})
	}

	return result, nil
}

// SyncGroups fetches all joined groups and saves them using the provided save function
func (m *Manager) SyncGroups(ctx context.Context, saveFunc func(jid, name, description string) error) error {
	groups, err := m.GetJoinedGroups(ctx)
	if err != nil {
		return fmt.Errorf("get joined groups: %w", err)
	}

	m.log.Info().Int("count", len(groups)).Msg("Syncing groups to database")

	for _, g := range groups {
		if err := saveFunc(g.JID, g.Name, g.Description); err != nil {
			m.log.Warn().Err(err).Str("jid", g.JID).Msg("Failed to save group")
		}
	}

	return nil
}

// handleEvent processes WhatsApp events
func (m *Manager) handleEvent(evt any) {
	switch v := evt.(type) {
	case *events.Message:
		m.handleMessageEvent(v)
	case *events.Connected:
		m.onConnected()
		m.log.Info().Msg("WhatsApp connected")
	case *events.Disconnected:
		if m.State() != StateReconnecting {
			m.setState(StateReconnecting)
			m.log.Warn().Msg("WhatsApp disconnected, starting reconnection")
			go m.reconnectWithBackoff()
		}
	case *events.HistorySync:
		m.handleHistorySync(v)
	}
}

// handleHistorySync processes history sync events
func (m *Manager) handleHistorySync(v *events.HistorySync) {
	m.log.Info().
		Str("type", fmt.Sprintf("%v", v.Data.SyncType)).
		Int("conversations", len(v.Data.Conversations)).
		Msg("Processing History Sync")

	for _, conv := range v.Data.Conversations {
		for _, waMsg := range conv.Messages {
			if waMsg.Message == nil || waMsg.Message.Key == nil {
				continue
			}

			key := waMsg.Message.Key
			ts := int64(0)
			if waMsg.Message.MessageTimestamp != nil {
				ts = int64(*waMsg.Message.MessageTimestamp)
			}

			pushName := ""
			if waMsg.Message.PushName != nil {
				pushName = *waMsg.Message.PushName
			}

			info := types.MessageInfo{
				ID:        key.GetID(),
				Timestamp: time.Unix(ts, 0),
				PushName:  pushName,
			}
			info.IsFromMe = key.GetFromMe()

			// Parse chat JID
			if key.RemoteJID != nil {
				if chatJID, err := types.ParseJID(*key.RemoteJID); err == nil {
					info.Chat = chatJID
				}
			}

			// Parse sender JID
			if key.Participant != nil {
				if senderJID, err := types.ParseJID(*key.Participant); err == nil {
					info.Sender = senderJID
				}
			} else if !info.IsFromMe {
				info.Sender = info.Chat
			}

			// Mark as group if chat server is g.us
			if info.Chat.Server == "g.us" {
				info.IsGroup = true
			}

			// Only process group messages
			if info.IsGroup {
				msgEvt := &events.Message{
					Info:    info,
					Message: waMsg.Message.Message,
				}
				m.handleMessageEvent(msgEvt)
			}
		}
	}
}

func (m *Manager) handleMessageEvent(evt *events.Message) {
	// Only process group messages
	if !evt.Info.IsGroup {
		return
	}

	// Extract message content using helper
	content := extractTextContent(evt.Message)
	if content == "" {
		return // Skip non-text messages
	}

	// Get group info with timeout
	groupJID := evt.Info.Chat.String()
	groupName := m.fetchGroupName(evt.Info.Chat, groupJID)

	// Get bot handler under lock
	m.mu.RLock()
	botHandler := m.botHandler
	m.mu.RUnlock()

	// Build incoming message
	msg := &IncomingMessage{
		ID:          evt.Info.ID,
		GroupJID:    groupJID,
		GroupName:   groupName,
		SenderJID:   evt.Info.Sender.String(),
		SenderPhone: extractPhoneNumber(evt.Info.Sender),
		SenderName:  evt.Info.PushName,
		Content:     content,
		Timestamp:   evt.Info.Timestamp,
		IsFromMe:    evt.Info.IsFromMe,
	}

	// Extract reply context if this is a reply to another message
	m.extractReplyContext(evt.Message, msg)

	// Check if this is a bot command (before group monitoring check)
	if botHandler != nil && core.IsCommand(content) {
		response := botHandler.HandleIncoming(context.Background(), evt.Info.Sender.String(), groupJID, content, evt.Info.Timestamp)
		if response != nil && response.Text != "" {
			go m.sendBotResponse(evt.Info.Chat, response.Text)
		}
		return // Don't process as regular message
	}

	// Check if group is monitored
	if !m.isGroupMonitored(groupJID) {
		return
	}

	// Notify handlers with panic recovery
	m.notifyHandlers(msg)
}

// extractTextContent extracts text content from a WhatsApp message
func extractTextContent(msg *waE2E.Message) string {
	if msg == nil {
		return ""
	}
	if msg.Conversation != nil {
		return *msg.Conversation
	}
	if msg.ExtendedTextMessage != nil && msg.ExtendedTextMessage.Text != nil {
		return *msg.ExtendedTextMessage.Text
	}
	return ""
}

// fetchGroupName retrieves the group name with timeout, falling back to JID
func (m *Manager) fetchGroupName(chat types.JID, fallback string) string {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		return fallback
	}

	ctx, cancel := context.WithTimeout(context.Background(), groupInfoTimeout)
	defer cancel()

	groupInfo, err := client.GetGroupInfo(ctx, chat)
	if err != nil {
		m.log.Debug().Err(err).Str("jid", fallback).Msg("Failed to get group info")
		return fallback
	}
	return groupInfo.Name
}

// extractReplyContext extracts reply context from an extended text message
func (m *Manager) extractReplyContext(waMsg *waE2E.Message, msg *IncomingMessage) {
	if waMsg.ExtendedTextMessage == nil {
		return
	}

	ctxInfo := waMsg.ExtendedTextMessage.ContextInfo
	if ctxInfo == nil {
		return
	}

	if ctxInfo.StanzaID != nil {
		msg.ReplyToID = *ctxInfo.StanzaID
	}
	if ctxInfo.Participant != nil {
		msg.ReplyToSender = *ctxInfo.Participant
	}
	if ctxInfo.QuotedMessage != nil {
		msg.ReplyToContent = extractTextContent(ctxInfo.QuotedMessage)
	}
}

// notifyHandlers sends the message to all registered handlers with panic recovery
func (m *Manager) notifyHandlers(msg *IncomingMessage) {
	m.mu.RLock()
	handlers := m.handlers
	m.mu.RUnlock()

	for _, h := range handlers {
		go func(handler EventHandler) {
			defer func() {
				if r := recover(); r != nil {
					m.log.Error().
						Interface("panic", r).
						Str("message_id", msg.ID).
						Str("group", msg.GroupName).
						Msg("Handler panic recovered - message processing failed")
				}
			}()
			handler.HandleMessage(msg)
		}(h)
	}
}

func (m *Manager) isGroupMonitored(jid string) bool {
	// If no specific groups configured, monitor all
	if len(m.cfg.MonitoredGroups) == 0 {
		return true
	}

	return slices.Contains(m.cfg.MonitoredGroups, jid)
}

// reconnectWithBackoff attempts to reconnect with exponential backoff and jitter.
func (m *Manager) reconnectWithBackoff() {
	cfg := m.reconnectCfg
	baseDelay := cfg.BaseDelay
	if baseDelay == 0 {
		baseDelay = defaultBaseDelay
	}
	maxDelay := cfg.MaxDelay
	if maxDelay == 0 {
		maxDelay = defaultMaxDelay
	}

	attempt := 0
	for {
		// Check if already connected
		if m.State() == StateConnected {
			return
		}

		// Check max attempts (0 = infinite)
		if cfg.MaxAttempts > 0 && attempt >= cfg.MaxAttempts {
			m.onMaxAttemptsReached(attempt)
			return
		}

		attempt++
		m.reconnectCount.Store(int32(attempt))
		metrics.WhatsAppReconnectAttempts.Inc()

		// Calculate delay with exponential backoff and jitter
		delay := m.calculateBackoffDelay(attempt, baseDelay, maxDelay, cfg.JitterFactor)

		m.log.Info().
			Int("attempt", attempt).
			Int("max_attempts", cfg.MaxAttempts).
			Dur("delay", delay).
			Msg("Attempting reconnection")

		// Wait with cancellation support
		select {
		case <-time.After(delay):
		case <-m.stopChan:
			m.log.Info().Msg("Reconnection cancelled")
			return
		case <-m.reconnectChan:
			m.log.Info().Msg("Manual reconnect triggered")
		}

		ctx, cancel := context.WithTimeout(context.Background(), defaultConnectTimeout)
		err := m.Connect(ctx)
		cancel()

		if err == nil {
			m.log.Info().Int("attempt", attempt).Msg("Reconnection successful")
			return
		}

		m.log.Error().Err(err).Int("attempt", attempt).Msg("Reconnection failed")
	}
}

// calculateBackoffDelay calculates delay with exponential backoff and jitter.
func (m *Manager) calculateBackoffDelay(attempt int, baseDelay, maxDelay time.Duration, jitterFactor float64) time.Duration {
	// Exponential backoff: baseDelay * 2^(attempt-1)
	delay := min(baseDelay*time.Duration(1<<uint(attempt-1)), maxDelay)

	// Add jitter: delay * (1 ± jitterFactor)
	if jitterFactor > 0 {
		jitter := float64(delay) * jitterFactor * (2*rand.Float64() - 1)
		delay = time.Duration(float64(delay) + jitter)
	}

	return delay
}

// onMaxAttemptsReached handles when max reconnection attempts are exhausted.
func (m *Manager) onMaxAttemptsReached(attempts int) {
	m.setState(StateFailed)
	metrics.WhatsAppReconnectFailures.Inc()

	m.log.Error().
		Int("attempts", attempts).
		Msg("Max reconnection attempts reached")

	// Call user callback
	if m.reconnectCfg.OnMaxAttempts != nil {
		go m.reconnectCfg.OnMaxAttempts()
	}

	// Send admin alert
	if m.alerter != nil {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		err := m.alerter.SendAlert(ctx, "critical", "WhatsApp Disconnected",
			fmt.Sprintf("WhatsApp connection failed after %d attempts. Manual intervention required.", attempts))
		if err != nil {
			m.log.Error().Err(err).Msg("Failed to send admin alert")
		}
	}
}

// ForceReconnect triggers an immediate reconnection attempt.
func (m *Manager) ForceReconnect() {
	select {
	case m.reconnectChan <- struct{}{}:
		m.log.Info().Msg("Force reconnect signal sent")
	default:
		m.log.Warn().Msg("Reconnect already in progress")
	}
}

// SetAlerter sets the alert notifier for admin notifications.
func (m *Manager) SetAlerter(alerter AlertNotifier) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.alerter = alerter
}

// GetConnectionStatus returns detailed connection status.
func (m *Manager) GetConnectionStatus() ConnectionStatus {
	return ConnectionStatus{
		State:           m.State(),
		ReconnectCount:  int(m.reconnectCount.Load()),
		LastConnectedAt: time.Unix(m.lastConnectedAt.Load(), 0),
		UptimeSeconds:   m.getUptimeSeconds(),
	}
}

// ConnectionStatus represents detailed connection status.
type ConnectionStatus struct {
	State           ConnectionState `json:"state"`
	ReconnectCount  int             `json:"reconnect_count"`
	LastConnectedAt time.Time       `json:"last_connected_at"`
	UptimeSeconds   int64           `json:"uptime_seconds"`
}

func (m *Manager) getUptimeSeconds() int64 {
	if m.State() != StateConnected {
		return 0
	}
	lastConn := m.lastConnectedAt.Load()
	if lastConn == 0 {
		return 0
	}
	return time.Now().Unix() - lastConn
}

func extractPhoneNumber(jid types.JID) string {
	return jid.User
}

// SendMessage sends a text message to the specified JID
func (m *Manager) SendMessage(ctx context.Context, jidStr, content string) error {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		return fmt.Errorf("not connected")
	}

	jid, err := types.ParseJID(jidStr)
	if err != nil {
		return fmt.Errorf("invalid JID: %w", err)
	}

	msg := &waE2E.Message{
		ExtendedTextMessage: &waE2E.ExtendedTextMessage{
			Text: proto.String(content),
		},
	}

	_, err = client.SendMessage(ctx, jid, msg)
	return err
}

// GenerateMessageID creates a unique message ID
func GenerateMessageID() string {
	return uuid.New().String()
}

// SetBotHandler sets the bot command handler
func (m *Manager) SetBotHandler(handler *whatsappbot.Bot) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.botHandler = handler
}

// sendBotResponse sends a response message back to the chat
func (m *Manager) sendBotResponse(chat types.JID, response string) {
	m.mu.RLock()
	client := m.client
	m.mu.RUnlock()

	if client == nil {
		m.log.Warn().Msg("Cannot send bot response - not connected")
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), botResponseTimeout)
	defer cancel()

	msg := &waE2E.Message{
		ExtendedTextMessage: &waE2E.ExtendedTextMessage{
			Text: proto.String(response),
		},
	}

	_, err := client.SendMessage(ctx, chat, msg)
	if err != nil {
		m.log.Error().Err(err).Str("chat", chat.String()).Msg("Failed to send bot response")
	} else {
		m.log.Debug().Str("chat", chat.String()).Msg("Bot response sent")
	}
}
