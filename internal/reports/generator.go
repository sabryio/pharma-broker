package reports

import (
	"bytes"
	"context"
	"encoding/csv"
	"fmt"
	"strconv"

	"github.com/rs/zerolog"
)

// ReportRepository interface for data fetching
type ReportRepository interface {
	GetMatchesForReport(ctx context.Context, config ReportConfig) ([]*MatchReport, error)
	GetReportSummary(ctx context.Context, periodHours int) (*ReportSummary, error)
	GetAlerts(ctx context.Context, periodHours int) ([]Alert, error)
}

// Generator generates reports from data
type Generator struct {
	repo ReportRepository
	log  zerolog.Logger
}

// NewGenerator creates a new report generator
func NewGenerator(repo ReportRepository, log zerolog.Logger) *Generator {
	return &Generator{
		repo: repo,
		log:  log.With().Str("component", "reports").Logger(),
	}
}

// GenerateHourlyReport creates a complete hourly report
func (g *Generator) GenerateHourlyReport(ctx context.Context, config ReportConfig) (*HourlyReport, error) {
	g.log.Info().
		Int("period_hours", config.PeriodHours).
		Float64("min_score", config.MinScore).
		Int("limit", config.Limit).
		Msg("Generating hourly report")

	// Fetch matches
	matches, err := g.repo.GetMatchesForReport(ctx, config)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch matches: %w", err)
	}

	// Fetch summary
	summary, err := g.repo.GetReportSummary(ctx, config.PeriodHours)
	if err != nil {
		return nil, fmt.Errorf("failed to generate summary: %w", err)
	}

	// Fetch alerts
	alerts, err := g.repo.GetAlerts(ctx, config.PeriodHours)
	if err != nil {
		g.log.Warn().Err(err).Msg("Failed to fetch alerts, continuing without")
		alerts = []Alert{}
	}

	report := &HourlyReport{
		Summary: *summary,
		Matches: matches,
		Alerts:  alerts,
	}

	g.log.Info().
		Int("matches", len(matches)).
		Int("alerts", len(alerts)).
		Int("pending", summary.PendingCount).
		Msg("Report generated successfully")

	return report, nil
}

// ExportToCSV exports the report to CSV format
func (g *Generator) ExportToCSV(report *HourlyReport) ([]byte, error) {
	var buf bytes.Buffer
	writer := csv.NewWriter(&buf)

	// Write header
	header := []string{
		"Match ID",
		"Created At",
		"Status",
		"Confidence",
		"Score",
		"Breakdown",
		"Offer Medication",
		"Offer Qty",
		"Offer Unit",
		"Offer Price",
		"Seller Name",
		"Seller Phone",
		"Seller WhatsApp",
		"Seller Group",
		"Offer Age",
		"Request Medication",
		"Request Qty",
		"Max Price",
		"Urgent",
		"Buyer Name",
		"Buyer Phone",
		"Buyer WhatsApp",
		"Buyer Group",
		"Request Age",
	}

	if err := writer.Write(header); err != nil {
		return nil, fmt.Errorf("failed to write header: %w", err)
	}

	// Write data rows
	for _, m := range report.Matches {
		row := []string{
			m.MatchID,
			m.CreatedAt.Format("2006-01-02 15:04:05"),
			m.Status,
			m.Confidence,
			fmt.Sprintf("%.2f", m.Score),
			m.Breakdown,
			m.OfferMedication,
			fmt.Sprintf("%.0f", m.OfferQty),
			m.OfferUnit,
			fmt.Sprintf("%.2f", m.OfferPrice),
			m.SellerName,
			m.SellerPhone,
			m.SellerWhatsApp,
			m.SellerGroup,
			CalculateAge(m.OfferCreatedAt),
			m.RequestMedication,
			fmt.Sprintf("%.0f", m.RequestQty),
			fmt.Sprintf("%.2f", m.RequestMaxPrice),
			strconv.FormatBool(m.RequestUrgent),
			m.BuyerName,
			m.BuyerPhone,
			m.BuyerWhatsApp,
			m.BuyerGroup,
			CalculateAge(m.RequestCreatedAt),
		}

		if err := writer.Write(row); err != nil {
			return nil, fmt.Errorf("failed to write row: %w", err)
		}
	}

	writer.Flush()
	if err := writer.Error(); err != nil {
		return nil, fmt.Errorf("csv write error: %w", err)
	}

	return buf.Bytes(), nil
}

// GenerateSummaryText creates a human-readable summary for messaging
func (g *Generator) GenerateSummaryText(report *HourlyReport) string {
	s := report.Summary

	var buf bytes.Buffer

	// Header
	buf.WriteString(fmt.Sprintf("📊 PharmaBroker Report - %s\n\n", s.GeneratedAt.Format("Jan 2, 15:04")))

	// Summary stats
	buf.WriteString("📈 Summary:\n")
	buf.WriteString(fmt.Sprintf("• Total matches: %d\n", s.TotalMatches))
	buf.WriteString(fmt.Sprintf("• Pending review: %d\n", s.PendingCount))
	buf.WriteString(fmt.Sprintf("• Auto-confirmed: %d\n", s.HighConfidence))
	buf.WriteString(fmt.Sprintf("• Avg score: %.2f\n", s.AvgScore))

	if s.UrgentMatches > 0 {
		buf.WriteString(fmt.Sprintf("• 🔥 Urgent matches: %d\n", s.UrgentMatches))
	}

	// Alerts
	if len(report.Alerts) > 0 {
		buf.WriteString("\n⚠️ Alerts:\n")
		for _, alert := range report.Alerts {
			priority := ""
			if alert.Priority == "HIGH" {
				priority = "🔴 "
			} else if alert.Priority == "MEDIUM" {
				priority = "🟡 "
			}
			buf.WriteString(fmt.Sprintf("• %s%s\n", priority, alert.Message))
		}
	}

	// Top pending matches
	pendingMatches := filterByStatus(report.Matches, "PENDING")
	if len(pendingMatches) > 0 {
		buf.WriteString("\n🔍 Top Pending Matches:\n")
		limit := min(3, len(pendingMatches))
		for i := 0; i < limit; i++ {
			m := pendingMatches[i]
			buf.WriteString(formatMatchLine(m))
		}
	}

	// Top medications
	if len(s.TopMedications) > 0 {
		buf.WriteString("\n💊 Top Medications:\n")
		for i, med := range s.TopMedications {
			if i >= 3 {
				break
			}
			buf.WriteString(fmt.Sprintf("• %s\n", med))
		}
	}

	return buf.String()
}

// GenerateHTMLReport creates an HTML table for email
func (g *Generator) GenerateHTMLReport(report *HourlyReport) string {
	var buf bytes.Buffer

	// Header
	buf.WriteString(`<!DOCTYPE html>
<html>
<head>
<style>
body { font-family: Arial, sans-serif; margin: 20px; }
h1 { color: #2c3e50; }
table { border-collapse: collapse; width: 100%; margin-top: 20px; }
th { background: #3498db; color: white; padding: 12px; text-align: left; }
td { border: 1px solid #ddd; padding: 10px; }
tr:nth-child(even) { background: #f9f9f9; }
.pending { background: #fff3cd; }
.confirmed { background: #d4edda; }
.urgent { color: #dc3545; font-weight: bold; }
.score-high { color: #28a745; }
.score-med { color: #ffc107; }
.score-low { color: #dc3545; }
.summary { background: #f8f9fa; padding: 15px; border-radius: 8px; margin-bottom: 20px; }
.alert { background: #fff3cd; padding: 10px; margin: 5px 0; border-left: 4px solid #ffc107; }
.alert-high { border-left-color: #dc3545; background: #f8d7da; }
a { color: #3498db; }
</style>
</head>
<body>
`)

	s := report.Summary
	buf.WriteString(fmt.Sprintf("<h1>📊 PharmaBroker Report - %s</h1>\n", s.GeneratedAt.Format("Jan 2, 2006 15:04")))

	// Summary box
	buf.WriteString(`<div class="summary">`)
	buf.WriteString(fmt.Sprintf("<strong>Period:</strong> %s to %s<br>", s.PeriodStart.Format("15:04"), s.PeriodEnd.Format("15:04")))
	buf.WriteString(fmt.Sprintf("<strong>Total Matches:</strong> %d | ", s.TotalMatches))
	buf.WriteString(fmt.Sprintf("<strong>Pending:</strong> %d | ", s.PendingCount))
	buf.WriteString(fmt.Sprintf("<strong>Confirmed:</strong> %d | ", s.ConfirmedCount))
	buf.WriteString(fmt.Sprintf("<strong>Avg Score:</strong> %.2f", s.AvgScore))
	buf.WriteString("</div>\n")

	// Alerts
	if len(report.Alerts) > 0 {
		buf.WriteString("<h2>⚠️ Alerts</h2>\n")
		for _, alert := range report.Alerts {
			class := "alert"
			if alert.Priority == "HIGH" {
				class += " alert-high"
			}
			buf.WriteString(fmt.Sprintf(`<div class="%s">%s</div>`+"\n", class, alert.Message))
		}
	}

	// Matches table
	buf.WriteString("<h2>📋 Matches</h2>\n")
	buf.WriteString(`<table>
<tr>
<th>Match</th>
<th>Score</th>
<th>Status</th>
<th>Medication</th>
<th>Seller</th>
<th>Buyer</th>
<th>Groups</th>
</tr>
`)

	for _, m := range report.Matches {
		statusClass := ""
		if m.Status == "PENDING" {
			statusClass = "pending"
		} else if m.Status == "CONFIRMED" {
			statusClass = "confirmed"
		}

		scoreClass := "score-low"
		if m.Score >= 0.9 {
			scoreClass = "score-high"
		} else if m.Score >= 0.7 {
			scoreClass = "score-med"
		}

		urgentFlag := ""
		if m.RequestUrgent {
			urgentFlag = `<span class="urgent"> 🔥 URGENT</span>`
		}

		sellerContact := fmt.Sprintf(`%s<br><a href="%s">%s</a>`, m.SellerName, m.SellerWhatsApp, m.SellerPhone)
		buyerContact := fmt.Sprintf(`%s<br><a href="%s">%s</a>`, m.BuyerName, m.BuyerWhatsApp, m.BuyerPhone)

		buf.WriteString(fmt.Sprintf(`<tr class="%s">
<td>%s<br><small>%s</small></td>
<td class="%s">%.2f</td>
<td>%s</td>
<td><strong>%s</strong> %dx @ %.0f EGP%s<br>↔ <strong>%s</strong> %dx max %.0f EGP</td>
<td>%s</td>
<td>%s</td>
<td>%s<br>↔ %s</td>
</tr>
`,
			statusClass,
			m.MatchID[:8], m.Breakdown,
			scoreClass, m.Score,
			m.Status,
			m.OfferMedication, int(m.OfferQty), m.OfferPrice, urgentFlag,
			m.RequestMedication, int(m.RequestQty), m.RequestMaxPrice,
			sellerContact,
			buyerContact,
			m.SellerGroup, m.BuyerGroup,
		))
	}

	buf.WriteString("</table>\n</body>\n</html>")

	return buf.String()
}

func filterByStatus(matches []*MatchReport, status string) []*MatchReport {
	var result []*MatchReport
	for _, m := range matches {
		if m.Status == status {
			result = append(result, m)
		}
	}
	return result
}

func formatMatchLine(m *MatchReport) string {
	urgent := ""
	if m.RequestUrgent {
		urgent = " 🔥"
	}
	return fmt.Sprintf("• %s %.0fx @ %.0f EGP%s\n  ↔️ Request %.0fx (max %.0f EGP)\n  👤 %s → %s\n  Score: %.2f\n\n",
		m.OfferMedication, m.OfferQty, m.OfferPrice, urgent,
		m.RequestQty, m.RequestMaxPrice,
		m.SellerName, m.BuyerName,
		m.Score,
	)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
