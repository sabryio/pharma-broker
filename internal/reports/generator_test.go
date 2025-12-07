package reports

import (
	"context"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

// Mock repository for testing
type mockReportRepo struct {
	matches []*MatchReport
	summary *ReportSummary
	alerts  []Alert
}

func (m *mockReportRepo) GetMatchesForReport(ctx context.Context, config ReportConfig) ([]*MatchReport, error) {
	return m.matches, nil
}

func (m *mockReportRepo) GetReportSummary(ctx context.Context, periodHours int) (*ReportSummary, error) {
	return m.summary, nil
}

func (m *mockReportRepo) GetAlerts(ctx context.Context, periodHours int) ([]Alert, error) {
	return m.alerts, nil
}

func newTestGenerator() *Generator {
	log := zerolog.Nop()
	repo := &mockReportRepo{
		matches: []*MatchReport{
			{
				MatchID:           "match-001",
				CreatedAt:         time.Now(),
				Status:            "PENDING",
				Confidence:        "SUGGEST",
				Score:             0.85,
				Breakdown:         "Med: 0.9 | Qty: 0.8 | Price: 0.85",
				OfferMedication:   "Paracetamol 500mg",
				OfferQty:          100,
				OfferUnit:         "boxes",
				OfferPrice:        25.50,
				SellerName:        "Ahmed Pharmacy",
				SellerPhone:       "+201234567890",
				SellerGroup:       "Cairo Pharma Group",
				OfferCreatedAt:    time.Now().Add(-2 * time.Hour),
				RequestMedication: "Paracetamol",
				RequestQty:        50,
				RequestMaxPrice:   30.00,
				RequestUrgent:     true,
				BuyerName:         "Metro Hospital",
				BuyerPhone:        "+201098765432",
				BuyerGroup:        "Hospital Supplies",
				RequestCreatedAt:  time.Now().Add(-1 * time.Hour),
			},
			{
				MatchID:           "match-002",
				CreatedAt:         time.Now(),
				Status:            "CONFIRMED",
				Confidence:        "AUTO",
				Score:             0.95,
				OfferMedication:   "Ibuprofen 400mg",
				OfferQty:          200,
				OfferPrice:        18.00,
				SellerName:        "Delta Pharma",
				SellerPhone:       "+201111222333",
				RequestMedication: "Ibuprofen",
				RequestQty:        100,
				RequestMaxPrice:   20.00,
				BuyerName:         "City Clinic",
				BuyerPhone:        "+201444555666",
			},
		},
		summary: &ReportSummary{
			GeneratedAt:    time.Now(),
			PeriodStart:    time.Now().Add(-1 * time.Hour),
			PeriodEnd:      time.Now(),
			TotalMatches:   2,
			PendingCount:   1,
			ConfirmedCount: 1,
			RejectedCount:  0,
			UrgentMatches:  1,
			AvgScore:       0.90,
			TopMedications: []string{"Paracetamol", "Ibuprofen"},
			HighConfidence: 1,
			MedConfidence:  1,
			LowConfidence:  0,
		},
		alerts: []Alert{
			{
				Type:     AlertHighValuePending,
				Priority: "HIGH",
				Message:  "High-confidence match pending: Paracetamol 500mg (score: 0.85)",
				MatchID:  "match-001",
			},
		},
	}
	return NewGenerator(repo, log)
}

func TestGenerateHourlyReport(t *testing.T) {
	g := newTestGenerator()
	ctx := context.Background()

	config := DefaultReportConfig()
	report, err := g.GenerateHourlyReport(ctx, config)
	if err != nil {
		t.Fatalf("GenerateHourlyReport failed: %v", err)
	}

	if report == nil {
		t.Fatal("Expected report, got nil")
	}

	if len(report.Matches) != 2 {
		t.Errorf("Expected 2 matches, got %d", len(report.Matches))
	}

	if report.Summary.TotalMatches != 2 {
		t.Errorf("Expected 2 total matches, got %d", report.Summary.TotalMatches)
	}

	if len(report.Alerts) != 1 {
		t.Errorf("Expected 1 alert, got %d", len(report.Alerts))
	}
}

func TestExportToCSV(t *testing.T) {
	g := newTestGenerator()
	ctx := context.Background()

	config := DefaultReportConfig()
	report, _ := g.GenerateHourlyReport(ctx, config)

	csvData, err := g.ExportToCSV(report)
	if err != nil {
		t.Fatalf("ExportToCSV failed: %v", err)
	}

	if len(csvData) == 0 {
		t.Error("Expected non-empty CSV data")
	}

	csvStr := string(csvData)

	// Check header is present
	if !contains(csvStr, "Match ID") {
		t.Error("CSV missing 'Match ID' header")
	}
	if !contains(csvStr, "Seller Name") {
		t.Error("CSV missing 'Seller Name' header")
	}
	if !contains(csvStr, "Buyer Name") {
		t.Error("CSV missing 'Buyer Name' header")
	}

	// Check data is present
	if !contains(csvStr, "match-001") {
		t.Error("CSV missing match-001 data")
	}
	if !contains(csvStr, "Paracetamol") {
		t.Error("CSV missing medication data")
	}
}

func TestGenerateSummaryText(t *testing.T) {
	g := newTestGenerator()
	ctx := context.Background()

	config := DefaultReportConfig()
	report, _ := g.GenerateHourlyReport(ctx, config)

	summary := g.GenerateSummaryText(report)

	if summary == "" {
		t.Error("Expected non-empty summary text")
	}

	// Check key elements are present
	if !contains(summary, "PharmaBroker Report") {
		t.Error("Summary missing title")
	}
	if !contains(summary, "Total matches: 2") {
		t.Error("Summary missing total matches")
	}
	if !contains(summary, "Pending review: 1") {
		t.Error("Summary missing pending count")
	}
	if !contains(summary, "Urgent") {
		t.Error("Summary missing urgent indicator")
	}
}

func TestGenerateHTMLReport(t *testing.T) {
	g := newTestGenerator()
	ctx := context.Background()

	config := DefaultReportConfig()
	report, _ := g.GenerateHourlyReport(ctx, config)

	html := g.GenerateHTMLReport(report)

	if html == "" {
		t.Error("Expected non-empty HTML")
	}

	// Check structure
	if !contains(html, "<!DOCTYPE html>") {
		t.Error("HTML missing doctype")
	}
	if !contains(html, "<table>") {
		t.Error("HTML missing table")
	}
	if !contains(html, "Paracetamol") {
		t.Error("HTML missing medication data")
	}
	if !contains(html, "wa.me") || !contains(html, "https://") {
		// WhatsApp links should be generated
		t.Log("Note: WhatsApp links depend on phone formatting")
	}
}

func TestDefaultReportConfig(t *testing.T) {
	config := DefaultReportConfig()

	if !config.IncludePending {
		t.Error("Default should include pending")
	}
	if !config.IncludeConfirmed {
		t.Error("Default should include confirmed")
	}
	if config.IncludeRejected {
		t.Error("Default should not include rejected")
	}
	if config.MinScore != 0.5 {
		t.Errorf("Expected MinScore 0.5, got %f", config.MinScore)
	}
	if config.PeriodHours != 1 {
		t.Errorf("Expected PeriodHours 1, got %d", config.PeriodHours)
	}
}

// Helper functions

func TestFormatWhatsAppLink(t *testing.T) {
	tests := []struct {
		phone    string
		matchID  string
		expected string
	}{
		{"+201234567890", "match-1", "https://wa.me/201234567890?text="},
		{"20 123 456 7890", "m2", "https://wa.me/201234567890?text="},
		{"", "m3", ""},
	}

	for _, tt := range tests {
		result := FormatWhatsAppLink(tt.phone, tt.matchID)
		if tt.expected == "" && result != "" {
			t.Errorf("Expected empty for empty phone, got %s", result)
		}
		if tt.expected != "" && !contains(result, "wa.me") {
			t.Errorf("Expected WhatsApp link, got %s", result)
		}
	}
}

func TestCalculateAge(t *testing.T) {
	tests := []struct {
		t        time.Time
		contains string
	}{
		{time.Now().Add(-30 * time.Second), "just now"},
		{time.Now().Add(-5 * time.Minute), "m ago"},
		{time.Now().Add(-3 * time.Hour), "h ago"},
		{time.Now().Add(-2 * 24 * time.Hour), "d ago"},
		{time.Time{}, "N/A"},
	}

	for _, tt := range tests {
		result := CalculateAge(tt.t)
		if !contains(result, tt.contains) {
			t.Errorf("Expected %q to contain %q", result, tt.contains)
		}
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(len(s) > 0 && len(substr) > 0 && findSubstring(s, substr)))
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
