// Package whatsapp provides the WhatsApp adapter using whatsmeow.
package whatsapp

import (
	"context"
	"fmt"
	"time"

	_ "github.com/mattn/go-sqlite3"
	"github.com/rs/zerolog"
	"go.mau.fi/whatsmeow"
	waProto "go.mau.fi/whatsmeow/proto/waE2E"
	"go.mau.fi/whatsmeow/store/sqlstore"
	"go.mau.fi/whatsmeow/types"
	"go.mau.fi/whatsmeow/types/events"
	waLog "go.mau.fi/whatsmeow/util/log"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
)

// Client implements ports.MessageSource for WhatsApp.
type Client struct {
	wa        *whatsmeow.Client
	store     *sqlstore.Container
	qrHandler ports.QRHandler
	logger    zerolog.Logger
	messages  chan domain.Message
	cfg       ClientConfig
}

// ClientConfig holds configuration for the WhatsApp client.
type ClientConfig struct {
	StorePath    string
	QRMaxRetries int
	QRTimeout    time.Duration
}

// NewClient creates a new WhatsApp client.
func NewClient(ctx context.Context, cfg ClientConfig, qrHandler ports.QRHandler, logger zerolog.Logger) (*Client, error) {
	dbLog := waLog.Stdout("Database", "DEBUG", true)
	container, err := sqlstore.New(ctx, "sqlite3", fmt.Sprintf("file:%s?_foreign_keys=on&_busy_timeout=5000&_journal_mode=WAL", cfg.StorePath), dbLog)
	if err != nil {
		return nil, fmt.Errorf("failed to create store: %w", err)
	}

	return &Client{
		store:     container,
		qrHandler: qrHandler,
		logger:    logger.With().Str("component", "whatsapp").Logger(),
		messages:  make(chan domain.Message, 1000),
		cfg:       cfg,
	}, nil
}

// Connect establishes connection to WhatsApp.
func (c *Client) Connect(ctx context.Context) error {
	deviceStore, err := c.store.GetFirstDevice(ctx)
	if err != nil {
		return fmt.Errorf("failed to get device: %w", err)
	}

	clientLog := waLog.Stdout("Client", "INFO", true)
	c.wa = whatsmeow.NewClient(deviceStore, clientLog)
	c.wa.AddEventHandler(c.handleEvent)

	if c.wa.Store.ID == nil {
		return c.pairWithQR(ctx)
	}

	c.qrHandler.SetPaired()
	if err := c.wa.Connect(); err != nil {
		return fmt.Errorf("failed to connect: %w", err)
	}

	c.logger.Info().Msg("WhatsApp connected (already paired)")
	return nil
}

func (c *Client) pairWithQR(ctx context.Context) error {
	qrAttempt := 0
	maxRetries := c.cfg.QRMaxRetries

	for {
		qrAttempt++
		c.logger.Info().
			Int("attempt", qrAttempt).
			Int("max_retries", maxRetries).
			Msg("📱 Starting QR code pairing...")

		qrChan, _ := c.wa.GetQRChannel(ctx)
		if err := c.wa.Connect(); err != nil {
			c.qrHandler.HandleError(err)
			return fmt.Errorf("failed to connect: %w", err)
		}

		paired := false
		for evt := range qrChan {
			switch evt.Event {
			case "code":
				c.qrHandler.HandleQRCode(evt.Code)
			case "success":
				c.qrHandler.HandleEvent("success")
				c.logger.Info().Msg("✅ WhatsApp paired successfully")
				paired = true
			case "timeout":
				c.qrHandler.HandleEvent("timeout")
				c.logger.Warn().Int("attempt", qrAttempt).Msg("⏰ QR code expired")
			default:
				c.qrHandler.HandleEvent(evt.Event)
			}
		}

		if paired {
			return nil
		}

		if maxRetries > 0 && qrAttempt >= maxRetries {
			return fmt.Errorf("QR code pairing failed after %d attempts", qrAttempt)
		}

		c.wa.Disconnect()
		c.logger.Info().Int("attempt", qrAttempt).Msg("🔄 Retrying QR code pairing in 5 seconds...")

		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(5 * time.Second):
		}
	}
}

// Disconnect closes the WhatsApp connection.
func (c *Client) Disconnect() {
	if c.wa != nil {
		c.wa.Disconnect()
	}
	close(c.messages)
}

// Messages returns a channel of incoming messages.
func (c *Client) Messages() <-chan domain.Message {
	return c.messages
}

// IsConnected returns true if connected to WhatsApp.
func (c *Client) IsConnected() bool {
	return c.wa != nil && c.wa.IsConnected()
}

func (c *Client) handleEvent(evt any) {
	switch v := evt.(type) {
	case *events.Message:
		c.handleMessage(v)
	case *events.Connected:
		c.logger.Info().Msg("WhatsApp connected")
	case *events.Disconnected:
		c.logger.Warn().Msg("WhatsApp disconnected")
	case *events.HistorySync:
		c.handleHistorySync(v)
	}
}

// extractContent extracts text content from a WhatsApp message.
// Handles both simple conversation and extended text messages.
func extractContent(msg *waProto.Message) string {
	if msg == nil {
		return ""
	}
	return domain.ExtractContent(
		msg.GetConversation(),
		getExtendedText(msg.GetExtendedTextMessage()),
	)
}

func getExtendedText(ext *waProto.ExtendedTextMessage) string {
	if ext == nil {
		return ""
	}
	return ext.GetText()
}

func (c *Client) handleMessage(evt *events.Message) {
	content := extractContent(evt.Message)

	// Comprehensive logging for sender JID diagnosis
	c.logger.Debug().
		Str("msg_id", evt.Info.ID).
		Str("sender_full_jid", evt.Info.Sender.String()).
		Str("sender_user", evt.Info.Sender.User).
		Str("sender_server", evt.Info.Sender.Server).
		Str("sender_device", fmt.Sprintf("%d", evt.Info.Sender.Device)).
		Bool("sender_is_empty", evt.Info.Sender.IsEmpty()).
		Str("chat_jid", evt.Info.Chat.String()).
		Str("chat_user", evt.Info.Chat.User).
		Str("push_name", evt.Info.PushName).
		Bool("is_group", evt.Info.IsGroup).
		Bool("is_from_me", evt.Info.IsFromMe).
		Msg("📱 Message sender info")

	// Log if sender phone looks unusual (not a typical phone number pattern)
	senderPhone := evt.Info.Sender.User
	senderJID := evt.Info.Sender.String()

	if evt.Info.Sender.Server == "lid" {
		// Try to resolve LID to Phone Number from local store
		pnJID, err := c.wa.Store.LIDs.GetPNForLID(context.Background(), evt.Info.Sender)
		if err == nil && !pnJID.IsEmpty() {
			senderPhone = pnJID.User
			senderJID = pnJID.String()
			c.logger.Debug().
				Str("lid", evt.Info.Sender.User).
				Str("pn", senderPhone).
				Str("resolved_jid", senderJID).
				Msg("✅ Resolved LID to Phone Number")
		} else {
			c.logger.Debug().
				Str("lid", evt.Info.Sender.User).
				Msg("ℹ️ Could not resolve LID to Phone Number (using LID as fallback)")
		}
	} else if len(senderPhone) > 15 || len(senderPhone) < 10 {
		c.logger.Warn().
			Str("sender_user", senderPhone).
			Int("length", len(senderPhone)).
			Str("full_jid", evt.Info.Sender.String()).
			Msg("⚠️ Unusual sender phone format detected")
	}

	msg := domain.Message{
		ID:          domain.MessageID(evt.Info.ID),
		ExternalID:  domain.MessageID(evt.Info.ID),
		GroupJID:    domain.JID(evt.Info.Chat.String()),
		GroupName:   evt.Info.Chat.String(),
		SenderJID:   domain.JID(senderJID),
		SenderPhone: domain.Phone(senderPhone),
		SenderName:  evt.Info.PushName,
		Content:     content,
		Timestamp:   domain.UnixTimestamp(evt.Info.Timestamp.Unix()),
		IsFromMe:    evt.Info.IsFromMe,
		IsGroup:     evt.Info.IsGroup,
	}

	select {
	case c.messages <- msg:
	default:
		c.logger.Warn().Msg("Message channel full, dropping message")
	}
}

func (c *Client) handleHistorySync(v *events.HistorySync) {
	c.logger.Info().Int("conversations", len(v.Data.Conversations)).Msg("Processing History Sync")

	for _, conv := range v.Data.Conversations {
		for _, waMsg := range conv.Messages {
			if waMsg.Message == nil || waMsg.Message.Key == nil {
				continue
			}

			key := waMsg.Message.Key
			msgID := domain.MessageID(key.GetID())

			ts := domain.UnixTimestamp(0)
			if waMsg.Message.MessageTimestamp != nil {
				ts = domain.UnixTimestamp(*waMsg.Message.MessageTimestamp)
			}

			if key.RemoteJID == nil {
				continue
			}

			chatJID, err := types.ParseJID(*key.RemoteJID)
			if err != nil || chatJID.Server != "g.us" {
				continue
			}

			content := extractContent(waMsg.Message.Message)
			if content == "" {
				continue
			}

			var senderJID domain.JID
			var senderPhone domain.Phone
			if key.Participant != nil {
				if parsed, err := types.ParseJID(*key.Participant); err == nil {
					user := parsed.User
					sJID := parsed.String()
					if parsed.Server == "lid" {
						pnJID, err := c.wa.Store.LIDs.GetPNForLID(context.Background(), parsed)
						if err == nil && !pnJID.IsEmpty() {
							user = pnJID.User
							sJID = pnJID.String()
							c.logger.Debug().
								Str("msg_id", msgID.String()).
								Str("lid", parsed.User).
								Str("pn", user).
								Str("resolved_jid", sJID).
								Msg("✅ Resolved HistorySync LID to Phone Number")
						}
					}
					senderPhone = domain.Phone(user)
					senderJID = domain.JID(sJID)
					// Log history sync sender
					c.logger.Debug().Str("msg_id", msgID.String()).Str("sender_phone", user).Msg("📚 History sync sender processed")
				}
			}

			pushName := ""
			if waMsg.Message.PushName != nil {
				pushName = *waMsg.Message.PushName
			}

			msg := domain.Message{
				ID:          msgID,
				ExternalID:  msgID,
				GroupJID:    domain.JID(chatJID.String()),
				GroupName:   chatJID.String(),
				SenderJID:   senderJID,
				SenderPhone: senderPhone,
				SenderName:  pushName,
				Content:     content,
				Timestamp:   ts,
				IsFromMe:    key.GetFromMe(),
				IsGroup:     true,
			}

			select {
			case c.messages <- msg:
			default:
			}
		}
	}
}

// GetWhatsmeowClient returns the underlying whatsmeow client.
func (c *Client) GetWhatsmeowClient() *whatsmeow.Client {
	return c.wa
}

// GetJoinedGroups returns all WhatsApp groups the user has joined.
func (c *Client) GetJoinedGroups(ctx context.Context) ([]domain.GroupInfo, error) {
	if c.wa == nil || !c.wa.IsConnected() {
		return nil, fmt.Errorf("not connected to WhatsApp")
	}

	groups, err := c.wa.GetJoinedGroups(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get joined groups: %w", err)
	}

	result := make([]domain.GroupInfo, 0, len(groups))
	for _, g := range groups {
		result = append(result, domain.GroupInfo{
			JID:         domain.JID(g.JID.String()),
			Name:        g.Name,
			Description: g.Topic,
			MemberCount: int32(len(g.Participants)),
		})
	}

	c.logger.Info().Int("count", len(result)).Msg("📋 Fetched joined groups from WhatsApp")
	return result, nil
}

// SendMessage sends a plain text message to a JID.
func (c *Client) SendMessage(ctx context.Context, to domain.JID, content string) error {
	target, err := types.ParseJID(to.String())
	if err != nil {
		return fmt.Errorf("invalid destination JID: %w", err)
	}

	_, err = c.wa.SendMessage(ctx, target, &waProto.Message{
		Conversation: &content,
	})
	if err != nil {
		return fmt.Errorf("failed to send message: %w", err)
	}

	c.logger.Debug().Str("to", to.String()).Msg("📤 Sent text message")
	return nil
}

// SendContactCard sends contact information (VCard) to a target JID.
func (c *Client) SendContactCard(ctx context.Context, to domain.JID, contactJID domain.JID, name string, phone string) error {
	target, err := types.ParseJID(to.String())
	if err != nil {
		return fmt.Errorf("invalid destination JID: %w", err)
	}

	cJID, err := types.ParseJID(contactJID.String())
	if err != nil {
		return fmt.Errorf("invalid contact JID: %w", err)
	}

	waid := phone

	// If it's a LID, try to resolve to PN for a clickable card
	if cJID.Server == "lid" {
		pnJID, err := c.wa.Store.LIDs.GetPNForLID(ctx, cJID)
		if err == nil && !pnJID.IsEmpty() {
			waid = pnJID.User
			c.logger.Debug().
				Str("lid", cJID.User).
				Str("resolved_pn", waid).
				Msg("✅ Resolved LID to PN for VCard")
		} else {
			c.logger.Debug().
				Str("lid", cJID.User).
				Msg("ℹ️ Could not resolve LID to PN for VCard (using provided phone/LID)")
		}
	}

	// Clean waid (must be digits only for waid parameter)
	waid = cleanPhone(waid)

	// Basic vCard 3.0 format
	// FN: Full Name
	// TEL: Phone number with + prefix
	// waid: WhatsApp ID (phone number without +)
	vcard := fmt.Sprintf("BEGIN:VCARD\nVERSION:3.0\nFN:%s\nTEL;type=CELL;type=VOICE;waid=%s:+%s\nEND:VCARD",
		name, waid, waid)

	_, err = c.wa.SendMessage(ctx, target, &waProto.Message{
		ContactMessage: &waProto.ContactMessage{
			DisplayName: &name,
			Vcard:       &vcard,
		},
	})
	if err != nil {
		return fmt.Errorf("failed to send contact card: %w", err)
	}

	c.logger.Debug().
		Str("to", to.String()).
		Str("contact", name).
		Str("waid", waid).
		Msg("👤 Sent contact card")
	return nil
}

func cleanPhone(p string) string {
	res := ""
	for _, c := range p {
		if c >= '0' && c <= '9' {
			res += string(c)
		}
	}
	return res
}

var _ ports.MessageSource = (*Client)(nil)
var _ ports.MessageProvider = (*Client)(nil)
