// Package storage provides the storage interface and database configuration.
// This package re-exports repository interfaces from domain for convenience.
package storage

import (
	"pharmabroker/domain/repository"
)

// Re-export repository interfaces for convenient imports
type (
	OfferRepository             = repository.OfferRepository
	RequestRepository           = repository.RequestRepository
	MatchRepository             = repository.MatchRepository
	RawMessageRepository        = repository.RawMessageRepository
	GroupRepository             = repository.GroupRepository
	StatsRepository             = repository.StatsRepository
	MedicationMappingRepository = repository.MedicationMappingRepository
	MatchQueueRepository        = repository.MatchQueueRepository
	ReviewQueueRepository       = repository.ReviewQueueRepository
)

// DatabaseConfig holds database connection settings
type DatabaseConfig struct {
	Path            string
	MaxOpenConns    int
	MaxIdleConns    int
	ConnMaxLifetime int // seconds
}
