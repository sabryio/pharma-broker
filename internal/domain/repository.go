package domain

import (
	"context"
	"time"
)

// MatchQueueItem represents a job in the persistent queue
type MatchQueueItem struct {
	ID         string
	SourceType string // "OFFER" or "REQUEST"
	SourceID   string
	CreatedAt  time.Time
}

// RawMessageRepository handles raw message storage
type RawMessageRepository interface {
	Save(ctx context.Context, msg *RawMessage) error
	GetByID(ctx context.Context, id string) (*RawMessage, error)
	GetUnprocessed(ctx context.Context, limit int) ([]*RawMessage, error)
	MarkProcessed(ctx context.Context, id string, err error) error
	GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*RawMessage, error)
}

// OfferRepository handles offer storage
type OfferRepository interface {
	Save(ctx context.Context, offer *Offer) error
	GetByID(ctx context.Context, id string) (*Offer, error)
	GetActive(ctx context.Context, limit, offset int) ([]*Offer, error)
	Search(ctx context.Context, query string, limit, offset int) ([]*Offer, error)
	UpdateStatus(ctx context.Context, id string, status ItemStatus) error
	CountActive(ctx context.Context) (int64, error)
}

// RequestRepository handles request storage
type RequestRepository interface {
	Save(ctx context.Context, req *Request) error
	GetByID(ctx context.Context, id string) (*Request, error)
	GetActive(ctx context.Context, limit, offset int) ([]*Request, error)
	Search(ctx context.Context, query string, limit, offset int) ([]*Request, error)
	UpdateStatus(ctx context.Context, id string, status ItemStatus) error
	CountActive(ctx context.Context) (int64, error)
}

// MatchRepository handles match storage
type MatchRepository interface {
	Save(ctx context.Context, match *Match) error
	GetByID(ctx context.Context, id string) (*Match, error)
	GetPending(ctx context.Context, limit, offset int) ([]*MatchWithDetails, error)
	GetByOfferID(ctx context.Context, offerID string) ([]*Match, error)
	GetByRequestID(ctx context.Context, requestID string) ([]*Match, error)
	UpdateStatus(ctx context.Context, id string, status MatchStatus, matchedBy string) error
	CountPending(ctx context.Context) (int64, error)
	CountConfirmedToday(ctx context.Context) (int64, error)
}

// GroupRepository handles group storage
type GroupRepository interface {
	Save(ctx context.Context, group *Group) error
	GetAll(ctx context.Context) ([]*Group, error)
	GetMonitored(ctx context.Context) ([]*Group, error)
	SetMonitored(ctx context.Context, jid string, monitored bool) error
	UpdateLastMessage(ctx context.Context, jid string) error
	IncrementMessageCount(ctx context.Context, jid string) error
}

// StatsRepository provides dashboard statistics
type StatsRepository interface {
	GetStats(ctx context.Context) (*Stats, error)
	GetProcessedToday(ctx context.Context) (int64, error)
}

// MedicationMappingRepository defines storage operations for medication mappings
type MedicationMappingRepository interface {
	Save(ctx context.Context, mapping *MedicationMapping) error
	GetByArabicName(ctx context.Context, arabicName string) (*MedicationMapping, error)
	GetAll(ctx context.Context) ([]*MedicationMapping, error)
	Search(ctx context.Context, query string) ([]*MedicationMapping, error)
	Count(ctx context.Context) (int, error)
}

// MatchQueueRepository handles persistent job queue
type MatchQueueRepository interface {
	Enqueue(ctx context.Context, item *MatchQueueItem) error
	DequeueBatch(ctx context.Context, limit int) ([]*MatchQueueItem, error)
	Delete(ctx context.Context, id string) error
	Count(ctx context.Context) (int, error)
}
