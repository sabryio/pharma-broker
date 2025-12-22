package reports

import (
	"context"
	"fmt"
	"net/url"
	"strings"
	"time"
)

// MatchReport represents a complete match with all contact details for reporting
type MatchReport struct {
	// Match Info
	MatchID    string    `json:"match_id"`
	CreatedAt  time.Time `json:"created_at"`
	Status     string    `json:"status"`
	Confidence string    `json:"confidence"`
	Score      float64   `json:"score"`
	Breakdown  string    `json:"breakdown"`

	// Offer/Seller Details
	OfferMedication string    `json:"offer_medication"`
	OfferQty        float64   `json:"offer_qty"`
	OfferUnit       string    `json:"offer_unit"`
	OfferPrice      float64   `json:"offer_price"`
	SellerName      string    `json:"seller_name"`
	SellerPhone     string    `json:"seller_phone"`
	SellerWhatsApp  string    `json:"seller_whatsapp"`
	SellerGroup     string    `json:"seller_group"`
	SellerGroupJID  string    `json:"seller_group_jid"`
	OfferCreatedAt  time.Time `json:"offer_created_at"`

	// Request/Buyer Details
	RequestMedication string    `json:"request_medication"`
	RequestQty        float64   `json:"request_qty"`
	RequestMaxPrice   float64   `json:"request_max_price"`
	RequestUrgent     bool      `json:"request_urgent"`
	BuyerName         string    `json:"buyer_name"`
	BuyerPhone        string    `json:"buyer_phone"`
	BuyerWhatsApp     string    `json:"buyer_whatsapp"`
	BuyerGroup        string    `json:"buyer_group"`
	BuyerGroupJID     string    `json:"buyer_group_jid"`
	RequestCreatedAt  time.Time `json:"request_created_at"`
}

// ReportSummary contains aggregated statistics
type ReportSummary struct {
	GeneratedAt    time.Time `json:"generated_at"`
	PeriodStart    time.Time `json:"period_start"`
	PeriodEnd      time.Time `json:"period_end"`
	TotalMatches   int       `json:"total_matches"`
	PendingCount   int       `json:"pending_count"`
	ConfirmedCount int       `json:"confirmed_count"`
	RejectedCount  int       `json:"rejected_count"`
	UrgentMatches  int       `json:"urgent_matches"`
	AvgScore       float64   `json:"avg_score"`
	TopMedications []string  `json:"top_medications"`
	HighConfidence int       `json:"high_confidence"` // AUTO band
	MedConfidence  int       `json:"med_confidence"`  // SUGGEST band
	LowConfidence  int       `json:"low_confidence"`  // REVIEW band
}

// HourlyReport is the complete report structure
type HourlyReport struct {
	Summary ReportSummary  `json:"summary"`
	Matches []*MatchReport `json:"matches"`
	Alerts  []Alert        `json:"alerts"`
}

// Alert represents a critical notification
type Alert struct {
	Type     AlertType `json:"type"`
	Priority string    `json:"priority"` // HIGH, MEDIUM, LOW
	Message  string    `json:"message"`
	MatchID  string    `json:"match_id,omitempty"`
}

// AlertType categorizes alerts
type AlertType string

const (
	AlertUrgentUnmatched  AlertType = "URGENT_UNMATCHED"
	AlertHighValuePending AlertType = "HIGH_VALUE_PENDING"
	AlertSystemError      AlertType = "SYSTEM_ERROR"
	AlertHighDemand       AlertType = "HIGH_DEMAND"
)

// FormatWhatsAppLink creates a clickable WhatsApp link with optional pre-filled message
func FormatWhatsAppLink(phone string, matchID string) string {
	// Clean phone number - remove spaces, dashes, plus
	clean := strings.ReplaceAll(phone, " ", "")
	clean = strings.ReplaceAll(clean, "-", "")
	clean = strings.ReplaceAll(clean, "+", "")

	if clean == "" {
		return ""
	}

	// Create pre-filled message
	msg := fmt.Sprintf("مرحباً، بخصوص PharmaBroker Match #%s", matchID)
	encoded := url.QueryEscape(msg)

	return fmt.Sprintf("https://wa.me/%s?text=%s", clean, encoded)
}

// FormatPhoneDisplay formats phone for display
func FormatPhoneDisplay(phone string) string {
	if phone == "" {
		return "N/A"
	}
	// Add + if not present
	if !strings.HasPrefix(phone, "+") {
		return "+" + phone
	}
	return phone
}

// CalculateAge returns human-readable age string
func CalculateAge(t time.Time) string {
	if t.IsZero() {
		return "N/A"
	}

	age := time.Since(t)

	if age < time.Minute {
		return "just now"
	} else if age < time.Hour {
		return fmt.Sprintf("%dm ago", int(age.Minutes()))
	} else if age < 24*time.Hour {
		return fmt.Sprintf("%dh ago", int(age.Hours()))
	} else {
		return fmt.Sprintf("%dd ago", int(age.Hours()/24))
	}
}

// CalculateAgeHours returns age in decimal hours
func CalculateAgeHours(t time.Time) float64 {
	if t.IsZero() {
		return 0
	}
	return time.Since(t).Hours()
}

// ReportConfig contains report generation settings
type ReportConfig struct {
	IncludePending   bool    `yaml:"include_pending"`
	IncludeConfirmed bool    `yaml:"include_confirmed"`
	IncludeRejected  bool    `yaml:"include_rejected"`
	UrgentOnly       bool    `yaml:"urgent_only"`
	MinScore         float64 `yaml:"min_score"`
	Limit            int     `yaml:"limit"`
	PeriodHours      int     `yaml:"period_hours"` // How many hours back to look
}

// DefaultReportConfig returns sensible defaults
func DefaultReportConfig() ReportConfig {
	return ReportConfig{
		IncludePending:   true,
		IncludeConfirmed: true,
		IncludeRejected:  false,
		UrgentOnly:       false,
		MinScore:         0.5,
		Limit:            100,
		PeriodHours:      1,
	}
}

// ReportGenerator interface for generating reports
type ReportGenerator interface {
	GenerateHourlyReport(ctx context.Context, config ReportConfig) (*HourlyReport, error)
	ExportToCSV(report *HourlyReport) ([]byte, error)
	ExportToExcel(report *HourlyReport) ([]byte, error)
}
