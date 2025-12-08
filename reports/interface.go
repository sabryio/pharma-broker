// Package reports provides report generation interfaces.
package reports

import (
	"context"
	"time"
)

// Generator generates reports from data
type Generator interface {
	// GenerateHourlyReport creates a complete hourly report
	GenerateHourlyReport(ctx context.Context, config Config) (*Report, error)

	// GenerateSummaryText creates a human-readable summary for messaging
	GenerateSummaryText(report *Report) string

	// GenerateHTMLReport creates an HTML table for email
	GenerateHTMLReport(report *Report) string

	// ExportToCSV exports the report to CSV format
	ExportToCSV(report *Report) ([]byte, error)

	// ExportToExcel exports the report to Excel format
	ExportToExcel(report *Report) ([]byte, error)
}

// Repository provides data for report generation
type Repository interface {
	GetMatchesForReport(ctx context.Context, config Config) ([]*MatchData, error)
	GetReportSummary(ctx context.Context, periodHours int) (*Summary, error)
	GetAlerts(ctx context.Context, periodHours int) ([]Alert, error)
}

// Config holds report configuration
type Config struct {
	PeriodHours   int
	IncludeAlerts bool
	MinConfidence float64
}

// Report represents a complete report
type Report struct {
	GeneratedAt time.Time
	PeriodHours int
	Summary     Summary
	Matches     []*MatchData
	Alerts      []Alert
}

// Summary contains aggregate statistics
type Summary struct {
	TotalMatches     int
	ConfirmedMatches int
	RejectedMatches  int
	PendingMatches   int
	AvgScore         float64
	TopMedications   []string
}

// MatchData represents match data for reports
type MatchData struct {
	ID                string
	OfferMedication   string
	OfferQuantity     float64
	OfferPrice        float64
	OfferSource       string
	RequestMedication string
	RequestQuantity   float64
	RequestMaxPrice   float64
	RequestSource     string
	Score             float64
	Status            string
	CreatedAt         time.Time
	ConfirmedAt       *time.Time
}

// Alert represents a system alert
type Alert struct {
	Type      string
	Message   string
	Severity  string // "info", "warning", "critical"
	CreatedAt time.Time
}
