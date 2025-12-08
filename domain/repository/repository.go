// Package repository defines interfaces for data persistence.
// Following Interface Segregation Principle (ISP): small, focused interfaces.
package repository

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
)

// OfferReader provides read operations for offers
type OfferReader interface {
	GetByID(ctx context.Context, id string) (*entity.Offer, error)
	GetActive(ctx context.Context, limit, offset int) ([]*entity.Offer, error)
	Search(ctx context.Context, query string, limit, offset int) ([]*entity.Offer, error)
	CountActive(ctx context.Context) (int64, error)
}

// OfferWriter provides write operations for offers
type OfferWriter interface {
	Save(ctx context.Context, offer *entity.Offer) error
	UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error
}

// OfferRepository combines read and write operations
type OfferRepository interface {
	OfferReader
	OfferWriter
}

// RequestReader provides read operations for requests
type RequestReader interface {
	GetByID(ctx context.Context, id string) (*entity.Request, error)
	GetActive(ctx context.Context, limit, offset int) ([]*entity.Request, error)
	Search(ctx context.Context, query string, limit, offset int) ([]*entity.Request, error)
	CountActive(ctx context.Context) (int64, error)
}

// RequestWriter provides write operations for requests
type RequestWriter interface {
	Save(ctx context.Context, req *entity.Request) error
	UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error
}

// RequestRepository combines read and write operations
type RequestRepository interface {
	RequestReader
	RequestWriter
}

// MatchReader provides read operations for matches
type MatchReader interface {
	GetByID(ctx context.Context, id string) (*entity.Match, error)
	GetPending(ctx context.Context, limit, offset int) ([]*entity.MatchWithDetails, error)
	GetByOfferID(ctx context.Context, offerID string) ([]*entity.Match, error)
	GetByRequestID(ctx context.Context, requestID string) ([]*entity.Match, error)
	CountPending(ctx context.Context) (int64, error)
	CountConfirmedToday(ctx context.Context) (int64, error)
}

// MatchWriter provides write operations for matches
type MatchWriter interface {
	Save(ctx context.Context, match *entity.Match) error
	UpdateStatus(ctx context.Context, id string, status entity.MatchStatus, matchedBy string) error
}

// MatchRepository combines read and write operations
type MatchRepository interface {
	MatchReader
	MatchWriter
}

// RawMessageRepository handles raw message storage
type RawMessageRepository interface {
	Save(ctx context.Context, msg *entity.RawMessage) error
	GetByID(ctx context.Context, id string) (*entity.RawMessage, error)
	GetUnprocessed(ctx context.Context, limit int) ([]*entity.RawMessage, error)
	MarkProcessed(ctx context.Context, id string, err error) error
	GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*entity.RawMessage, error)
	ArchiveOldMessages(ctx context.Context, archivePath string, cutoff time.Time) (int64, error)
}

// GroupRepository handles group storage
type GroupRepository interface {
	Save(ctx context.Context, group *entity.Group) error
	GetAll(ctx context.Context) ([]*entity.Group, error)
	GetMonitored(ctx context.Context) ([]*entity.Group, error)
	SetMonitored(ctx context.Context, jid string, monitored bool) error
	UpdateLastMessage(ctx context.Context, jid string) error
	IncrementMessageCount(ctx context.Context, jid string) error
}

// StatsRepository provides dashboard statistics
type StatsRepository interface {
	GetStats(ctx context.Context) (*entity.Stats, error)
	GetProcessedToday(ctx context.Context) (int64, error)
}

// MedicationMappingRepository handles medication mappings
type MedicationMappingRepository interface {
	Save(ctx context.Context, mapping *entity.MedicationMapping) error
	GetByArabicName(ctx context.Context, arabicName string) (*entity.MedicationMapping, error)
	GetAll(ctx context.Context) ([]*entity.MedicationMapping, error)
	Search(ctx context.Context, query string) ([]*entity.MedicationMapping, error)
	Count(ctx context.Context) (int, error)
}

// MatchQueueRepository handles persistent match job queue
type MatchQueueRepository interface {
	Enqueue(ctx context.Context, item *entity.MatchQueueItem) error
	DequeueBatch(ctx context.Context, limit int) ([]*entity.MatchQueueItem, error)
	Delete(ctx context.Context, id string) error
	Count(ctx context.Context) (int, error)
}

// ReviewQueueRepository handles review queue operations
type ReviewQueueRepository interface {
	Save(ctx context.Context, item *entity.ReviewQueueItem) error
	GetByID(ctx context.Context, id string) (*entity.ReviewQueueItem, error)
	GetPending(ctx context.Context, limit, offset int) ([]*entity.ReviewQueueItem, error)
	CountPending(ctx context.Context) (int64, error)
	Approve(ctx context.Context, id string, reviewedBy string, correctedItems []entity.ParsedItem, note string) error
	Reject(ctx context.Context, id string, reviewedBy string, reason string) error
	GetByRawMessageID(ctx context.Context, rawMessageID string) (*entity.ReviewQueueItem, error)
}
