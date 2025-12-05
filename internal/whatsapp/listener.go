package whatsapp

import (
	"context"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
)

// Listener listens for WhatsApp messages and queues them for processing
type Listener struct {
	log        zerolog.Logger
	rawMsgRepo domain.RawMessageRepository
	groupRepo  domain.GroupRepository
	msgChannel chan *domain.RawMessage
	mu         sync.RWMutex
	running    bool
}

// NewListener creates a new message listener
func NewListener(
	log zerolog.Logger,
	rawMsgRepo domain.RawMessageRepository,
	groupRepo domain.GroupRepository,
) *Listener {
	return &Listener{
		log:        log.With().Str("component", "listener").Logger(),
		rawMsgRepo: rawMsgRepo,
		groupRepo:  groupRepo,
		msgChannel: make(chan *domain.RawMessage, 1000), // Buffer for burst handling
	}
}

// MessageChannel returns channel that receives raw messages
func (l *Listener) MessageChannel() <-chan *domain.RawMessage {
	return l.msgChannel
}

// HandleMessage implements EventHandler interface
func (l *Listener) HandleMessage(msg *IncomingMessage) {
	// Skip own messages
	if msg.IsFromMe {
		return
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
		l.log.Debug().Str("group", msg.GroupName).Msg("Skipping message from non-monitored group")
		return
	}

	// Create raw message
	rawMsg := &domain.RawMessage{
		ID:          uuid.New().String(),
		GroupJID:    msg.GroupJID,
		GroupName:   msg.GroupName,
		SenderJID:   msg.SenderJID,
		SenderPhone: msg.SenderPhone,
		SenderName:  msg.SenderName,
		Content:     msg.Content,
		Timestamp:   msg.Timestamp,
	}

	// Save to database
	if err := l.rawMsgRepo.Save(ctx, rawMsg); err != nil {
		l.log.Error().Err(err).Str("msg_id", rawMsg.ID).Msg("Failed to save raw message")
		return
	}

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
		l.log.Debug().
			Str("msg_id", rawMsg.ID).
			Str("group", rawMsg.GroupName).
			Str("sender", rawMsg.SenderName).
			Msg("Message queued for processing")
	default:
		l.log.Warn().Str("msg_id", rawMsg.ID).Msg("Message queue full, dropping message")
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

	g := &domain.Group{
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
		group := &domain.Group{
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
