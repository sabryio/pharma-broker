package storage

import (
	"context"
	"os"
	"testing"
	"time"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/reports"

	"github.com/google/uuid"
)

func TestGetMatchesForReport(t *testing.T) {
	// Setup DB
	dbPath := "test_report_repo.db"
	_ = os.Remove(dbPath)
	defer os.Remove(dbPath)

	dbConf := &config.DatabaseConfig{Path: dbPath}
	db, err := New(dbConf)
	if err != nil {
		t.Fatalf("Failed to init DB: %v", err)
	}
	defer db.Close()

	// Initialize Repos
	rawMsgRepo := NewRawMessageRepo(db)
	offerRepo := NewOfferRepo(db)
	requestRepo := NewRequestRepo(db)
	matchRepo := NewMatchRepo(db)
	reportRepo := NewReportRepo(db)

	ctx := context.Background()

	// Helper to create data
	createMatch := func(matchID string, createdAt time.Time, status string) {
		// 1. Raw Message (needed for FK)
		rawID := uuid.New().String()
		rawMsg := &domain.RawMessage{
			ID:         rawID,
			ExternalID: uuid.New().String(),
			Content:    "Test content",
			Timestamp:  time.Now(),
		}
		rawMsgRepo.Save(ctx, rawMsg)

		// 2. Offer
		offerID := uuid.New().String()
		offer := &domain.Offer{
			ID:           offerID,
			Medication:   "Med A",
			Status:       domain.StatusActive,
			RawMessageID: rawID,
			SourceName:   "Seller A",
		}
		offerRepo.Save(ctx, offer)

		// 3. Request
		reqID := uuid.New().String()
		req := &domain.Request{
			ID:           reqID,
			Medication:   "Med A",
			Status:       domain.StatusActive,
			RawMessageID: rawID,
			SourceName:   "Buyer B",
		}
		requestRepo.Save(ctx, req)

		// 4. Match
		match := &domain.Match{
			ID:        matchID,
			OfferID:   offerID,
			RequestID: reqID,
			Score:     0.9,
			Status:    domain.MatchStatus(status),
			CreatedAt: createdAt,
		}
		if err := matchRepo.Save(ctx, match); err != nil {
			t.Fatalf("Failed to save match: %v", err)
		}

		// Hack: Update CreatedAt manually because Save() might overwrite it with time.Now()
		// if logic dictates, or if we want precise control.
		// Looking at MatchRepo.Save, it uses query arg for CreatedAt.
		// But let's verify if repo overrides it.
		// Assuming repo saves what passed.
		// However, sqlite time might lose precision or be string.
		// Ideally we test retrieval logic.
	}

	now := time.Now()

	// Create matches with different ages
	createMatch("match-recent", now.Add(-1*time.Minute), "PENDING")
	createMatch("match-old", now.Add(-25*time.Hour), "PENDING")
	createMatch("match-confirmed", now.Add(-1*time.Hour), "CONFIRMED")
	createMatch("match-rejected", now.Add(-1*time.Hour), "REJECTED")

	tests := []struct {
		name          string
		config        reports.ReportConfig
		expectedCount int
		expectedIDs   []string
	}{
		{
			name: "Recent Pending (Last 24h)",
			config: reports.ReportConfig{
				PeriodHours:    24,
				IncludePending: true,
				Limit:          10,
				MinScore:       0.0,
			},
			expectedCount: 1, // match-recent only (confirmed is excluded by default false)
			expectedIDs:   []string{"match-recent"},
		},
		{
			name: "Recent Pending & Confirmed (Last 24h)",
			config: reports.ReportConfig{
				PeriodHours:      24,
				IncludePending:   true,
				IncludeConfirmed: true,
				Limit:            10,
			},
			expectedCount: 2, // match-recent, match-confirmed
			expectedIDs:   []string{"match-recent", "match-confirmed"},
		},
		{
			name: "All Time (Huge Window covers old)",
			config: reports.ReportConfig{
				PeriodHours:    48,
				IncludePending: true,
				Limit:          10,
			},
			expectedCount: 2, // match-recent, match-old
			expectedIDs:   []string{"match-recent", "match-old"},
		},
		{
			name: "Strict Score (Min 0.95)",
			config: reports.ReportConfig{
				PeriodHours:    24,
				IncludePending: true,
				MinScore:       0.95, // created matches have 0.9
				Limit:          10,
			},
			expectedCount: 0,
		},
		{
			name: "Zero Limit (Should Default to > 0)",
			config: reports.ReportConfig{
				PeriodHours:      24,
				IncludePending:   true,
				IncludeConfirmed: true,
				Limit:            0, // Testing the fix
			},
			expectedCount: 2,
			expectedIDs:   []string{"match-recent", "match-confirmed"},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			matches, err := reportRepo.GetMatchesForReport(ctx, tc.config)
			if err != nil {
				t.Fatalf("GetMatchesForReport failed: %v", err)
			}

			// Validate Count
			// Note: "Recent Pending" case calculation above might be wrong if logic defaults apply.
			// Let's rely on explicit struct init.
			// match-recent (PENDING, -1m): Matches Period(24h) & Status(PENDING)
			// match-old (PENDING, -25h): Fails Period(24h)
			// match-confirmed (CONFIRMED, -1h): Matches Period(24h) but fails Status(PENDING only if !IncludeConfirmed)

			// So Case 1: PENDING only. Should match ONLY match-recent. (Count 1)

			// Adjusting expected counts in loop logic or hardcoding correctly above.
			// Case 1 "Recent Pending" -> expect 1 ("match-recent")
			// Case 2 "Recent P & C" -> expect 2 ("match-recent", "match-confirmed")
			// Case 3 "All Time P" -> expect 2 ("match-recent", "match-old")

			if len(matches) != tc.expectedCount {
				t.Errorf("Expected %d matches, got %d", tc.expectedCount, len(matches))
				for _, m := range matches {
					t.Logf("Found: %s (%s, %v)", m.MatchID, m.Status, m.CreatedAt)
				}
			}
		})
	}
}
