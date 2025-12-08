// Package domain provides backward-compatible type and interface aliases.
package domain

import (
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// MatchQueueItem type alias
type MatchQueueItem = entity.MatchQueueItem

// ========================================
// Repository Interface Aliases
// ========================================

type RawMessageRepository = repository.RawMessageRepository
type OfferRepository = repository.OfferRepository
type RequestRepository = repository.RequestRepository
type MatchRepository = repository.MatchRepository
type GroupRepository = repository.GroupRepository
type StatsRepository = repository.StatsRepository
type MedicationMappingRepository = repository.MedicationMappingRepository
type MatchQueueRepository = repository.MatchQueueRepository
