package whatsapp

import (
	"context"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Listener listens for WhatsApp messages and queues them for processing
type Listener struct {
	log                    zerolog.Logger
	rawMsgRepo             repository.RawMessageRepository
	groupRepo              repository.GroupRepository
	msgChannel             chan *entity.RawMessage
	skipOwnMessagesChecker func() bool // Check if should skip own messages
}

// NewListener creates a new message listener
func NewListener(
	log zerolog.Logger,
	rawMsgRepo repository.RawMessageRepository,
	groupRepo repository.GroupRepository,
) *Listener {
	return &Listener{
		log:                    log.With().Str("component", "listener").Logger(),
		rawMsgRepo:             rawMsgRepo,
		groupRepo:              groupRepo,
		msgChannel:             make(chan *entity.RawMessage, 1000), // Buffer for burst handling
		skipOwnMessagesChecker: func() bool { return true },         // Default: skip own messages
	}
}

// SetSkipOwnMessagesChecker sets the function to check if own messages should be skipped
func (l *Listener) SetSkipOwnMessagesChecker(fn func() bool) {
	l.skipOwnMessagesChecker = fn
}

// MessageChannel returns channel that receives raw messages
func (l *Listener) MessageChannel() <-chan *entity.RawMessage {
	return l.msgChannel
}

// HandleMessage implements EventHandler interface
func (l *Listener) HandleMessage(msg *IncomingMessage) {
	// Log every incoming message
	l.log.Info().
		Str("step", "1_RECEIVED").
		Str("group", msg.GroupName).
		Str("sender", msg.SenderName).
		Str("content_preview", truncate(msg.Content, 100)).
		Msg("📥 Message received from WhatsApp")

	// Skip own messages if configured
	if msg.IsFromMe && l.skipOwnMessagesChecker() {
		l.log.Debug().
			Str("step", "1_SKIPPED").
			Msg("⏭️ Skipping own message (config: skip_own_messages=true)")
		return
	}

	if msg.IsFromMe {
		l.log.Info().
			Str("step", "1_OWN_MESSAGE").
			Str("content", msg.Content).
			Msg("📨 Processing OWN message (config: skip_own_messages=false)")
	}

	// ----------------------------------------------------
	// Semantic Deduplication (Spam/Repetition Filter)
	// ----------------------------------------------------
	// Check if this user sent the exact same message very recently (e.g. < 10s)
	// This prevents duplicate requests if they double-tap send or history sync re-sends logic.
	ctxShort, cancelShort := context.WithTimeout(context.Background(), 2*time.Second)
	lastMsg, err := l.rawMsgRepo.GetLastMessageBySender(ctxShort, msg.GroupJID, msg.SenderJID)
	cancelShort()

	if err == nil && lastMsg != nil {
		// Time threshold: 10 seconds
		timeDiff := msg.Timestamp.Sub(lastMsg.Timestamp)
		if timeDiff < 0 {
			timeDiff = -timeDiff // Handle out-of-order clocks slightly
		}

		if timeDiff < 10*time.Second && lastMsg.Content == msg.Content {
			l.log.Warn().
				Str("step", "1_DUPLICATE_IGNORED").
				Str("sender", msg.SenderName).
				Str("content", truncate(msg.Content, 50)).
				Dur("time_diff", timeDiff).
				Msg("🛑 Ignoring duplicate message from same user < 10s")
			return
		}
	}

	// Check if this group is monitored
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	monitored, err := l.isGroupMonitored(ctx, msg.GroupJID)
	if err != nil {
		l.log.Error().Err(err).Str("jid", msg.GroupJID).Msg("Failed to check if group is monitored")
		return
	}
	if !monitored {
		l.log.Warn().
			Str("step", "2_NOT_MONITORED").
			Str("group", msg.GroupName).
			Str("jid", msg.GroupJID).
			Msg("⚠️ Group NOT monitored - message ignored")
		return
	}

	l.log.Info().
		Str("step", "2_MONITORED").
		Str("group", msg.GroupName).
		Msg("✅ Group is monitored - processing")

	// Create raw message
	rawMsg := &entity.RawMessage{
		ID:             uuid.New().String(),
		ExternalID:     msg.ID, // WhatsApp Message ID
		GroupJID:       msg.GroupJID,
		GroupName:      msg.GroupName,
		SenderJID:      msg.SenderJID,
		SenderPhone:    msg.SenderPhone,
		SenderName:     msg.SenderName,
		Content:        msg.Content,
		Timestamp:      msg.Timestamp,
		ReplyToID:      msg.ReplyToID,
		ReplyToContent: msg.ReplyToContent,
		ReplyToSender:  msg.ReplyToSender,
	}

	// Save to database
	if err := l.rawMsgRepo.Save(ctx, rawMsg); err != nil {
		l.log.Error().Err(err).Str("msg_id", rawMsg.ID).Msg("Failed to save raw message")
		return
	}

	l.log.Info().
		Str("step", "3_SAVED").
		Str("msg_id", rawMsg.ID).
		Str("group", rawMsg.GroupName).
		Msg("💾 Message saved to database")

	// Update group stats
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = l.groupRepo.UpdateLastMessage(ctx, msg.GroupJID)
		_ = l.groupRepo.IncrementMessageCount(ctx, msg.GroupJID)
	}()

	// Queue for processing
	select {
	case l.msgChannel <- rawMsg:
		l.log.Info().
			Str("step", "4_QUEUED").
			Str("msg_id", rawMsg.ID).
			Str("content", rawMsg.Content).
			Msg("📤 Message queued for AI processing")
	default:
		l.log.Error().Str("msg_id", rawMsg.ID).Msg("❌ Message queue full, dropping message")
	}
}

// isGroupMonitored checks if a group is set as monitored in the database
func (l *Listener) isGroupMonitored(ctx context.Context, jid string) (bool, error) {
	groups, err := l.groupRepo.GetMonitored(ctx)
	if err != nil {
		return false, err
	}

	// If no groups are in database yet, don't monitor anything (user must sync first)
	if len(groups) == 0 {
		return false, nil
	}

	for _, g := range groups {
		if g.JID == jid {
			return true, nil
		}
	}
	return false, nil
}

// HandleGroupJoined implements EventHandler interface
func (l *Listener) HandleGroupJoined(group *GroupInfo) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	g := &entity.Group{
		JID:         group.JID,
		Name:        group.Name,
		Description: group.Description,
		Monitored:   true,
		AddedAt:     time.Now(),
	}

	if err := l.groupRepo.Save(ctx, g); err != nil {
		l.log.Error().Err(err).Str("jid", group.JID).Msg("Failed to save group")
		return
	}

	l.log.Info().Str("jid", group.JID).Str("name", group.Name).Msg("New group discovered")
}

// SyncGroups synchronizes known groups with database (defaults to not monitored)
func (l *Listener) SyncGroups(ctx context.Context, groups []*GroupInfo) error {
	for _, g := range groups {
		group := &entity.Group{
			JID:         g.JID,
			Name:        g.Name,
			Description: g.Description,
			Monitored:   false, // Default to false - user must explicitly enable
			AddedAt:     time.Now(),
		}
		if err := l.groupRepo.Save(ctx, group); err != nil {
			return err
		}
	}
	return nil
}

// truncate limits string length for log output
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}
