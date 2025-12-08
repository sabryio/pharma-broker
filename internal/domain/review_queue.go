// Package domain provides backward-compatible type aliases.
package domain

import (
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// ========================================
// Review Queue Type Aliases
// ========================================

type ReviewStatus = entity.ReviewStatus

const (
	ReviewStatusPending  = entity.ReviewStatusPending
	ReviewStatusApproved = entity.ReviewStatusApproved
	ReviewStatusRejected = entity.ReviewStatusRejected
)

type ReviewQueueItem = entity.ReviewQueueItem
type ReviewQueueRepository = repository.ReviewQueueRepository
