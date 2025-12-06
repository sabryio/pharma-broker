package storage

import (
	"context"
	"time"

	"pharmabroker/internal/reports"
)

// ReportRepo implements report data fetching
type ReportRepo struct {
	db *DB
}

// NewReportRepo creates a new ReportRepo
func NewReportRepo(db *DB) *ReportRepo {
	return &ReportRepo{db: db}
}

// GetMatchesForReport fetches matches with full contact details for reporting
func (r *ReportRepo) GetMatchesForReport(ctx context.Context, config reports.ReportConfig) ([]*reports.MatchReport, error) {
	periodStart := time.Now().Add(-time.Duration(config.PeriodHours) * time.Hour)

	// Build status filter
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

	query := `
		SELECT 
			m.id as match_id,
			m.created_at,
			m.status,
			COALESCE(m.matched_by, '') as confidence,
			m.score,
			COALESCE(m.reasoning, '') as breakdown,
			
			-- Offer details
			o.medication as offer_medication,
			o.quantity as offer_qty,
			COALESCE(o.unit, '') as offer_unit,
			COALESCE(o.price, 0) as offer_price,
			COALESCE(o.source_name, '') as seller_name,
			COALESCE(o.source_phone, '') as seller_phone,
			o.source_group as seller_group_jid,
			COALESCE(og.name, o.source_group) as seller_group_name,
			o.created_at as offer_created_at,
			
			-- Request details
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
		  AND m.status IN (` + joinStrings(statusConditions) + `)
		ORDER BY m.score DESC, m.created_at DESC
		LIMIT ?
	`

	rows, err := r.db.Reader().QueryContext(ctx, query, periodStart, config.MinScore, config.Limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var results []*reports.MatchReport
	for rows.Next() {
		mr := &reports.MatchReport{}
		var urgentInt int

		err := rows.Scan(
			&mr.MatchID,
			&mr.CreatedAt,
			&mr.Status,
			&mr.Confidence,
			&mr.Score,
			&mr.Breakdown,
			&mr.OfferMedication,
			&mr.OfferQty,
			&mr.OfferUnit,
			&mr.OfferPrice,
			&mr.SellerName,
			&mr.SellerPhone,
			&mr.SellerGroupJID,
			&mr.SellerGroup,
			&mr.OfferCreatedAt,
			&mr.RequestMedication,
			&mr.RequestQty,
			&mr.RequestMaxPrice,
			&urgentInt,
			&mr.BuyerName,
			&mr.BuyerPhone,
			&mr.BuyerGroupJID,
			&mr.BuyerGroup,
			&mr.RequestCreatedAt,
		)
		if err != nil {
			return nil, err
		}

		mr.RequestUrgent = urgentInt == 1

		// Generate WhatsApp links
		mr.SellerWhatsApp = reports.FormatWhatsAppLink(mr.SellerPhone, mr.MatchID)
		mr.BuyerWhatsApp = reports.FormatWhatsAppLink(mr.BuyerPhone, mr.MatchID)

		// Format phone for display
		mr.SellerPhone = reports.FormatPhoneDisplay(mr.SellerPhone)
		mr.BuyerPhone = reports.FormatPhoneDisplay(mr.BuyerPhone)

		results = append(results, mr)
	}

	return results, rows.Err()
}

// GetReportSummary generates summary statistics for the period
func (r *ReportRepo) GetReportSummary(ctx context.Context, periodHours int) (*reports.ReportSummary, error) {
	periodStart := time.Now().Add(-time.Duration(periodHours) * time.Hour)

	summary := &reports.ReportSummary{
		GeneratedAt: time.Now(),
		PeriodStart: periodStart,
		PeriodEnd:   time.Now(),
	}

	// Count by status
	statusQuery := `
		SELECT 
			status,
			COUNT(*) as count,
			AVG(score) as avg_score
		FROM matches
		WHERE created_at >= ?
		GROUP BY status
	`
	rows, err := r.db.Reader().QueryContext(ctx, statusQuery, periodStart)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var totalScore float64
	var totalCount int
	for rows.Next() {
		var status string
		var count int
		var avgScore float64
		if err := rows.Scan(&status, &count, &avgScore); err != nil {
			return nil, err
		}

		switch status {
		case "PENDING":
			summary.PendingCount = count
		case "CONFIRMED":
			summary.ConfirmedCount = count
		case "REJECTED":
			summary.RejectedCount = count
		}
		totalCount += count
		totalScore += avgScore * float64(count)
	}

	summary.TotalMatches = totalCount
	if totalCount > 0 {
		summary.AvgScore = totalScore / float64(totalCount)
	}

	// Count by confidence band
	confidenceQuery := `
		SELECT 
			COALESCE(matched_by, 'UNKNOWN') as confidence,
			COUNT(*) as count
		FROM matches
		WHERE created_at >= ?
		GROUP BY matched_by
	`
	confRows, err := r.db.Reader().QueryContext(ctx, confidenceQuery, periodStart)
	if err != nil {
		return nil, err
	}
	defer confRows.Close()

	for confRows.Next() {
		var confidence string
		var count int
		if err := confRows.Scan(&confidence, &count); err != nil {
			return nil, err
		}

		switch confidence {
		case "AUTO":
			summary.HighConfidence = count
		case "SUGGEST":
			summary.MedConfidence = count
		case "REVIEW":
			summary.LowConfidence = count
		}
	}

	// Count urgent matches
	urgentQuery := `
		SELECT COUNT(DISTINCT m.id)
		FROM matches m
		JOIN requests r ON m.request_id = r.id
		WHERE m.created_at >= ? AND r.urgent = 1
	`
	r.db.Reader().QueryRowContext(ctx, urgentQuery, periodStart).Scan(&summary.UrgentMatches)

	// Get top medications
	topMedsQuery := `
		SELECT r.medication, COUNT(*) as cnt
		FROM matches m
		JOIN requests r ON m.request_id = r.id
		WHERE m.created_at >= ?
		GROUP BY r.medication
		ORDER BY cnt DESC
		LIMIT 5
	`
	medRows, err := r.db.Reader().QueryContext(ctx, topMedsQuery, periodStart)
	if err != nil {
		return nil, err
	}
	defer medRows.Close()

	for medRows.Next() {
		var med string
		var cnt int
		if err := medRows.Scan(&med, &cnt); err != nil {
			continue
		}
		summary.TopMedications = append(summary.TopMedications, med)
	}

	return summary, nil
}

// GetAlerts fetches items requiring attention
func (r *ReportRepo) GetAlerts(ctx context.Context, periodHours int) ([]reports.Alert, error) {
	var alerts []reports.Alert
	periodStart := time.Now().Add(-time.Duration(periodHours) * time.Hour)

	// High-value pending matches (score > 0.8)
	pendingQuery := `
		SELECT m.id, m.score, o.medication
		FROM matches m
		JOIN offers o ON m.offer_id = o.id
		WHERE m.created_at >= ?
		  AND m.status = 'PENDING'
		  AND m.score >= 0.8
		ORDER BY m.score DESC
		LIMIT 5
	`
	pendingRows, err := r.db.Reader().QueryContext(ctx, pendingQuery, periodStart)
	if err == nil {
		defer pendingRows.Close()
		for pendingRows.Next() {
			var id, med string
			var score float64
			if pendingRows.Scan(&id, &score, &med) == nil {
				alerts = append(alerts, reports.Alert{
					Type:     reports.AlertHighValuePending,
					Priority: "HIGH",
					Message:  "High-confidence match pending: " + med + " (score: " + formatScore(score) + ")",
					MatchID:  id,
				})
			}
		}
	}

	// Urgent unmatched requests
	urgentQuery := `
		SELECT r.id, r.medication, r.source_name
		FROM requests r
		LEFT JOIN matches m ON r.id = m.request_id
		WHERE r.created_at >= ?
		  AND r.urgent = 1
		  AND r.status = 'ACTIVE'
		  AND m.id IS NULL
		LIMIT 5
	`
	urgentRows, err := r.db.Reader().QueryContext(ctx, urgentQuery, periodStart)
	if err == nil {
		defer urgentRows.Close()
		for urgentRows.Next() {
			var id, med, name string
			if urgentRows.Scan(&id, &med, &name) == nil {
				alerts = append(alerts, reports.Alert{
					Type:     reports.AlertUrgentUnmatched,
					Priority: "HIGH",
					Message:  "Urgent request unmatched: " + med + " by " + name,
				})
			}
		}
	}

	return alerts, nil
}

func joinStrings(strs []string) string {
	result := ""
	for i, s := range strs {
		if i > 0 {
			result += ", "
		}
		result += s
	}
	return result
}

func formatScore(score float64) string {
	if score >= 1.0 {
		return "1.00"
	}
	tens := int(score*100) / 10
	ones := int(score*100) % 10
	return "0." + string(rune(tens+'0')) + string(rune(ones+'0'))
}
