// Package domain provides backward compatibility setup for the new pharmabroker/domain module.
// This file validates that the new module is accessible from the old package.
//
// MIGRATION GUIDE:
// 1. New code should import from pharmabroker/domain/entity and pharmabroker/domain/repository
// 2. Once all code is migrated, remove models.go, repository.go, etc. from this package
// 3. Then enable the type aliases below to maintain backward compatibility
package domain

import (
	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that new module is accessible
var (
	_ entity.MessageType         = entity.MessageTypeOffer
	_ repository.OfferRepository = nil
)

// FUTURE: After removing original declarations from this package,
// uncomment these type aliases for backward compatibility:
//
// type (
// 	MessageType      = entity.MessageType
// 	ItemStatus       = entity.ItemStatus
// 	MatchStatus      = entity.MatchStatus
// 	RawMessage       = entity.RawMessage
// 	Offer            = entity.Offer
// 	Request          = entity.Request
// 	Match            = entity.Match
// 	MatchWithDetails = entity.MatchWithDetails
// 	Group            = entity.Group
// 	Stats            = entity.Stats
// )
//
// type (
// 	OfferRepository   = repository.OfferRepository
// 	RequestRepository = repository.RequestRepository
// 	MatchRepository   = repository.MatchRepository
// )
