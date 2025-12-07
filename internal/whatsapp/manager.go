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

	"pharmabroker/internal/config"
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
		qrChannel: make(chan string, 1),
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
func (m *Manager) handleEvent(evt interface{}) {
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
		go m.reconnect()
	case *events.HistorySync:
		m.log.Info().
			Str("type", fmt.Sprintf("%v", v.Data.SyncType)).
			Int("conversations", len(v.Data.Conversations)).
			Msg("📥 Processing History Sync...")

		for _, conv := range v.Data.Conversations {
			for _, waMsg := range conv.Messages {
				if waMsg.Message == nil || waMsg.Message.Key == nil {
					continue
				}

				// Extract basics
				key := waMsg.Message.Key
				ts := int64(0)
				if waMsg.Message.MessageTimestamp != nil {
					ts = int64(*waMsg.Message.MessageTimestamp)
				}

				pushName := ""
				if waMsg.Message.PushName != nil {
					pushName = *waMsg.Message.PushName
				}

				// Construct MessageInfo
				info := types.MessageInfo{
					ID:        key.GetID(),
					Timestamp: time.Unix(ts, 0),
					PushName:  pushName,
				}
				info.IsFromMe = key.GetFromMe()

				// Parse JIDs
				if key.RemoteJID != nil {
					chatJID, err := types.ParseJID(*key.RemoteJID)
					if err == nil {
						info.Chat = chatJID
					}
				}
				if key.Participant != nil {
					senderJID, err := types.ParseJID(*key.Participant)
					if err == nil {
						info.Sender = senderJID
					}
				} else {
					// In DM, sender is RemoteJid if not from me
					if !info.IsFromMe {
						info.Sender = info.Chat
					}
				}

				// Handle Group logic
				if info.Chat.Server == "g.us" {
					info.IsGroup = true
					// For groups, sender is in Participant. If missing and not from me, might be issue.
					// HistorySync usually has Participant for groups.
				}

				msgEvt := &events.Message{
					Info:    info,
					Message: waMsg.Message.Message,
					// Raw: waMsg.Message, // Removed as it caused error and unused
				}

				// Only process if it looks like a valid group message
				if info.IsGroup {
					m.handleMessageEvent(msgEvt)
				}
			}
		}
	}
}

func (m *Manager) handleMessageEvent(evt *events.Message) {
	// Only process group messages
	if !evt.Info.IsGroup {
		return
	}

	// Extract message content
	var content string
	if evt.Message.Conversation != nil {
		content = *evt.Message.Conversation
	} else if evt.Message.ExtendedTextMessage != nil && evt.Message.ExtendedTextMessage.Text != nil {
		content = *evt.Message.ExtendedTextMessage.Text
	} else {
		// Skip non-text messages
		return
	}

	// Get group info
	groupJID := evt.Info.Chat.String()
	groupName := groupJID // Will be updated if we can get group info

	// Try to get actual group name
	if groupInfo, err := m.client.GetGroupInfo(context.Background(), evt.Info.Chat); err == nil {
		groupName = groupInfo.Name
	}

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

	// Check if this is a bot command (before group monitoring check)
	if m.botHandler != nil && IsCommand(content) {
		response := m.botHandler.HandleCommand(context.Background(), msg)
		if response != "" {
			// Send response back to the sender
			go m.sendBotResponse(evt.Info.Chat, response)
		}
		return // Don't process as regular message
	}

	// Check if group is monitored
	if !m.isGroupMonitored(groupJID) {
		return
	}

	// Notify handlers with panic recovery
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

func (m *Manager) reconnect() {
	m.mu.Lock()
	if m.connected {
		m.mu.Unlock()
		return
	}
	m.mu.Unlock()

	m.log.Info().Dur("delay", m.cfg.ReconnectDelay).Msg("Attempting reconnection...")
	time.Sleep(m.cfg.ReconnectDelay)

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if err := m.Connect(ctx); err != nil {
		m.log.Error().Err(err).Msg("Reconnection failed")
		go m.reconnect() // Try again
	}
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

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
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
