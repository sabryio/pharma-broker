// Package domain provides backward-compatible type aliases.
package domain

import "pharmabroker/domain/entity"

// ========================================
// Feedback Type Aliases
// ========================================

type FeedbackAction = entity.FeedbackAction

const (
	MatchFeedbackConfirmed = entity.FeedbackActionConfirmed
	MatchFeedbackRejected  = entity.FeedbackActionRejected
)

type WeightSource = entity.WeightSource

const (
	WeightSourceDefault     = entity.WeightSourceDefault
	WeightSourceManual      = entity.WeightSourceManual
	WeightSourceAutoLearned = entity.WeightSourceAutoLearned
)

type FeedbackRecord = entity.FeedbackRecord
type WeightHistory = entity.WeightHistory
type FeedbackStats = entity.FeedbackStats
type PerformanceMetrics = entity.PerformanceMetrics

// ========================================
// Audit Type Aliases
// ========================================

type AuditAction = entity.AuditAction

const (
	AuditMatchConfirmed  = entity.AuditMatchConfirmed
	AuditMatchRejected   = entity.AuditMatchRejected
	AuditConfigChanged   = entity.AuditConfigChanged
	AuditGroupEnabled    = entity.AuditGroupEnabled
	AuditGroupDisabled   = entity.AuditGroupDisabled
	AuditReportGenerated = entity.AuditReportGenerated
)

type AuditLog = entity.AuditLog
type FeedbackAnalysis = entity.FeedbackAnalysis
