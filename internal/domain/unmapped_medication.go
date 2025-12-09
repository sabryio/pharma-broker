// Package domain provides backward-compatible type aliases.
package domain

import (
	"context"

	"pharmabroker/domain/entity"
)

// UnmappedMedication type alias
type UnmappedMedication = entity.UnmappedMedication

// UnmappedMedicationRepo defines the interface for unmapped medication storage.
type UnmappedMedicationRepo interface {
	// Save creates or updates an unmapped medication record
	Save(ctx context.Context, rawText, aiOutput, sourceMessage, sourceGroup, messageID string) error

	// GetPending returns unmapped medications that haven't been reviewed
	GetPending(ctx context.Context, limit, offset int) ([]*UnmappedMedication, error)

	// GetByRawText finds an unmapped medication by raw text
	GetByRawText(ctx context.Context, rawText string) (*UnmappedMedication, error)

	// MarkReviewed marks a medication as reviewed with the approved English name
	MarkReviewed(ctx context.Context, id uint, approvedName, reviewedBy string) error

	// Count returns the total number of unmapped medications
	Count(ctx context.Context) (int64, error)

	// CountPending returns number of pending reviews
	CountPending(ctx context.Context) (int64, error)
}
