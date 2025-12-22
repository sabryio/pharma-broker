package whatsapp

import (
	"context"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/messaging/deduplicator"
	"pharmabroker/messaging/queue"
)

// Constants for listener configuration
const (
	deduplicationTimeout     = 2 * time.Second
	deduplicationWindow      = 10 * time.Second
	groupCheckTimeout        = 5 * time.Second
	groupStatsUpdateTimeout  = 5 * time.Second
	defaultLogTruncateLength = 100
)

// Listener listens for WhatsApp messages and queues them for processing.
type Listener struct {
	log                    zerolog.Logger
	rawMsgRepo             repository.RawMessageRepository
	groupRepo              repository.GroupRepository
	queue                  *queue.Queue[*entity.RawMessage]
	deduplicator           *deduplicator.Deduplicator[*entity.RawMessage]
	skipOwnMessagesChecker func() bool
}

// RawMessageLookup adapts RawMessageRepository to the generic Lookup interface.
type RawMessageLookup struct {
	repo repository.RawMessageRepository
}

// GetLast implements Lookup[entity.RawMessage].
func (l *RawMessageLookup) GetLast(ctx context.Context, groupID, senderID string) (*entity.RawMessage, error) {
	return l.repo.GetLastMessageBySender(ctx, groupID, senderID)
}

// NewListener creates a new message listener with enhanced queue and deduplication.
func NewListener(
	log zerolog.Logger,
	rawMsgRepo repository.RawMessageRepository,
	groupRepo repository.GroupRepository,
) *Listener {
	lookup := &RawMessageLookup{repo: rawMsgRepo}
	dedup, err := deduplicator.NewDeduplicator(
		context.Background(),
		deduplicator.DefaultDeduplicatorConfig(),
		lookup,
		log,
	)
	if err != nil {
		log.Error().Err(err).Msg("Failed to create deduplicator, using nil")
		dedup = nil
	}
	return &Listener{
		log:                    log.With().Str("component", "listener").Logger(),
		rawMsgRepo:             rawMsgRepo,
		groupRepo:              groupRepo,
		queue:                  queue.NewQueue[*entity.RawMessage](queue.DefaultQueueConfig(), log),
		deduplicator:           dedup,
		skipOwnMessagesChecker: func() bool { return true },
	}
}

// NewListenerWithConfig creates a listener with custom queue and deduplication config.
func NewListenerWithConfig(
	log zerolog.Logger,
	rawMsgRepo repository.RawMessageRepository,
	groupRepo repository.GroupRepository,
	queueCfg queue.QueueConfig,
	dedupCfg deduplicator.DeduplicatorConfig,
) *Listener {
	lookup := &RawMessageLookup{repo: rawMsgRepo}
	dedup, err := deduplicator.NewDeduplicator(
		context.Background(),
		dedupCfg,
		lookup,
		log,
	)
	if err != nil {
		log.Error().Err(err).Msg("Failed to create deduplicator, using nil")
		dedup = nil
	}
	return &Listener{
		log:                    log.With().Str("component", "listener").Logger(),
		rawMsgRepo:             rawMsgRepo,
		groupRepo:              groupRepo,
		queue:                  queue.NewQueue[*entity.RawMessage](queueCfg, log),
		deduplicator:           dedup,
		skipOwnMessagesChecker: func() bool { return true },
	}
}

// SetSkipOwnMessagesChecker sets the function to check if own messages should be skipped.
func (l *Listener) SetSkipOwnMessagesChecker(fn func() bool) {
	l.skipOwnMessagesChecker = fn
}

// SetMessageHandler sets the handler for processing messages.
func (l *Listener) SetMessageHandler(handler queue.MessageHandler[*entity.RawMessage]) {
	l.queue.SetHandler(handler)
}

// StartQueue starts the queue worker pool.
func (l *Listener) StartQueue() {
	l.queue.Start()
}

// StopQueue gracefully stops the queue.
func (l *Listener) StopQueue(ctx context.Context) error {
	return l.queue.Stop(ctx)
}

// GetQueue returns the queue for health checks.
func (l *Listener) GetQueue() *queue.Queue[*entity.RawMessage] {
	return l.queue
}

// QueueStats returns queue statistics.
func (l *Listener) QueueStats() queue.QueueStats {
	return l.queue.Stats()
}

// QueueHealth returns queue health status.
func (l *Listener) QueueHealth() queue.QueueHealth {
	return l.queue.HealthStatus()
}

// HandleMessage implements EventHandler interface
func (l *Listener) HandleMessage(msg *IncomingMessage) {
	l.logMessageReceived(msg)

	// Skip own messages if configured
	if l.shouldSkipOwnMessage(msg) {
		return
	}

	// Check for duplicate messages (spam/repetition filter)
	if l.isDuplicateMessage(msg) {
		return
	}

	// Check if this group is monitored
	ctx, cancel := context.WithTimeout(context.Background(), groupCheckTimeout)
	defer cancel()

	if !l.checkGroupMonitored(ctx, msg) {
		return
	}

	// Create and save raw message
	rawMsg := l.createRawMessage(msg)
	if err := l.saveMessage(ctx, rawMsg); err != nil {
		return
	}

	// Update group stats asynchronously
	l.updateGroupStatsAsync(msg.GroupJID)

	// Queue for processing
	l.queueMessage(rawMsg)
}

// logMessageReceived logs the incoming message
func (l *Listener) logMessageReceived(msg *IncomingMessage) {
	l.log.Info().
		Str("step", "1_RECEIVED").
		Str("group", msg.GroupName).
		Str("sender", msg.SenderName).
		Str("content_preview", truncate(msg.Content, defaultLogTruncateLength)).
		Msg("Message received from WhatsApp")
}

// shouldSkipOwnMessage checks if own messages should be skipped
func (l *Listener) shouldSkipOwnMessage(msg *IncomingMessage) bool {
	if !msg.IsFromMe {
		return false
	}

	if l.skipOwnMessagesChecker() {
		l.log.Debug().
			Str("step", "1_SKIPPED").
			Msg("Skipping own message (config: skip_own_messages=true)")
		return true
	}

	l.log.Info().
		Str("step", "1_OWN_MESSAGE").
		Str("content", msg.Content).
		Msg("Processing OWN message (config: skip_own_messages=false)")
	return false
}

// isDuplicateMessage checks if this is a duplicate message within the deduplication window.
func (l *Listener) isDuplicateMessage(msg *IncomingMessage) bool {
	ctx, cancel := context.WithTimeout(context.Background(), deduplicationTimeout)
	defer cancel()

	// Use enhanced deduplicator if available (fast path with cache)
	if l.deduplicator != nil {
		isDupe := l.deduplicator.IsDuplicate(ctx, msg.GroupJID, msg.SenderJID, msg.Content, msg.Timestamp)
		if isDupe {
			l.log.Warn().
				Str("step", "1_DUPLICATE_IGNORED").
				Str("sender", msg.SenderName).
				Str("content", truncate(msg.Content, 50)).
				Msg("Ignoring duplicate message")
			return true
		}
		l.deduplicator.RecordMessage(msg.GroupJID, msg.SenderJID, msg.Content, msg.Timestamp)
		return false
	}

	// Fallback: DB-based deduplication
	lastMsg, err := l.rawMsgRepo.GetLastMessageBySender(ctx, msg.GroupJID, msg.SenderJID)
	if err != nil || lastMsg == nil {
		return false
	}

	timeDiff := absDuration(msg.Timestamp.Sub(lastMsg.Timestamp))
	if timeDiff < deduplicationWindow && lastMsg.Content == msg.Content {
		l.log.Warn().
			Str("step", "1_DUPLICATE_IGNORED").
			Str("sender", msg.SenderName).
			Str("content", truncate(msg.Content, 50)).
			Dur("time_diff", timeDiff).
			Msg("Ignoring duplicate message")
		return true
	}

	return false
}

// absDuration returns the absolute value of a duration
func absDuration(d time.Duration) time.Duration {
	if d < 0 {
		return -d
	}
	return d
}

// checkGroupMonitored verifies the group is monitored and logs appropriately
func (l *Listener) checkGroupMonitored(ctx context.Context, msg *IncomingMessage) bool {
	monitored, err := l.isGroupMonitored(ctx, msg.GroupJID)
	if err != nil {
		l.log.Error().Err(err).Str("jid", msg.GroupJID).Msg("Failed to check if group is monitored")
		return false
	}

	if !monitored {
		l.log.Warn().
			Str("step", "2_NOT_MONITORED").
			Str("group", msg.GroupName).
			Str("jid", msg.GroupJID).
			Msg("Group NOT monitored - message ignored")
		return false
	}

	l.log.Info().
		Str("step", "2_MONITORED").
		Str("group", msg.GroupName).
		Msg("Group is monitored - processing")
	return true
}

// createRawMessage converts an IncomingMessage to a RawMessage entity
func (l *Listener) createRawMessage(msg *IncomingMessage) *entity.RawMessage {
	return &entity.RawMessage{
		ID:             uuid.New().String(),
		ExternalID:     msg.ID,
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
}

// saveMessage persists the raw message to the database
func (l *Listener) saveMessage(ctx context.Context, rawMsg *entity.RawMessage) error {
	if err := l.rawMsgRepo.Save(ctx, rawMsg); err != nil {
		l.log.Error().Err(err).Str("msg_id", rawMsg.ID).Msg("Failed to save raw message")
		return err
	}

	l.log.Info().
		Str("step", "3_SAVED").
		Str("msg_id", rawMsg.ID).
		Str("group", rawMsg.GroupName).
		Msg("Message saved to database")
	return nil
}

// updateGroupStatsAsync updates group statistics in a background goroutine
func (l *Listener) updateGroupStatsAsync(groupJID string) {
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), groupStatsUpdateTimeout)
		defer cancel()

		if err := l.groupRepo.UpdateLastMessage(ctx, groupJID); err != nil {
			l.log.Debug().Err(err).Str("jid", groupJID).Msg("Failed to update last message timestamp")
		}
		if err := l.groupRepo.IncrementMessageCount(ctx, groupJID); err != nil {
			l.log.Debug().Err(err).Str("jid", groupJID).Msg("Failed to increment message count")
		}
	}()
}

// queueMessage adds the message to the processing queue.
func (l *Listener) queueMessage(rawMsg *entity.RawMessage) {
	if l.queue.Enqueue(&rawMsg) {
		l.log.Info().
			Str("step", "4_QUEUED").
			Str("msg_id", rawMsg.ID).
			Int("queue_size", l.queue.Size()).
			Msg("Message queued for AI processing")
	} else {
		l.log.Error().
			Str("msg_id", rawMsg.ID).
			Msg("Message dropped - queue full")
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
	ctx, cancel := context.WithTimeout(context.Background(), groupCheckTimeout)
	defer cancel()

	g := l.groupInfoToEntity(group, true)

	if err := l.groupRepo.Save(ctx, g); err != nil {
		l.log.Error().Err(err).Str("jid", group.JID).Msg("Failed to save group")
		return
	}

	l.log.Info().Str("jid", group.JID).Str("name", group.Name).Msg("New group discovered")
}

// SyncGroups synchronizes known groups with database (defaults to not monitored)
func (l *Listener) SyncGroups(ctx context.Context, groups []*GroupInfo) error {
	for _, g := range groups {
		group := l.groupInfoToEntity(g, false)
		if err := l.groupRepo.Save(ctx, group); err != nil {
			return err
		}
	}
	return nil
}

// groupInfoToEntity converts GroupInfo to entity.Group
func (l *Listener) groupInfoToEntity(g *GroupInfo, monitored bool) *entity.Group {
	return &entity.Group{
		JID:         g.JID,
		Name:        g.Name,
		Description: g.Description,
		Monitored:   monitored,
		AddedAt:     time.Now(),
	}
}

// truncate limits string length for log output
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}
