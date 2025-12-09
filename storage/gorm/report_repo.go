// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"fmt"
	"strings"
	"time"
)

// ReportConfig holds configuration for report generation
type ReportConfig struct {
	PeriodHours      int
	MinScore         float64
	Limit            int
	IncludePending   bool
	IncludeConfirmed bool
	IncludeRejected  bool
}

// MatchReport represents a match with full details for reporting
type MatchReport struct {
	MatchID           string
	CreatedAt         time.Time
	Status            string
	Confidence        string
	Score             float64
	Breakdown         string
	OfferMedication   string
	OfferQty          float64
	OfferUnit         string
	OfferPrice        float64
	SellerName        string
	SellerPhone       string
	SellerGroupJID    string
	SellerGroup       string
	SellerWhatsApp    string
	OfferCreatedAt    time.Time
	RequestMedication string
	RequestQty        float64
	RequestMaxPrice   float64
	RequestUrgent     bool
	BuyerName         string
	BuyerPhone        string
	BuyerGroupJID     string
	BuyerGroup        string
	BuyerWhatsApp     string
	RequestCreatedAt  time.Time
}

// ReportSummary holds summary statistics
type ReportSummary struct {
	PeriodStart    time.Time
	PeriodEnd      time.Time
	TotalMatches   int
	ConfirmedCount int
	PendingCount   int
	RejectedCount  int
	HighConfidence int
	MedConfidence  int
	LowConfidence  int
	TopMedications []string
}

// Alert represents an item requiring attention
type Alert struct {
	Type     string
	Priority string
	Message  string
}

// ReportRepo implements report data fetching
type ReportRepo struct {
	db *DB
}

// NewReportRepo creates a new report repository
func NewReportRepo(db *DB) *ReportRepo {
	return &ReportRepo{db: db}
}

// GetMatchesForReport fetches matches with full contact details
func (r *ReportRepo) GetMatchesForReport(ctx context.Context, config ReportConfig) ([]*MatchReport, error) {
	periodStart := time.Now().Add(-time.Duration(config.PeriodHours) * time.Hour)

	var statusConditions []string
	if config.IncludePending {
		statusConditions = append(statusConditions, "'PENDING'")
	}
	if config.IncludeConfirmed {
		statusConditions = append(statusConditions, "'CONFIRMED'")
	}
	if config.IncludeRejected {
		statusConditions = append(statusConditions, "'REJECTED'")
	}

	if len(statusConditions) == 0 {
		statusConditions = []string{"'PENDING'", "'CONFIRMED'"}
	}

	limit := config.Limit
	if limit <= 0 {
		limit = 1000
	}

	query := fmt.Sprintf(`
		SELECT 
			m.id as match_id,
			m.created_at,
			m.status,
			COALESCE(m.matched_by, '') as confidence,
			m.score,
			COALESCE(m.reasoning, '') as breakdown,
			
			o.medication as offer_medication,
			o.quantity as offer_qty,
			COALESCE(o.unit, '') as offer_unit,
			COALESCE(o.price, 0) as offer_price,
			COALESCE(o.source_name, '') as seller_name,
			COALESCE(o.source_phone, '') as seller_phone,
			o.source_group as seller_group_jid,
			COALESCE(og.name, o.source_group) as seller_group_name,
			o.created_at as offer_created_at,
			
			r.medication as request_medication,
			r.quantity as request_qty,
			COALESCE(r.max_price, 0) as request_max_price,
			COALESCE(r.urgent, 0) as request_urgent,
			COALESCE(r.source_name, '') as buyer_name,
			COALESCE(r.source_phone, '') as buyer_phone,
			r.source_group as buyer_group_jid,
			COALESCE(rg.name, r.source_group) as buyer_group_name,
			r.created_at as request_created_at

		FROM matches m
		JOIN offers o ON m.offer_id = o.id
		JOIN requests r ON m.request_id = r.id

		LEFT JOIN groups og ON o.source_group = og.jid
		LEFT JOIN groups rg ON r.source_group = rg.jid
		WHERE m.created_at >= ?
		  AND m.score >= ?
		  AND m.status IN (%s)
		ORDER BY m.score DESC, m.created_at DESC
		LIMIT ?
	`, strings.Join(statusConditions, ", "))

	var results []struct {
		MatchID           string
		CreatedAt         time.Time
		Status            string
		Confidence        string
		Score             float64
		Breakdown         string
		OfferMedication   string
		OfferQty          float64
		OfferUnit         string
		OfferPrice        float64
		SellerName        string
		SellerPhone       string
		SellerGroupJID    string
		SellerGroupName   string
		OfferCreatedAt    time.Time
		RequestMedication string
		RequestQty        float64
		RequestMaxPrice   float64
		RequestUrgent     int
		BuyerName         string
		BuyerPhone        string
		BuyerGroupJID     string
		BuyerGroupName    string
		RequestCreatedAt  time.Time
	}

	err := r.db.Conn.WithContext(ctx).Raw(query, periodStart, config.MinScore, limit).Scan(&results).Error
	if err != nil {
		return nil, err
	}

	matchReports := make([]*MatchReport, len(results))
	for i, res := range results {
		matchReports[i] = &MatchReport{
			MatchID:           res.MatchID,
			CreatedAt:         res.CreatedAt,
			Status:            res.Status,
			Confidence:        res.Confidence,
			Score:             res.Score,
			Breakdown:         res.Breakdown,
			OfferMedication:   res.OfferMedication,
			OfferQty:          res.OfferQty,
			OfferUnit:         res.OfferUnit,
			OfferPrice:        res.OfferPrice,
			SellerName:        res.SellerName,
			SellerPhone:       res.SellerPhone,
			SellerGroupJID:    res.SellerGroupJID,
			SellerGroup:       res.SellerGroupName,
			OfferCreatedAt:    res.OfferCreatedAt,
			RequestMedication: res.RequestMedication,
			RequestQty:        res.RequestQty,
			RequestMaxPrice:   res.RequestMaxPrice,
			RequestUrgent:     res.RequestUrgent == 1,
			BuyerName:         res.BuyerName,
			BuyerPhone:        res.BuyerPhone,
			BuyerGroupJID:     res.BuyerGroupJID,
			BuyerGroup:        res.BuyerGroupName,
			RequestCreatedAt:  res.RequestCreatedAt,
		}
	}

	return matchReports, nil
}

// GetReportSummary generates summary statistics
func (r *ReportRepo) GetReportSummary(ctx context.Context, periodHours int) (*ReportSummary, error) {
	periodStart := time.Now().Add(-time.Duration(periodHours) * time.Hour)

	summary := &ReportSummary{
		PeriodStart: periodStart,
		PeriodEnd:   time.Now(),
	}

	var matchStats struct {
		TotalMatches   int
		ConfirmedCount int
		PendingCount   int
		RejectedCount  int
		HighConfidence int
		MedConfidence  int
		LowConfidence  int
	}

	r.db.Conn.WithContext(ctx).Raw(`
		SELECT 
			COUNT(*) as total_matches,
			SUM(CASE WHEN status = 'CONFIRMED' THEN 1 ELSE 0 END) as confirmed_count,
			SUM(CASE WHEN status = 'PENDING' THEN 1 ELSE 0 END) as pending_count,
			SUM(CASE WHEN status = 'REJECTED' THEN 1 ELSE 0 END) as rejected_count,
			SUM(CASE WHEN score >= 0.85 THEN 1 ELSE 0 END) as high_confidence,
			SUM(CASE WHEN score >= 0.7 AND score < 0.85 THEN 1 ELSE 0 END) as med_confidence,
			SUM(CASE WHEN score < 0.7 THEN 1 ELSE 0 END) as low_confidence
		FROM matches WHERE created_at >= ?
	`, periodStart).Scan(&matchStats)

	summary.TotalMatches = matchStats.TotalMatches
	summary.ConfirmedCount = matchStats.ConfirmedCount
	summary.PendingCount = matchStats.PendingCount
	summary.RejectedCount = matchStats.RejectedCount
	summary.HighConfidence = matchStats.HighConfidence
	summary.MedConfidence = matchStats.MedConfidence
	summary.LowConfidence = matchStats.LowConfidence

	var topMeds []struct {
		Medication string
	}
	r.db.Conn.WithContext(ctx).Raw(`
		SELECT medication
		FROM requests 
		WHERE created_at >= ? AND status = 'ACTIVE'
		GROUP BY medication 
		ORDER BY COUNT(*) DESC 
		LIMIT 5
	`, periodStart).Scan(&topMeds)

	for _, med := range topMeds {
		summary.TopMedications = append(summary.TopMedications, med.Medication)
	}

	return summary, nil
}

// GetAlerts fetches items requiring attention
func (r *ReportRepo) GetAlerts(ctx context.Context, periodHours int) ([]Alert, error) {
	periodStart := time.Now().Add(-time.Duration(periodHours) * time.Hour)
	var alerts []Alert

	var urgentCount int64
	r.db.Conn.WithContext(ctx).Raw(`
		SELECT COUNT(*) FROM requests r
		WHERE r.urgent = 1 AND r.status = 'ACTIVE'
		AND r.created_at >= ?
		AND NOT EXISTS (
			SELECT 1 FROM matches m 
			WHERE m.request_id = r.id AND m.status = 'CONFIRMED'
		)
	`, periodStart).Scan(&urgentCount)

	if urgentCount > 0 {
		alerts = append(alerts, Alert{
			Type:     "URGENT_UNMATCHED",
			Priority: "HIGH",
			Message:  fmt.Sprintf("%d urgent requests without confirmed matches", urgentCount),
		})
	}

	var lowScoreCount int64
	r.db.Conn.WithContext(ctx).
		Model(&Match{}).
		Where("status = 'PENDING' AND score < 0.7 AND created_at >= ?", periodStart).
		Count(&lowScoreCount)

	if lowScoreCount > 0 {
		alerts = append(alerts, Alert{
			Type:     "LOW_SCORE_PENDING",
			Priority: "MEDIUM",
			Message:  fmt.Sprintf("%d low-confidence matches need review", lowScoreCount),
		})
	}

	return alerts, nil
}
