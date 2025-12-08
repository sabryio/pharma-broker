// Package domain provides backward-compatible type aliases to the new domain/entity types.
// This allows the existing internal/ code to work with the new clean architecture types.
package domain

import (
	"pharmabroker/domain/entity"
)

// ========================================
// Type Aliases - Status Enums
// ========================================

type MessageType = entity.MessageType

const (
	MessageTypeOffer   = entity.MessageTypeOffer
	MessageTypeRequest = entity.MessageTypeRequest
	MessageTypeBoth    = entity.MessageTypeBoth
	MessageTypeUnknown = entity.MessageTypeUnknown
)

type ItemStatus = entity.ItemStatus

const (
	StatusActive   = entity.StatusActive
	StatusMatched  = entity.StatusMatched
	StatusExpired  = entity.StatusExpired
	StatusArchived = entity.StatusArchived
)

type MatchStatus = entity.MatchStatus

const (
	MatchStatusPending   = entity.MatchStatusPending
	MatchStatusConfirmed = entity.MatchStatusConfirmed
	MatchStatusRejected  = entity.MatchStatusRejected
)

type FeedbackDecision = entity.FeedbackDecision

const (
	FeedbackConfirmed = entity.FeedbackConfirmed
	FeedbackRejected  = entity.FeedbackRejected
)

// ========================================
// Type Aliases - Core Entities
// ========================================

type RawMessage = entity.RawMessage
type Offer = entity.Offer
type Request = entity.Request
type Match = entity.Match
type MatchWithDetails = entity.MatchWithDetails
type Group = entity.Group
type Stats = entity.Stats

// ========================================
// Type Aliases - AI Processing
// ========================================

type ParsedItem = entity.ParsedItem
type AIParseResult = entity.AIParseResult
type FailedMessage = entity.FailedMessage
type MatchFeedback = entity.MatchFeedback
type DemandStats = entity.DemandStats
