package whatsapp

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"sync"
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

	"pharmabroker/pkg/config"
)

// Constants for timeouts and limits
const (
	defaultConnectTimeout = 30 * time.Second
	groupInfoTimeout      = 5 * time.Second
	botResponseTimeout    = 30 * time.Second
	maxReconnectAttempts  = 5
	maxReconnectDelay     = 5 * time.Minute
	qrChannelBufferSize   = 1
)

// Manager manages WhatsApp client connections
type Manager struct {
	cfg      *config.WhatsAppConfig
	client   *whatsmeow.Client
	store    *sqlstore.Container
	log      zerolog.Logger
	mu       sync.RWMutex
	handlers []EventHandler

	// Bot command handler (optional)
	botHandler *BotCommandHandler

	// State
	connected bool
	qrChannel chan string
	stopChan  chan struct{}
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

// NewManager creates a new WhatsApp manager
func NewManager(ctx context.Context, cfg *config.WhatsAppConfig, log zerolog.Logger) (*Manager, error) {
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

	return &Manager{
		cfg:       cfg,
		store:     container,
		log:       log.With().Str("component", "whatsapp").Logger(),
		qrChannel: make(chan string, qrChannelBufferSize),
		stopChan:  make(chan struct{}),
	}, nil
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
				m.connected = true
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
		m.connected = true
		m.log.Info().Msg("Connected to existing session")
	}

	return nil
}

// GetQRChannel returns channel that receives QR codes for pairing
func (m *Manager) GetQRChannel() <-chan string {
	return m.qrChannel
}

// IsConnected returns connection status
func (m *Manager) IsConnected() bool {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.connected
}

// Disconnect closes the WhatsApp connection
func (m *Manager) Disconnect() {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.client != nil {
		m.client.Disconnect()
		m.connected = false
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
		m.mu.Lock()
		m.connected = true
		m.mu.Unlock()
		m.log.Info().Msg("WhatsApp connected")
	case *events.Disconnected:
		m.mu.Lock()
		m.connected = false
		m.mu.Unlock()
		m.log.Warn().Msg("WhatsApp disconnected")
		go m.reconnectWithBackoff()
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
	if botHandler != nil && IsCommand(content) {
		response := botHandler.HandleCommand(context.Background(), msg)
		if response != "" {
			go m.sendBotResponse(evt.Info.Chat, response)
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

// reconnectWithBackoff attempts to reconnect with exponential backoff
func (m *Manager) reconnectWithBackoff() {
	baseDelay := m.cfg.ReconnectDelay
	if baseDelay == 0 {
		baseDelay = 5 * time.Second
	}

	for attempt := range maxReconnectAttempts {
		m.mu.RLock()
		isConnected := m.connected
		m.mu.RUnlock()

		if isConnected {
			return // Already reconnected
		}

		// Exponential backoff: delay * 2^attempt, capped at maxReconnectDelay
		delay := min(baseDelay*time.Duration(1<<attempt), maxReconnectDelay)

		m.log.Info().
			Int("attempt", attempt+1).
			Int("max_attempts", maxReconnectAttempts).
			Dur("delay", delay).
			Msg("Attempting reconnection")

		time.Sleep(delay)

		ctx, cancel := context.WithTimeout(context.Background(), defaultConnectTimeout)
		err := m.Connect(ctx)
		cancel()

		if err == nil {
			m.log.Info().Int("attempt", attempt+1).Msg("Reconnection successful")
			return
		}

		m.log.Error().Err(err).Int("attempt", attempt+1).Msg("Reconnection failed")
	}

	m.log.Error().
		Int("max_attempts", maxReconnectAttempts).
		Msg("Max reconnection attempts reached, giving up")
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
func (m *Manager) SetBotHandler(handler *BotCommandHandler) {
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
