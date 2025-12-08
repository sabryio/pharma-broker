// Package domain provides backward-compatible type aliases.
package domain

import "pharmabroker/domain/entity"

// UnmappedMedication type alias
type UnmappedMedication = entity.UnmappedMedication

// UnmappedMedicationRepo defines the interface for unmapped medication storage.
// NOTE: This interface is kept in internal/domain as it differs from the
// standard repository pattern (no context, different method signatures).
type UnmappedMedicationRepo interface {
	// Save creates or updates an unmapped medication record
	// If the same RawText already exists, it increments the count
	Save(rawText, aiOutput, sourceMessage, sourceGroup, messageID string) error

	// GetPending returns unmapped medications that haven't been reviewed
	GetPending(limit, offset int) ([]*UnmappedMedication, error)

	// GetByRawText finds an unmapped medication by raw text
	GetByRawText(rawText string) (*UnmappedMedication, error)

	// MarkReviewed marks a medication as reviewed with the approved English name
	MarkReviewed(id uint, approvedName, reviewedBy string) error

	// Count returns the total number of unmapped medications
	Count() (int64, error)

	// CountPending returns number of pending reviews
	CountPending() (int64, error)
}
