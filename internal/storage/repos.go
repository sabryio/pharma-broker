package storage

import (
	"github.com/rs/zerolog"

	"pharmabroker/internal/domain"
)

// GormRepos holds all GORM-based repository instances
type GormRepos struct {
	RawMessage        domain.RawMessageRepository
	Offer             domain.OfferRepository
	Request           domain.RequestRepository
	Match             domain.MatchRepository
	MatchQueue        domain.MatchQueueRepository
	Group             domain.GroupRepository
	MedicationMapping domain.MedicationMappingRepository
	Config            *GormConfigRepo
	Feedback          *GormFeedbackRepo
	// Stats, Leaderboard, Audit, Report use old DB until migrated
}

// InitGormRepos initializes all GORM-based repositories
func InitGormRepos(db *GormDB, log zerolog.Logger) *GormRepos {
	log.Info().Msg("Initializing GORM repositories")
	return &GormRepos{
		RawMessage:        NewGormRawMessageRepo(db),
		Offer:             NewGormOfferRepo(db),
		Request:           NewGormRequestRepo(db),
		Match:             NewGormMatchRepo(db),
		MatchQueue:        NewGormMatchQueueRepo(db),
		Group:             NewGormGroupRepo(db),
		MedicationMapping: NewGormMedicationMappingRepo(db),
		Config:            NewGormConfigRepo(db),
		Feedback:          NewGormFeedbackRepo(db),
	}
}

// Close closes the underlying database connection
func (r *GormRepos) Close() error {
	// Close is delegated to GormDB, not repositories
	return nil
}

// ====================
// Backwards-Compatible Constructors
// These wrap GORM repos for code expecting the old API: storage.NewXxxRepo(db)
// ====================

// NewRawMessageRepo creates a new GORM-based raw message repository
func NewRawMessageRepo(db *GormDB) *GormRawMessageRepo {
	return NewGormRawMessageRepo(db)
}

// NewOfferRepo creates a new GORM-based offer repository
func NewOfferRepo(db *GormDB) *GormOfferRepo {
	return NewGormOfferRepo(db)
}

// NewRequestRepo creates a new GORM-based request repository
func NewRequestRepo(db *GormDB) *GormRequestRepo {
	return NewGormRequestRepo(db)
}

// NewMatchRepo creates a new GORM-based match repository
func NewMatchRepo(db *GormDB) *GormMatchRepo {
	return NewGormMatchRepo(db)
}

// NewMatchQueueRepo creates a new GORM-based match queue repository
func NewMatchQueueRepo(db *GormDB) *GormMatchQueueRepo {
	return NewGormMatchQueueRepo(db)
}

// NewGroupRepo creates a new GORM-based group repository
func NewGroupRepo(db *GormDB) *GormGroupRepo {
	return NewGormGroupRepo(db)
}

// NewMedicationMappingRepo creates a new GORM-based medication mapping repository
func NewMedicationMappingRepo(db *GormDB) *GormMedicationMappingRepo {
	return NewGormMedicationMappingRepo(db)
}

// NewConfigRepo creates a new GORM-based config repository
func NewConfigRepo(db *GormDB) *GormConfigRepo {
	return NewGormConfigRepo(db)
}

// NewFeedbackRepo creates a new GORM-based feedback repository
func NewFeedbackRepo(db *GormDB) *GormFeedbackRepo {
	return NewGormFeedbackRepo(db)
}

// NewLeaderboardRepo creates a new GORM-based leaderboard repository
func NewLeaderboardRepo(db *GormDB) *GormLeaderboardRepo {
	return NewGormLeaderboardRepo(db)
}

// NewAuditRepo creates a new GORM-based audit repository
func NewAuditRepo(db *GormDB) *GormAuditRepo {
	return NewGormAuditRepo(db)
}

// NewReportRepo creates a new GORM-based report repository
func NewReportRepo(db *GormDB) *GormReportRepo {
	return NewGormReportRepo(db)
}
